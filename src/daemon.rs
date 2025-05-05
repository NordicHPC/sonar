#![allow(clippy::comparison_to_empty)]
#![allow(clippy::len_zero)]

// TODO in this file, all marked:
//
// Low pri
//  - lock file
//  - signal handling to deal properly with with lock file
//  - reload config under signal + remote control (exiting and restarting via systemd is
//    just fine for starters)
//  - maybe pause / restart remote control
//  - more flexible cadence computation
//  - more test cases for the cadence computation

// For documentation about configuration and message topics that are used, see ~/doc/HOWTO-DAEMON.md.

// THREADS AND I/O
//
// The main thread of the daemon listens on a channel from which it reads events: alarms (for work
// to do), signals (keyboard interrupts), and incoming messages (from some controlling agent).
//
// Signal handlers place signals in the daemon's channel as events.

use crate::cluster;
use crate::datasink::{DataSink, StdioSink};
use crate::jobsapi;
use crate::json_tags;
#[cfg(feature = "kafka")]
use crate::kafka::RdKafka;
use crate::log;
use crate::ps;
use crate::realsystem;
use crate::slurmjobs;
use crate::sysinfo;
use crate::systemapi::SystemAPI;
use crate::time::{unix_now, unix_time_components};

use std::io::BufRead;
use std::sync::mpsc;
use std::thread;

pub struct GlobalIni {
    pub cluster: String,
    pub role: String,
    pub lockdir: Option<String>,
    pub topic_prefix: Option<String>,
}

#[cfg(feature = "kafka")]
pub struct KafkaIni {
    pub broker_address: String,
    pub sending_window: Dur,
    pub ca_file: Option<String>,
    pub sasl_password: Option<String>,
}

pub struct DebugIni {
    pub verbose: bool,
}

pub struct SampleIni {
    pub cadence: Option<Dur>,
    pub exclude_system_jobs: bool,
    pub load: bool,
    pub batchless: bool,
    pub exclude_commands: Vec<String>,
    pub exclude_users: Vec<String>,
}

pub struct SysinfoIni {
    pub on_startup: bool,
    pub cadence: Option<Dur>,
}

pub struct JobsIni {
    pub cadence: Option<Dur>,
    pub window: Option<Dur>,
    pub uncompleted: bool,
}

pub struct ClusterIni {
    pub cadence: Option<Dur>,
}

pub struct Ini {
    pub global: GlobalIni,
    #[cfg(feature = "kafka")]
    pub kafka: KafkaIni,
    pub debug: DebugIni,
    pub sample: SampleIni,
    pub sysinfo: SysinfoIni,
    pub jobs: JobsIni,
    pub cluster: ClusterIni,
}

#[derive(Clone, Debug)]
pub enum Operation {
    Sample,
    Sysinfo,
    Jobs,
    Cluster,
    // Signals: signal code
    #[allow(dead_code)]
    Signal(i32),
    // Control messages: key/value
    #[allow(dead_code)]
    Incoming(String, String),
    // Unrecoverable errors on other threads: explanatory message
    #[allow(dead_code)]
    Fatal(String),
    // Maybe-recoverable message delivery error: explanatory message
    #[allow(dead_code)]
    MessageDeliveryError(String),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Dur {
    Hours(u64),
    Minutes(u64),
    Seconds(u64),
}

#[allow(dead_code)]
impl Dur {
    pub fn to_seconds(self) -> u64 {
        match self {
            Dur::Hours(n) => n * 60 * 60,
            Dur::Minutes(n) => n * 60,
            Dur::Seconds(n) => n,
        }
    }

