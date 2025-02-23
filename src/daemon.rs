// TODO in this file, all marked:
//
// Medium pri
//  - implementation of deluge filtering logic
//  - the alarm threads must be more careful about re-syncing with the proper points on the
//    clock to avoid drift
//  - api token handling
//
// Low pri
//  - lock file
//  - signal handling to deal properly with with lock file
//  - reload config under signal + remote control (exiting and restarting via systemd is
//    just fine for starters)
//  - maybe pause / restart remote control
//  - more flexible cadence computation
//  - more test cases for the cadence computation

// For documentation about configuration and message topics that are used, see ~/doc/HOWTO-KAFKA.md.

// THREADS AND I/O
//
// The main thread of the daemon listens on a channel from which it reads events: alarms (for work
// to do), signals (keyboard interrupts), and incoming messages (from the Kafka broker).
//
// Kafka runs on its own thread(s) and handles interactions with the Kafka broker.  The main thread
// sends outgoing messages to Kafka over a dedicated channel.  The code in this module has been
// abstracted away from the specific Kafka library.
//
// Signal handlers place signals in the daemon's channel as events.

use crate::cluster;
use crate::jobsapi;
use crate::json_tags;
#[cfg(feature = "kafka-kafka")]
use crate::kafka;
use crate::ps;
use crate::realsystem;
use crate::slurmjobs;
use crate::sysinfo;
use crate::systemapi::SystemAPI;
use crate::time::{unix_now, unix_time_components};

#[cfg(feature = "kafka-kafka")]
use std::fs;
use std::io::{BufRead, Write};
#[cfg(feature = "kafka-kafka")]
use std::path;
use std::sync::mpsc;
use std::thread;

pub struct GlobalIni {
    pub cluster: String,
    pub role: String,
    pub lockdir: Option<String>,
}

#[cfg(feature = "kafka-kafka")]
pub struct KafkaIni {
    pub broker_address: String,
    pub poll_interval: Dur,
    pub compression: Option<String>,
    pub api_token_file: Option<String>,
    pub password_file: Option<String>,
    pub cert_file: Option<String>,
    pub key_file: Option<String>,
    pub ca_file: Option<String>,
}

pub struct DebugIni {
    pub dump: bool,
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
    pub delta_coding: bool,
    pub dump: Option<String>,
}

pub struct ClusterIni {
    pub cadence: Option<Dur>,
}

pub struct Ini {
    pub global: GlobalIni,
    #[cfg(feature = "kafka-kafka")]
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

// Kafka library abstraction.
//
// The Kafka subsystem sends outgoing messages and reacts to incoming traffic.  Outgoing messages
// may be batched, the connection may go up and down, and so on.  Incoming traffic is forwarded to
// the main thread.  Outgoing traffic is received on a channel from the main thread.  The
// KafkaManager abstracts the details and hides the specific Kafka library we use.

pub trait KafkaManager<K, V> {
    fn init(
        &mut self,
        ini: &Ini,
        client_id: String,
        sender: mpsc::Sender<Operation>,
    ) -> Result<(), String>;
    fn post(&self, topic: String, key: K, value: V, sending_window: u64);
    fn stop(&self);
}

// The daemon_mode() should return under these circumstances *only*:
//
// - it fails to parse the ini file
// - it fails to read the api token or password files
// - it fails to read the tls files
// - it fails to acquire the lock file
// - it fails to setup an interrupt handler
// - an exit control message is received from the broker
// - a signal is received from the OS or user that signifies an exit condition
//
// There are some problems that are not handled well.  The credentials for tls are file names that
// are processed in the kafka threads, where the only recourse is currently to panic, which is
// unacceptable.  It would be better for those threads to post errors back to the main thread so
// that we can return from here.
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

    #[cfg(feature = "kafka-kafka")]
    let api_token = if let Some(filename) = &ini.kafka.api_token_file {
        match fs::read_to_string(path::Path::new(filename)) {
            Ok(s) => s,
            Err(_) => {
                return Err("Can't read api token file".to_string());
            }
        }
    } else {
        "".to_string()
    };

    #[cfg(not(feature = "kafka-kafka"))]
    let api_token = "".to_string();

    let ps_opts = ps::PsOptions {
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
    };

    if ini.global.lockdir.is_some() {
        // TODO: Acquire lockdir here
        // TODO: Set up interrupt handling
        todo!();
    }