    pub fn to_minutes(self) -> u64 {
        match self {
            Dur::Hours(n) => n * 60,
            Dur::Minutes(n) => n,
            Dur::Seconds(n) => (n + 59) / 60,
        }
    }
}

// The daemon_mode() should return under these circumstances *only*:
//
// - it fails to parse the ini file
// - it fails to read any specified files
// - it fails to acquire the lock file
// - it fails to setup an interrupt handler
// - an exit control message is received from the broker
// - a signal is received from the OS or user that signifies an exit condition
//
// Other errors require threads to post messages back to the main thread.
//
// It is crucial that the system does not terminate suddenly once the lock file is held: we must
// unwind properly and do what we can to relinquish the lock.

pub fn daemon_mode(
    config_file: &str,
    mut system: realsystem::RealSystemBuilder,
    force_slurm: bool,
) -> Result<(), String> {
    let ini = parse_config(config_file)?;

    if ini.sample.cadence.is_some() {
        system = system.with_jobmanager(Box::new(jobsapi::AnyJobManager::new(force_slurm)));
    }

    let system = system.with_cluster(&ini.global.cluster).freeze()?;
    let hostname = system.get_hostname();
    let api_token = "".to_string();

    if ini.global.lockdir.is_some() {
        // TODO: Acquire lockdir here
        // TODO: Set up interrupt handling
        todo!();
    }

    // For communicating with the main thread.  event_sender is used to send timer ticks (from timer
    // threads), incoming messages (from the data sink), signals (from the signal handling thread),
    // and errors (from anywhere); event_receiver receives those events in the main thread.
    let (event_sender, event_receiver) = mpsc::channel();

    // If sysinfo runs on startup then post a message to ourselves.  Should never fail.
    if ini.sysinfo.cadence.is_some() && ini.sysinfo.on_startup {
        let _ignored = event_sender.send(Operation::Sysinfo);
    }

    // Alarms for daemon operations - each alarm gets its own thread, wasteful but OK for the time
    // being.  These will post the given events at the given cadences.
    if let Some(c) = ini.sysinfo.cadence {
        let sender = event_sender.clone();
        thread::spawn(move || {
            repeated_event(json_tags::DATA_TAG_SYSINFO, sender, Operation::Sysinfo, c);
        });
    }
    if let Some(c) = ini.sample.cadence {
        let sender = event_sender.clone();
        thread::spawn(move || {
            repeated_event(json_tags::DATA_TAG_SAMPLE, sender, Operation::Sample, c);
        });
    }
    if let Some(c) = ini.jobs.cadence {
        let sender = event_sender.clone();
        thread::spawn(move || {
            repeated_event(json_tags::DATA_TAG_JOBS, sender, Operation::Jobs, c);
        });
    }
    if let Some(c) = ini.cluster.cadence {
        let sender = event_sender.clone();
        thread::spawn(move || {
            repeated_event(json_tags::DATA_TAG_CLUSTER, sender, Operation::Cluster, c);
        });
    }

    let client_id = ini.global.cluster.clone() + "/" + &hostname;

    let mut control_topic = ini.global.cluster.clone() + ".control." + &ini.global.role;
    if let Some(ref prefix) = ini.global.topic_prefix {
        control_topic = prefix.clone() + "." + &control_topic;
    }

    #[cfg(not(feature = "kafka"))]
    let data_sink: Box<dyn DataSink> =
        Box::new(StdioSink::new(client_id, control_topic, event_sender));

    #[cfg(feature = "kafka")]
    let data_sink: Box<dyn DataSink> = if ini.kafka.broker_address != "" {
        Box::new(RdKafka::new(&ini, client_id, control_topic, event_sender))
    } else {
        Box::new(StdioSink::new(client_id, control_topic, event_sender))
    };

    if ini.debug.verbose {
        log::verbose("Initialization succeeded");
    }

    let mut sample_extractor = ps::State::new(
        &system,
        &ps::PsOptions {
            rollup: false,
            min_cpu_percent: None,
            min_mem_percent: None,
            min_cpu_time: None,
            exclude_system_jobs: ini.sample.exclude_system_jobs,
            load: ini.sample.load,
            exclude_users: ini.sample.exclude_users.clone(),
            exclude_commands: ini.sample.exclude_commands.clone(),
            lockdir: ini.global.lockdir.clone(),
            new_json: true,
            cpu_util: true,
            token: api_token.clone(),
        },
    );

    let mut sysinfo_extractor = sysinfo::State::new(&system, api_token.clone());

    let mut cluster_extractor = cluster::State::new(&system, api_token.clone());

    let mut slurm_extractor = slurmjobs::State::new(
        ini.jobs.window.map(|c| c.to_minutes() as u32),
        ini.jobs.uncompleted,
        &system,
        api_token.clone(),
    );

    let mut fatal_msg = "".to_string();
    'messageloop: loop {
        let mut output = Vec::new();

        // Nobody gets to close this channel, so panic on error
        let op = event_receiver.recv().expect("Event queue receive");

        system.update_time();
        let topic: &'static str;
        match op {
            Operation::Sample => {
                if ini.debug.verbose {
                    log::verbose("Sample");
                }
                sample_extractor.run(&mut output);
                topic = json_tags::DATA_TAG_SAMPLE;
            }
            Operation::Sysinfo => {
                if ini.debug.verbose {
                    log::verbose("Sysinfo");
                }
                sysinfo_extractor.run(&mut output);
                topic = json_tags::DATA_TAG_SYSINFO;
            }
            Operation::Jobs => {
                if ini.debug.verbose {
                    log::verbose("Jobs");
                }
                slurm_extractor.run(&mut output);
                topic = json_tags::DATA_TAG_JOBS;
            }
            Operation::Cluster => {
                if ini.debug.verbose {
                    log::verbose("Cluster");
                }
                cluster_extractor.run(&mut output);
                topic = json_tags::DATA_TAG_CLUSTER;
            }
            Operation::Signal(s) => {
                if ini.debug.verbose {
                    log::verbose(&format!("signal {s}"));
                }
                match s {
                    libc::SIGINT | libc::SIGHUP => {
                        break 'messageloop;
                    }
                    libc::SIGTERM => {
                        // TODO: reload
                        continue 'messageloop;
                    }
                    _ => {
                        continue 'messageloop;
                    }
                }
            }
            Operation::Incoming(key, value) => {
                if ini.debug.verbose {
                    log::verbose("Incoming");
                }
                // TODO: Maybe a reload function
                // TODO: Maybe pause / restart functions
                match (key.as_str(), value.as_str()) {
                    ("exit", _) => {
                        break 'messageloop;
                    }
                    _ => {}
                }
                continue 'messageloop;
            }
            Operation::Fatal(msg) => {
                if ini.debug.verbose {
                    log::verbose(&format!("Fatal error: {msg}"));
                }
                fatal_msg = msg;
                break 'messageloop;
            }
            Operation::MessageDeliveryError(msg) => {
                if ini.debug.verbose {
                    log::verbose(&msg);
                }
                log::error(&msg);
                break 'messageloop;
            }
        }

        let mut topic = ini.global.cluster.clone() + "." + topic;
        if let Some(ref prefix) = ini.global.topic_prefix {
            topic = prefix.clone() + "." + &topic;
        }
        let key = hostname.clone();
        let value = String::from_utf8_lossy(&output).to_string();

        data_sink.post(topic, key, value);
    }

    data_sink.stop();

    // Other threads will need to not panic when they are killed on shutdown, but otherwise there's
    // nothing special to be done here for the repeater threads or even the signal processing
    // thread.

    if ini.global.lockdir.is_some() {
        // TODO: Relinquish lockdir here
        // TODO: Maybe tear down interrupt handling?
        todo!();
    }

    if fatal_msg != "" {
        Err(fatal_msg)
    } else {
        Ok(())
    }
}

///////////////////////////////////////////////////////////////////////////////////////////////////
//
// Alarms and cadences.

fn repeated_event(_whoami: &str, sender: mpsc::Sender<Operation>, op: Operation, cadence: Dur) {
    let now = unix_now();
    let first = time_at_next_cadence_point(now, cadence);
    let initial_delay = first as i64 - now as i64;
    if initial_delay > 0 {
        thread::sleep(std::time::Duration::from_secs(initial_delay as u64));
    }
    let delay = cadence.to_seconds();
    let mut count = 0u64;
    loop {
        // If the send fails then the main loop has closed the connection and this is our signal to
        // shut down the thread.
        if sender.send(op.clone()).is_err() {
            break;
        }

        // It may be important to the back-end that events are triggered roughly at the expected
        // time according to the cadence (certainly it was important for the initial back-end).
        // Therefore we compute the next event point as the initial event point plus a multiple of
        // the cadence.  This mostly removes the risk of getting out of sync with the desired
        // cadence points.
        //
        // There should be no risk of wrapping around or overflowing here.  The product of count and
        // delay is always the number of seconds since the first event.  2^64 seconds is nearly 6e11
        // years.  The initial timestamp is some 32-bit value for the foreseeable future.

        count += 1;
        let next = first + count * delay;
        let next_delay = next as i64 - unix_now() as i64;
        if next_delay > 0 {
            thread::sleep(std::time::Duration::from_secs(next_delay as u64));
        }
    }
}