    // For communicating with the daemon thread.
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

    #[cfg(feature = "kafka-kafka")]
    let mut kafka = kafka::new_kafka();

    #[cfg(not(feature = "kafka-kafka"))]
    let mut kafka = DataSink::new();

    // This can fail for a number of reasons, notably cert/key/passwd files may not be found or they
    // may not validate.
    kafka.init(
        &ini,
        ini.global.cluster.clone() + "/" + &hostname,
        event_sender,
    )?;

    if ini.debug.verbose {
        println!("Initialization succeeded");
    }

    let w = ini.jobs.window.map(|c| c.to_minutes() as u32);
    let mut slurmjobber = slurmjobs::Jobber::new(&w, &None, ini.jobs.uncompleted, ini.jobs.delta_coding, &system);

    let mut dump = ini.debug.dump; // For testing
    let mut fatal_msg = "".to_string();
    'messageloop: loop {
        let mut output = Vec::new();

        // Nobody gets to close this channel, so panic on error
        let op = event_receiver.recv().expect("Event queue receive");

        system.update_time();
        let topic: &'static str;
        let mut sending_window: u64 = 0;
        match op {
            Operation::Sample => {
                if ini.debug.verbose {
                    println!("Sample");
                }
                ps::create_snapshot(&mut output, &system, &ps_opts);
                topic = json_tags::DATA_TAG_SAMPLE;
                sending_window = ini.sample.cadence.unwrap().to_seconds();
            }
            Operation::Sysinfo => {
                if ini.debug.verbose {
                    println!("Sysinfo");
                }
                sysinfo::show_system(&mut output, &system, api_token.clone(), false, true);
                topic = json_tags::DATA_TAG_SYSINFO;
            }
            Operation::Jobs => {
                if ini.debug.verbose {
                    println!("Jobs");
                }
                slurmjobber.show_jobs(api_token.clone(), &mut output);
                topic = json_tags::DATA_TAG_JOBS;
            }
            Operation::Cluster => {
                if ini.debug.verbose {
                    println!("Cluster");
                }
                cluster::show_cluster(&mut output, api_token.clone(), &system);
                topic = json_tags::DATA_TAG_CLUSTER;
            }
            Operation::Signal(s) => {
                if ini.debug.verbose {
                    println!("signal {s}");
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
                    println!("Incoming");
                }
                // TODO: Maybe a reload function
                // TODO: Maybe pause / restart functions
                match (key.as_str(), value.as_str()) {
                    ("exit", _) => {
                        break 'messageloop;
                    }
                    ("dump", "true") => {
                        dump = true;
                    }
                    ("dump", "false") => {
                        dump = false;
                    }
                    _ => {}
                }
                continue 'messageloop;
            }
            Operation::Fatal(msg) => {
                if ini.debug.verbose {
                    println!("Fatal {msg}");
                }
                fatal_msg = msg;
                break 'messageloop;
            }
        }

        let topic = ini.global.cluster.clone() + "." + topic;
        let key = hostname.clone();
        let value = String::from_utf8_lossy(&output).to_string();

        if let Operation::Jobs = op {
            if let Some(ref filename) = ini.jobs.dump {
                dump_data(filename, &value, ini.debug.verbose);
            }
        }

        if dump {
            println!("DUMP\nTOPIC: {topic}\nKEY: {key}\n{value}");
        }

        kafka.post(topic, key, value, sending_window);
    }

    kafka.stop();

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

fn dump_data(filename: &str, value: &str, verbose: bool) {
    match std::fs::File::options().append(true).open(filename) {
        Ok(mut f) => {
            match writeln!(&mut f, "{value}") {
                Ok(_) => {}
                Err(_) => {
                    if verbose {
                        println!("Failed to dump data");
                    }
                }
            }
        }
        Err(_) => {
            if verbose {
                println!("Failed to dump data");
            }
        }
    }
}

///////////////////////////////////////////////////////////////////////////////////////////////////
//
// Alarms and cadences.