// Round up `now` to the next multiple of `cadence`.  For example, if `cadence` is 5m then the value
// returned represents the unix time at the next 5 minute mark; if `cadence` is 24h then the value
// is the time at next midnight.  It's OK for this to be expensive (for now).  This can validly
// return `now`.
//
// The many restrictions on cadences ensure that this rounding is well-defined and leads to
// well-defined sample points across all nodes (that have sensibly synchronized clocks and are in
// compatible time zones).
//
// Multi-day boundaries are a little tricky but we can use the next midnight s.t.  the number of
// days evenly divides the day number.
//
// TODO: Some sensible cadences such as 90m aka 1h30m are not currently expressible.

fn time_at_next_cadence_point(now: u64, cadence: Dur) -> u64 {
    let (_, _, day, hour, minute, second) = unix_time_components(now);
    now + match cadence {
        Dur::Seconds(s) => s - second % s,
        Dur::Minutes(m) => 60 * (m - minute % m) - second,
        Dur::Hours(h) if h <= 24 => 60 * (60 * (h - hour % h) - minute) - second,
        Dur::Hours(h) => {
            let d = h / 24;
            60 * (60 * (24 * (d - day % d) - hour) - minute) - second
        }
    }
}

#[test]
pub fn test_cadence_computer() {
    // TODO: Add some harder test cases

    // 1740568588-2025-02-26T11:16:28
    let now = 1740568588;

    // next 15-second boundary
    let (_year, _month, _day, hour, minute, second) =
        unix_time_components(time_at_next_cadence_point(now, Dur::Seconds(15)));
    assert!(hour == 11);
    assert!(minute == 16);
    assert!(second == 30);

    let (_year, _month, _day, hour, minute, second) =
        unix_time_components(time_at_next_cadence_point(now + 15, Dur::Seconds(15)));
    assert!(hour == 11);
    assert!(minute == 16);
    assert!(second == 45);

    let (_year, _month, _day, hour, minute, second) =
        unix_time_components(time_at_next_cadence_point(now + 30, Dur::Seconds(15)));
    assert!(hour == 11);
    assert!(minute == 17);
    assert!(second == 00);

    let (_year, _month, _day, hour, minute, second) =
        unix_time_components(time_at_next_cadence_point(now + 45, Dur::Seconds(15)));
    assert!(hour == 11);
    assert!(minute == 17);
    assert!(second == 15);

    // next 2-second boundary...
    let (_year, _month, _day, hour, minute, second) =
        unix_time_components(time_at_next_cadence_point(now, Dur::Seconds(2)));
    assert!(hour == 11);
    assert!(minute == 16);
    assert!(second == 30);

    let (_year, _month, _day, hour, minute, second) =
        unix_time_components(time_at_next_cadence_point(now + 31, Dur::Seconds(2)));
    println!("{hour} {minute} {second}");
    assert!(hour == 11);
    assert!(minute == 17);
    assert!(second == 00);

    // next 1-minute boundary
    let (_year, _month, _day, hour, minute, second) =
        unix_time_components(time_at_next_cadence_point(now, Dur::Minutes(1)));
    println!("{hour} {minute} {second}");
    assert!(hour == 11);
    assert!(minute == 17);
    assert!(second == 00);

    // next 5-minute boundary
    let (year, month, day, hour, minute, second) =
        unix_time_components(time_at_next_cadence_point(now, Dur::Minutes(5)));
    assert!(year == 2025);
    assert!(month == 1);
    assert!(day == 25);
    assert!(hour == 11);
    assert!(minute == 20);
    assert!(second == 0);

    // next 2-hour boundary
    let (_year, _month, _day, hour, minute, second) =
        unix_time_components(time_at_next_cadence_point(now, Dur::Hours(2)));
    assert!(hour == 12);
    assert!(minute == 00);
    assert!(second == 00);

    // next 24-hour boundary is just next midnight
    let (year, month, day, hour, minute, second) =
        unix_time_components(time_at_next_cadence_point(now, Dur::Hours(24)));
    assert!(year == 2025);
    assert!(month == 1);
    assert!(day == 26);
    assert!(hour == 00);
    assert!(minute == 00);
    assert!(second == 00);

    let (year, month, day, hour, minute, second) =
        unix_time_components(time_at_next_cadence_point(now, Dur::Hours(48)));
    assert!(year == 2025);
    assert!(month == 1);
    assert!(day == 26);
    assert!(hour == 00);
    assert!(minute == 00);
    assert!(second == 00);

    let (year, month, day, hour, minute, second) =
        unix_time_components(time_at_next_cadence_point(now, Dur::Hours(72)));
    //println!("72: {year} {month} {day} {hour} {minute} {second}");
    assert!(year == 2025);
    assert!(month == 1);
    assert!(day == 27);
    assert!(hour == 00);
    assert!(minute == 00);
    assert!(second == 00);
}

///////////////////////////////////////////////////////////////////////////////////////////////////
//
// Yet another config file parser.

fn parse_config(config_file: &str) -> Result<Ini, String> {
    let mut ini = Ini {
        global: GlobalIni {
            cluster: "".to_string(),
            role: "".to_string(),
            lockdir: None,
            topic_prefix: None,
        },
        #[cfg(feature = "kafka")]
        kafka: KafkaIni {
            broker_address: "".to_string(),
            sending_window: Dur::Minutes(5),
            ca_file: None,
            sasl_password: None,
        },
        debug: DebugIni { verbose: false },
        sample: SampleIni {
            cadence: None,
            exclude_system_jobs: true,
            load: true,
            batchless: false,
            exclude_commands: vec![],
            exclude_users: vec![],
        },
        sysinfo: SysinfoIni {
            on_startup: true,
            cadence: None,
        },
        jobs: JobsIni {
            cadence: None,
            window: None,
            uncompleted: false,
        },
        cluster: ClusterIni { cadence: None },
    };

    enum Section {
        None,
        Global,
        #[cfg(feature = "kafka")]
        Kafka,
        Debug,
        Sample,
        Sysinfo,
        Jobs,
        Cluster,
    }
    let mut curr_section = Section::None;
    #[cfg(feature = "kafka")]
    let mut have_kafka = false;
    #[cfg(feature = "kafka")]
    let mut have_kafka_remote = false;
    let file = match std::fs::File::open(config_file) {
        Ok(f) => f,
        Err(e) => {
            return Err(format!("{e}"));
        }
    };
    for l in std::io::BufReader::new(file).lines() {
        let l = match l {
            Ok(l) => l,
            Err(e) => {
                return Err(format!("{e}"));
            }
        };
        if l.starts_with('#') {
            continue;
        }
        let l = trim_ascii(&l);
        if l.len() == 0 {
            continue;
        }
        if l == "[global]" {
            curr_section = Section::Global;
            continue;
        }
        #[cfg(feature = "kafka")]
        if l == "[kafka]" {
            curr_section = Section::Kafka;
            have_kafka = true;
            continue;
        }
        if l == "[debug]" {
            curr_section = Section::Debug;
            continue;
        }
        if l == "[ps]" || l == "[sample]" {
            curr_section = Section::Sample;
            continue;
        }
        if l == "[sysinfo]" {
            curr_section = Section::Sysinfo;
            continue;
        }
        if l == "[slurm]" || l == "[jobs]" {
            curr_section = Section::Jobs;
            continue;
        }
        if l == "[cluster]" {
            curr_section = Section::Cluster;
            continue;
        }
        if l.starts_with("[") {
            return Err(format!("Unknown section {l}"));
        }

        let (name, value) = parse_setting(l)?;
        match curr_section {
            Section::None => return Err("Setting outside section".to_string()),
            Section::Global => match name.as_str() {
                "cluster" => {
                    ini.global.cluster = value;
                }
                "role" => match value.as_str() {
                    "node" | "master" => {
                        ini.global.role = value;
                    }
                    _ => return Err(format!("Invalid global.role value `{value}`")),
                },
                "lockdir" => {
                    ini.global.lockdir = Some(value);
                }
                "topic-prefix" => {
                    ini.global.topic_prefix = Some(value);
                }
                _ => return Err(format!("Invalid [global] setting name `{name}`")),
            },
            #[cfg(feature = "kafka")]
            Section::Kafka => match name.as_str() {
                "broker-address" | "remote-host" => {
                    ini.kafka.broker_address = value;
                    have_kafka_remote = true;
                }
                "sending-window" => {
                    ini.kafka.sending_window =
                        parse_duration("kafka.sending-window", &value, true)?;
                }
                "ca-file" => {
                    ini.kafka.ca_file = Some(value);
                }
                "sasl-password" => {
                    ini.kafka.sasl_password = Some(value);
                }
                _ => return Err(format!("Invalid [kafka] setting name `{name}`")),
            },
            Section::Debug => match name.as_str() {
                "verbose" => {
                    ini.debug.verbose = parse_bool(&value)?;
                }
                _ => return Err(format!("Invalid [debug] setting name `{name}`")),
            },
            Section::Sample => match name.as_str() {
                "cadence" => {
                    ini.sample.cadence = Some(parse_duration("sample.cadence", &value, false)?);
                }
                "exclude-system-jobs" => {
                    ini.sample.exclude_system_jobs = parse_bool(&value)?;
                }
                "load" => {
                    ini.sample.load = parse_bool(&value)?;
                }
                "exclude-users" => {
                    ini.sample.exclude_users = parse_strings(&value)?;
                }
                "exclude-commands" => {
                    ini.sample.exclude_commands = parse_strings(&value)?;
                }
                "batchless" => {
                    ini.sample.batchless = parse_bool(&value)?;
                }
                _ => return Err(format!("Invalid [sample]/[ps] setting name `{name}`")),
            },
            Section::Sysinfo => match name.as_str() {
                "on-startup" => {
                    ini.sysinfo.on_startup = parse_bool(&value)?;
                }
                "cadence" => {
                    ini.sysinfo.cadence = Some(parse_duration("sysinfo.cadence", &value, false)?);
                }
                _ => return Err(format!("Invalid [sysinfo] setting name `{name}`")),
            },
            Section::Jobs => match name.as_str() {
                "cadence" => {
                    let dur = parse_duration("jobs.cadence", &value, false)?;
                    ini.jobs.cadence = Some(dur);
                    if ini.jobs.window.is_none() {
                        ini.jobs.window = Some(Dur::Seconds(2 * dur.to_seconds()));
                    }
                }
                "window" => {
                    ini.jobs.window = Some(parse_duration("jobs.window", &value, true)?);
                }
                "uncompleted" | "incomplete" => {
                    ini.jobs.uncompleted = parse_bool(&value)?;
                }
                _ => return Err(format!("Invalid [jobs] setting name `{name}`")),
            },
            Section::Cluster => match name.as_str() {
                "cadence" => {
                    ini.cluster.cadence = Some(parse_duration("cluster.cadence", &value, false)?);
                }
                _ => return Err(format!("Invalid [cluster] setting name `{name}`")),
            },
        }
    }

    if ini.global.cluster == "" {
        return Err("Missing global.cluster setting".to_string());
    }
    if ini.global.role == "" {
        return Err("Missing global.role setting".to_string());
    }

    #[cfg(feature = "kafka")]
    if have_kafka {
        if !have_kafka_remote {
            return Err("Missing kafka.remote-host setting".to_string());
        }
        if ini.kafka.sasl_password.is_some() {
            if ini.kafka.ca_file.is_none()
                || (ini.kafka.sasl_password.as_ref().unwrap() != ""
                    && ini.kafka.ca_file.as_ref().unwrap() == "")
            {
                return Err("kafka.sasl_password requires kafka.ca_file".to_string());
            }
        }
    }

    Ok(ini)
}