fn repeated_event(_whoami: &str, sender: mpsc::Sender<Operation>, op: Operation, cadence: Dur) {
    let now = unix_now();
    let next = time_at_next_cadence_point(now, cadence);
    let initial_delay = next as i64 - now as i64;
    if initial_delay > 0 {
        thread::sleep(std::time::Duration::from_secs(initial_delay as u64));
    }
    // TODO: There's a small risk of getting out of sync here, and we should recalibrate with the
    // cadence point so that we don't drift.  I think basically, we keep track of the first time we
    // run the alarm, and the number of times we have run it, and add a multiple of the cadence to
    // the first time, and then compute the delay for *this* iteration, and that may be (say) a
    // second short of the cadence, and that's fine.
    let delay = cadence.to_seconds();
    loop {
        // If the send fails then the main loop has closed the connection and this is our signal to
        // shut down the thread.
        if sender.send(op.clone()).is_err() {
            break;
        }
        thread::sleep(std::time::Duration::from_secs(delay));
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
        },
        #[cfg(feature = "kafka-kafka")]
        kafka: KafkaIni {
            broker_address: "".to_string(),
            poll_interval: Dur::Minutes(5),
            compression: None,
            api_token_file: None,
            password_file: None,
            cert_file: None,
            key_file: None,
            ca_file: None,
        },
        debug: DebugIni {
            dump: false,
            verbose: false,
        },
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
            delta_coding: true,
            dump: None,
        },
        cluster: ClusterIni { cadence: None },
    };

    enum Section {
        None,
        Global,
        #[cfg(feature = "kafka-kafka")]
        Kafka,
        Debug,
        Sample,
        Sysinfo,
        Jobs,
        Cluster,
    }
    let mut curr_section = Section::None;
    #[cfg(feature = "kafka-kafka")]
    let mut have_kafka = false;
    #[cfg(feature = "kafka-kafka")]
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
        #[cfg(feature = "kafka-kafka")]
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
                _ => return Err(format!("Invalid [global] setting name `{name}`")),
            },
            #[cfg(feature = "kafka-kafka")]
            Section::Kafka => match name.as_str() {
                "broker-address" | "remote-host" => {
                    ini.kafka.broker_address = value;
                    have_kafka_remote = true;
                }
                "poll-interval" => {
                    ini.kafka.poll_interval = parse_duration(&value, false)?;
                }
                "compression" => match value.as_str() {
                    "none" => {
                        ini.kafka.compression = None;
                    }
                    "gzip" | "snappy" => {
                        ini.kafka.compression = Some(value);
                    }
                    _ => return Err(format!("Invalid kafka.compression value `{value}`")),
                }
                "api-token-file" => {
                    ini.kafka.api_token_file = Some(value);
                }
                "password-file" => {
                    ini.kafka.password_file = Some(value);
                }
                "cert-file" => {
                    ini.kafka.cert_file = Some(value);
                }
                "key-file" => {
                    ini.kafka.key_file = Some(value);
                }
                "ca-file" => {
                    ini.kafka.ca_file = Some(value);
                }
                _ => return Err(format!("Invalid [kafka] setting name `{name}`")),
            },
            Section::Debug => match name.as_str() {
                "dump" => {
                    ini.debug.dump = parse_bool(&value)?;
                }
                "verbose" => {
                    ini.debug.verbose = parse_bool(&value)?;
                }
                _ => return Err(format!("Invalid [debug] setting name `{name}`")),
            },
            Section::Sample => match name.as_str() {
                "cadence" => {
                    ini.sample.cadence = Some(parse_duration(&value, false)?);
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
                    ini.sysinfo.cadence = Some(parse_duration(&value, false)?);
                }
                _ => return Err(format!("Invalid [sysinfo] setting name `{name}`")),
            },
            Section::Jobs => match name.as_str() {
                "cadence" => {
                    let dur = parse_duration(&value, false)?;
                    ini.jobs.cadence = Some(dur);
                    if ini.jobs.window.is_none() {
                        ini.jobs.window = Some(Dur::Seconds(2 * dur.to_seconds()));
                    }
                }
                "window" => {
                    ini.jobs.window = Some(parse_duration(&value, true)?);
                }
                "uncompleted" | "incomplete" => {
                    ini.jobs.uncompleted = parse_bool(&value)?;
                }
                "delta-coding" => {
                    ini.jobs.delta_coding = parse_bool(&value)?;
                }
                "dump" => {
                    ini.jobs.dump = Some(value);
                }
                _ => return Err(format!("Invalid [jobs] setting name `{name}`")),
            },
            Section::Cluster => match name.as_str() {
                "cadence" => {
                    ini.cluster.cadence = Some(parse_duration(&value, false)?);
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

    #[cfg(feature = "kafka-kafka")]
    if have_kafka {
        if !have_kafka_remote {
            return Err("Missing kafka.remote-host setting".to_string());
        }
        if ini.kafka.cert_file.is_some() != ini.kafka.key_file.is_some()
            && ini.kafka.cert_file.is_some() != ini.kafka.ca_file.is_some()
        {
            return Err(
                "Define all or none of kafka.cert-file, kafka.key-file, kafka.ca-file".to_string(),
            );
        }
        if ini.kafka.password_file.is_some() && ini.kafka.cert_file.is_none() {
            return Err("Password file without TLS".to_string());
        }
        if ini.kafka.api_token_file.is_some() && ini.kafka.cert_file.is_none()  {
            return Err("Token file without TLS".to_string());
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

fn parse_duration(l: &str, lenient: bool) -> Result<Dur, String> {
    if let Some(hours) = l.strip_suffix(['h', 'H']) {
        if let Ok(k) = hours.parse::<u64>() {
            if k > 0 && (lenient || 24 % k == 0 || k % 24 == 0) {
                return Ok(Dur::Hours(k));
            }
        }
        return Err("Bad duration".to_string());
    }
    if let Some(minutes) = l.strip_suffix(['m', 'M']) {
        if let Ok(k) = minutes.parse::<u64>() {
            if k > 0 && (lenient || (k < 60 && 60 % k == 0)) {
                return Ok(Dur::Minutes(k));
            }
        }
        return Err("Bad duration".to_string());
    }
    if let Some(seconds) = l.strip_suffix(['s', 'S']) {
        if let Ok(k) = seconds.parse::<u64>() {
            if k > 0 && (lenient || (k < 60 && 60 % k == 0)) {
                return Ok(Dur::Seconds(k));
            }
        }
    }
    Err("Bad duration".to_string())
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
    assert!(parse_duration("30s", true).unwrap() == Dur::Seconds(30));
    assert!(parse_duration("10m", true).unwrap() == Dur::Minutes(10));
    assert!(parse_duration("6H", true).unwrap() == Dur::Hours(6));

    assert!(parse_setting("zappa").is_err());
    assert!(parse_setting("zappa = ").is_err());
    assert!(parse_setting("zappa = `abracadabra").is_err());
    assert!(parse_setting("zapp! = true").is_err());
    assert!(parse_bool("tru").is_err());
    assert!(parse_duration("35", true).is_err());
    assert!(parse_duration("12m35s", true).is_err());
    assert!(parse_duration("3H12M35X", true).is_err());

    let ini = parse_config("src/testdata/daemon-config.txt").unwrap();
    assert!(ini.global.cluster == "mlx.hpc.uio.no");
    assert!(ini.global.role == "node");
    assert!(ini.kafka.broker_address == "naic-monitor.uio.no:12345");
    assert!(ini.sample.cadence == Some(Dur::Minutes(5)));
    assert!(ini.sample.batchless);
    assert!(!ini.sample.load);
    assert!(ini.sysinfo.cadence == Some(Dur::Hours(24)));
    assert!(ini.jobs.cadence == Some(Dur::Hours(1)));
    assert!(ini.jobs.window == Some(Dur::Minutes(90)));
    // TODO: Test cluster
}

#[cfg(not(feature = "kafka-kafka"))]
struct DataSink<K, V> {
    _outgoing: Option<mpsc::Sender<(String, K, V)>>,
}

#[cfg(not(feature = "kafka-kafka"))]
impl<
        K: std::marker::Send + 'static,
        V: std::marker::Send + 'static,
    > DataSink<K, V>
{
    fn new() -> DataSink<K, V> {
        DataSink { _outgoing: None }
    }
}

#[cfg(not(feature = "kafka-kafka"))]
impl<
        K: std::marker::Send + 'static,
        V: std::marker::Send + 'static,
    > KafkaManager<K, V> for DataSink<K, V>
{
    fn init(
        &mut self,
        _ini: &Ini,
        _client_id: String,
        _sender: mpsc::Sender<Operation>,
    ) -> Result<(), String> {
        self._outgoing = None;
        Ok(())
    }

    fn post(&self, _topic: String, _key: K, _value: V, _sending_window: u64) {
        // Nothing
    }

    fn stop(&self) {
        // Nothing
    }
}