fn parse_setting(l: &str) -> Result<(String, String), String> {
    if let Some((name, value)) = l.split_once('=') {
        let name = trim_ascii(name);
        // A little too lenient
        for c in name.chars() {
            if !(c >= 'A' && c <= 'Z'
                || c >= 'a' && c <= 'z'
                || c >= '0' && c <= '9'
                || c == '-'
                || c == '_')
            {
                return Err("Illegal character in name".to_string());
            }
        }
        let value = trim_ascii(value);
        if value == "" {
            return Err("Empty string must be quoted".to_string());
        }
        let value = trim_quotes(value)?;
        Ok((name.to_string(), value.to_string()))
    } else {
        Err("Illegal property definition".to_string())
    }
}

fn parse_bool(l: &str) -> Result<bool, String> {
    match l {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(format!("Invalid boolean value {l}")),
    }
}

fn parse_duration(context: &str, l: &str, lenient: bool) -> Result<Dur, String> {
    if let Some(hours) = l.strip_suffix(['h', 'H']) {
        if let Ok(k) = hours.parse::<u64>() {
            if k > 0 && (lenient || 24 % k == 0 || k % 24 == 0) {
                return Ok(Dur::Hours(k));
            }
        }
        return Err(format!("Bad duration in {context}"));
    }
    if let Some(minutes) = l.strip_suffix(['m', 'M']) {
        if let Ok(k) = minutes.parse::<u64>() {
            if k > 0 && (lenient || (k < 60 && 60 % k == 0)) {
                return Ok(Dur::Minutes(k));
            }
        }
        return Err(format!("Bad duration in {context}"));
    }
    if let Some(seconds) = l.strip_suffix(['s', 'S']) {
        if let Ok(k) = seconds.parse::<u64>() {
            if k > 0 && (lenient || (k < 60 && 60 % k == 0)) {
                return Ok(Dur::Seconds(k));
            }
        }
    }
    Err(format!("Bad duration in {context}"))
}

fn parse_strings(l: &str) -> Result<Vec<String>, String> {
    if l == "" {
        Ok(vec![])
    } else {
        Ok(l.split(',').map(|x| x.to_string()).collect::<Vec<String>>())
    }
}

fn trim_ascii(l: &str) -> &str {
    // std::str::trim_ascii() is rust v1.80 or later, implement a simple one
    let bs = l.as_bytes();
    let mut first = 0;
    while first < bs.len() && (bs[first] == b' ' || bs[first] == b'\t') {
        first += 1;
    }
    if first == bs.len() {
        return "";
    }
    let mut limit = bs.len();
    while bs[limit - 1] == b' ' || bs[limit - 1] == b'\t' {
        limit -= 1;
    }
    std::str::from_utf8(&bs[first..limit]).unwrap()
}

#[test]
pub fn test_trim_ascii() {
    assert!(trim_ascii(" abc ") == "abc");
    assert!(trim_ascii("  ") == "");
    assert!(trim_ascii("a b") == "a b");
    assert!(trim_ascii("") == "");
    assert!(trim_ascii(" \t abc\t \t") == "abc");
}

fn trim_quotes(l: &str) -> Result<&str, String> {
    // Invariant: bs.len() > 0
    let bs = l.as_bytes();
    if bs[0] == b'\'' || bs[0] == b'"' || bs[0] == b'`' {
        if bs.len() < 2 || bs[0] != bs[bs.len() - 1] {
            Err("Mismatched quotes".to_string())
        } else {
            Ok(std::str::from_utf8(&bs[1..bs.len() - 1]).unwrap())
        }
    } else {
        Ok(l)
    }
}

#[test]
pub fn test_trim_quotes() {
    assert!(trim_quotes("abc").unwrap() == "abc");
    assert!(trim_quotes("'abc'").unwrap() == "abc");
    assert!(trim_quotes("`abc`").unwrap() == "abc");
    assert!(trim_quotes("abc`").unwrap() == "abc`"); // Only leading quote strips the trailing one
    assert!(trim_quotes("\"abc\"").unwrap() == "abc");
    assert!(trim_quotes("'abc").is_err());
    assert!(trim_quotes("'abc`").is_err());
}

#[test]
pub fn test_parser() {
    let (a, b) = parse_setting(" x-factor = 10 ").unwrap();
    assert!(a == "x-factor");
    assert!(b == "10");
    let (a, b) = parse_setting("X_fact0r=`10 + 20`").unwrap();
    assert!(a == "X_fact0r");
    assert!(b == "10 + 20");
    assert!(parse_bool("true") == Ok(true));
    assert!(parse_bool("false") == Ok(false));
    assert!(parse_strings("").unwrap().len() == 0);
    assert!(parse_strings("a,b").unwrap().len() == 2);
    assert!(parse_duration("", "30s", true).unwrap() == Dur::Seconds(30));
    assert!(parse_duration("", "10m", true).unwrap() == Dur::Minutes(10));
    assert!(parse_duration("", "6H", true).unwrap() == Dur::Hours(6));

    assert!(parse_setting("zappa").is_err());
    assert!(parse_setting("zappa = ").is_err());
    assert!(parse_setting("zappa = `abracadabra").is_err());
    assert!(parse_setting("zapp! = true").is_err());
    assert!(parse_bool("tru").is_err());
    assert!(parse_duration("", "35", true).is_err());
    assert!(parse_duration("", "12m35s", true).is_err());
    assert!(parse_duration("", "3H12M35X", true).is_err());

    let ini = parse_config("src/testdata/daemon-stdio-config.txt").unwrap();
    assert!(ini.global.cluster == "mlx.hpc.uio.no");
    assert!(ini.global.role == "node");
    assert!(ini.sample.cadence == Some(Dur::Minutes(5)));
    assert!(ini.sample.batchless);
    assert!(!ini.sample.load);
    assert!(ini.sysinfo.cadence == Some(Dur::Hours(24)));
    assert!(ini.jobs.cadence == Some(Dur::Hours(1)));
    assert!(ini.jobs.window == Some(Dur::Minutes(90)));
    // TODO: Test cluster
}
