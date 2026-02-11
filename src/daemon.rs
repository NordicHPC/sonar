#![allow(clippy::comparison_to_empty)]
#![allow(clippy::len_zero)]

// TODO in this file, all marked:
//
// Low pri
//  - lock file (mostly obsolete with systemd controlling things)
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
use crate::datasink::delay::DelaySink;
use crate::datasink::directory::DirectorySink;
#[cfg(feature = "kafka")]
use crate::datasink::kafka::KafkaSink;
use crate::datasink::stdio::StdioSink;
use crate::datasink::DataSink;
use crate::jobsapi;
use crate::json_tags;
use crate::linux;
use crate::ps;
use crate::slurmjobs;
use crate::sysinfo;
use crate::systemapi::SystemAPI;
use crate::time::{unix_now, unix_time_components};

use std::io::BufRead;
use std::thread;
use std::time::Duration;

use crossbeam::{channel, select};
use signal_hook::consts::signal;
use signal_hook::iterator::Signals;

pub struct GlobalIni {
    pub cluster: String,
    pub role: String,
    pub master_if: String,
    pub lockdir: Option<String>,
    pub topic_prefix: Option<String>,
}

#[cfg(feature = "kafka")]
pub struct KafkaIni {
    pub broker_address: String,
    pub rest_endpoint: String,
    pub http_proxy: String,
    pub sending_window: Dur,
    pub timeout: Dur,
    pub ca_file: Option<String>,
    pub sasl_password: Option<String>,
    pub sasl_password_file: Option<String>,
}

pub struct DebugIni {
    pub verbose: bool,
    pub time_limit: Option<Dur>,
    pub output_delay: Option<Dur>,
    pub oneshot: bool,
}

pub struct DirectoryIni {
    pub data_dir: Option<String>,
}

pub struct SampleIni {
    pub cadence: Option<Dur>,
    pub exclude_system_jobs: bool,
    pub load: bool,
    pub batchless: bool,
    pub rollup: bool,
    pub min_cpu_time: Option<Dur>,
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
    pub batch_size: Option<usize>,
}

pub struct ClusterIni {
    pub on_startup: bool,
    pub cadence: Option<Dur>,
    pub domain: Option<Vec<String>>,
}

pub struct ProgramsIni {
    pub curl_cmd: Option<String>,
    pub sacct_cmd: Option<String>,
    pub sinfo_cmd: Option<String>,
    pub scontrol_cmd: Option<String>,
    pub topo_svg_cmd: Option<String>,
    pub topo_text_cmd: Option<String>,
}

pub struct Ini {
    pub global: GlobalIni,
    #[cfg(feature = "kafka")]
    pub kafka: KafkaIni,
    pub directory: DirectoryIni,
    pub programs: ProgramsIni,
    pub debug: DebugIni,
    pub sample: SampleIni,
    pub sysinfo: SysinfoIni,
    pub jobs: JobsIni,
    pub cluster: ClusterIni,
}

// Cloneable because a repeated_event operation will clone it.
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
            Dur::Seconds(n) => n.div_ceil(60),
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
// It is crucial that the system does not terminate suddenly once the lock file is held: the daemon
// mode must unwind properly and do what it can to relinquish the lock.
//
// Note that installing the logger is delegated to the daemon mode (in order to get the log level
// from the config file) and the daemon mode is required to install the logger before returning,
// even if there are errors in parsing the config file.  Higher levels will depend on the logger
// having been installed in order to signal any errors the daemon mode returns.

pub fn daemon_mode(
    config_file: &str,
    mut system: linux::system::Builder,
    force_slurm: bool,
) -> Result<(), String> {
    #[allow(unused_mut)]
    let mut ini = match parse_config(config_file) {
        Ok(ini) => {
            simple_logger::SimpleLogger::new()
                .with_level(if ini.debug.verbose {
                    log::LevelFilter::Debug
                } else {
                    log::LevelFilter::Warn
                })
                .env()
                .init()
                .unwrap();
            ini
        }
        Err(e) => {
            simple_logger::SimpleLogger::new()
                .with_level(log::LevelFilter::Warn)
                .env()
                .init()
                .unwrap();
            return Err(e);
        }
    };

    #[cfg(feature = "kafka")]
    if let Some(ref pwfile) = ini.kafka.sasl_password_file {
        match std::fs::read_to_string(pwfile) {
            Ok(s) => {
                ini.kafka.sasl_password = Some(s.trim().to_string());
            }
            Err(e) => return Err(format!("Failed to read password file: {e}")),
        }
    }

    if ini.sample.cadence.is_some() {
        system = system.with_jobmanager(Box::new(jobsapi::AnyJobManager::new(force_slurm)));
    }

    if let Some(ref p) = ini.cluster.domain {
        system = system.with_node_domain(p);
    }

    if let Some(ref p) = ini.programs.topo_svg_cmd {
        system = system.with_topo_svg_cmd(p);
    }
    if let Some(ref p) = ini.programs.topo_text_cmd {
        system = system.with_topo_text_cmd(p);
    }
    if let Some(ref p) = ini.programs.sacct_cmd {
        system = system.with_sacct_cmd(p);
    }
    if let Some(ref p) = ini.programs.sinfo_cmd {
        system = system.with_sinfo_cmd(p);
    }
    if let Some(ref p) = ini.programs.scontrol_cmd {
        system = system.with_scontrol_cmd(p);
    }

    let system = system.with_cluster(&ini.global.cluster).freeze()?;
    let hostname = system.get_hostname();
    let api_token = "".to_string();

    // For communicating with the main thread.  event_sender is used to send timer ticks (from timer
    // threads), incoming messages (from the data sink), signals (from the signal handling thread),
    // and errors (from anywhere); event_receiver receives those events in the main thread.
    let (event_sender, event_receiver) = channel::unbounded();

    let signal_handlers = {
        let mut signals = Signals::new([signal::SIGINT, signal::SIGTERM, signal::SIGHUP])
            .map_err(|_| "Signal handling setup".to_string())?;
        let signal_handlers = signals.handle();
        let sender = event_sender.clone();
        thread::spawn(move || {
            for signal in signals.forever() {
                let _ = sender.send(Operation::Signal(signal));
            }
        });
        signal_handlers
    };

    if ini.global.lockdir.is_some() {
        // TODO: Acquire lockdir here
        todo!();
    }

    // If sysinfo runs on startup then post a message to ourselves.  Should never fail.
    if ini.sysinfo.cadence.is_some() && ini.sysinfo.on_startup {
        let _ignored = event_sender.send(Operation::Sysinfo);
    }

    // Ditto cluster.
    if ini.cluster.cadence.is_some() && ini.cluster.on_startup {
        let _ignored = event_sender.send(Operation::Cluster);
    }

    let is_master = ini.global.role == "master"
        || ini.global.role == "predicated"
            && master_predicate_match(&ini.global.master_if, &hostname);

    // Alarms for daemon operations - each alarm gets its own thread, wasteful but OK for the time
    // being.  These will post the given events at the given cadences.
    if !is_master {
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
    } else {
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
    }

    let client_id = ini.global.cluster.clone() + "/" + &hostname;

    let mut control_topic =
        ini.global.cluster.clone() + ".control." + if is_master { "master" } else { "node" };
    if let Some(ref prefix) = ini.global.topic_prefix {
        control_topic = prefix.clone() + "." + &control_topic;
    }

    let mut data_sink: Box<dyn DataSink> =
        if let Some(s) = try_make_kafka_sink(&ini, &event_sender, &client_id, &control_topic) {
            s
        } else if let Some(s) = try_make_directory_sink(&ini, &event_sender) {
            s
        } else {
            Box::new(StdioSink::new(client_id, control_topic, event_sender))
        };

    if let Some(delay) = ini.debug.output_delay {
        data_sink = Box::new(DelaySink::new(delay, data_sink));
    }

    log::debug!("Initialization succeeded");

    let mut sample_extractor = ps::State::new(
        &system,
        ps::PsOptions {
            rollup: ini.sample.rollup,
            min_cpu_time: ini.sample.min_cpu_time.map(|d| d.to_seconds()),
            exclude_system_jobs: ini.sample.exclude_system_jobs,
            load: ini.sample.load,
            exclude_users: ini.sample.exclude_users.clone(),
            exclude_commands: ini.sample.exclude_commands.clone(),
            lockdir: ini.global.lockdir.clone(),
            token: api_token.clone(),
            fmt: ps::Format::JSON,
            cpu_util: true,
            min_cpu_percent: None, // Nonmonotonic, considered obsolete
            min_mem_percent: None, // Nonmonotonic, considered obsolete
        },
    );

    let mut sysinfo_extractor = sysinfo::State::new(&system, api_token.clone());

    let mut cluster_extractor = cluster::State::new(&system, api_token.clone());

    let mut slurm_extractor = slurmjobs::State::new(
        ini.jobs.window.map(|c| c.to_minutes() as u32),
        ini.jobs.uncompleted,
        ini.jobs.batch_size,
        &system,
        api_token.clone(),
    );

    let cutoff = if let Some(limit) = ini.debug.time_limit {
        channel::after(Duration::from_secs(limit.to_seconds()))
    } else {
        channel::never()
    };
    let mut fatal_msg = "".to_string();
    'messageloop: loop {
        select! {
            recv(cutoff) -> _ => {
                log::debug!("Time limit reached");
                break 'messageloop;
            }
            recv(event_receiver) -> msg => match msg {
                Err(_) => {
                    // Nobody gets to close this channel, so panic on error
                    panic!("Event queue receive");
                }
                Ok(op) => {
                    system.update_time();
                    let output;
                    let data_tag: &'static str;
                    match op {
                        Operation::Sample => {
                            log::debug!("Sample");
                            output = sample_extractor.run();
                            data_tag = json_tags::DATA_TAG_SAMPLE;
                        }
                        Operation::Sysinfo => {
                            log::debug!("Sysinfo");
                            output = sysinfo_extractor.run();
                            data_tag = json_tags::DATA_TAG_SYSINFO;
                        }
                        Operation::Jobs => {
                            log::debug!("Jobs");
                            output = slurm_extractor.run();
                            data_tag = json_tags::DATA_TAG_JOBS;
                        }
                        Operation::Cluster => {
                            log::debug!("Cluster");
                            output = cluster_extractor.run();
                            data_tag = json_tags::DATA_TAG_CLUSTER;
                        }
                        Operation::Signal(s) => {
                            log::debug!("signal {s}");
                            match s {
                                signal::SIGINT | signal::SIGHUP | signal::SIGTERM => {
                                    log::debug!("Received signal {s}");
                                    break 'messageloop;
                                }
                                _ => {
                                    continue 'messageloop;
                                }
                            }
                        }
                        Operation::Incoming(key, value) => {
                            log::debug!("Incoming");
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
                            log::debug!("Fatal error: {msg}");
                            fatal_msg = msg;
                            break 'messageloop;
                        }
                        Operation::MessageDeliveryError(msg) => {
                            log::error!("{msg}");
                            continue 'messageloop;
                        }
                    }

                    for o in output {
                        let value = String::from_utf8_lossy(&o).to_string();
                        data_sink.post(
                            &system,
                            &ini.global.topic_prefix,
                            &ini.global.cluster,
                            data_tag,
                            &hostname,
                            value,
                        );
                    }
                    if ini.debug.oneshot {
                        break 'messageloop;
                    }
                }
            }
        }
    }

    data_sink.stop(&system);
    drop(data_sink);

    // Other threads will need to not panic when they are killed on shutdown, but otherwise there's
    // nothing special to be done here for the repeater threads or even the signal processing
    // thread.

    if ini.global.lockdir.is_some() {
        // TODO: Relinquish lockdir here
        todo!();
    }

    signal_handlers.close();

    if fatal_msg != "" {
        Err(fatal_msg)
    } else {
        Ok(())
    }
}

fn master_predicate_match(predicate: &str, hostname: &str) -> bool {
    let first = {
        if let Some((first, _)) = hostname.split_once('.') {
            first
        } else {
            hostname
        }
    };
    // The predicate has been tested for syntactic validity earlier so a '*' is always last
    if let Some((prefix, _)) = predicate.rsplit_once('*') {
        first.starts_with(prefix)
    } else {
        first == predicate
    }
}

// The master host: predicate is a string that matches either the first element of the host name
// exactly or a prefix of the first element of the host name, the latter is indicated by the last
// char of the predicate being '*'.  Other likely wildcard chars are illegal, also the host name
// element separator '.'.  An empty literal is illegal.  Other domains than host: are illegal.
fn decode_master_predicate(mut predicate: &str) -> Option<String> {
    if !predicate.starts_with("host:") {
        return None;
    }
    predicate = &predicate[5..];
    let l = predicate.len();
    let decode = {
        let mut it = predicate.char_indices();
        loop {
            match it.next() {
                Some((i, c)) => {
                    match c {
                        '?' | '[' | ']' | '.' => {
                            break 0
                        }
                        '*' => {
                            if i == l - 1 {
                                break 2
                            } else {
                                break 0
                            }
                        }
                        _ => {}
                    }
                }
                None => {
                    break 1
                }
            }
        }
    };
    match decode {
        0 => None,
        1 => if l == 0 { None } else { Some(predicate.to_string()) },
        _ => if l <= 1 { None } else { Some(predicate[..l-1].to_string()) }
    }
}

#[cfg(not(feature = "kafka"))]
fn try_make_kafka_sink(
    _: &Ini,
    _: &channel::Sender<Operation>,
    _: &str,
    _: &str,
) -> Option<Box<dyn DataSink>> {
    None
}

#[cfg(feature = "kafka")]
fn try_make_kafka_sink(
    ini: &Ini,
    event_sender: &channel::Sender<Operation>,
    client_id: &str,
    control_topic: &str,
) -> Option<Box<dyn DataSink>> {
    if ini.kafka.broker_address != "" || ini.kafka.rest_endpoint != "" {
        Some(Box::new(KafkaSink::new(
            ini,
            client_id.to_string(),
            control_topic.to_string(),
            event_sender.clone(),
        )))
    } else {
        None
    }
}

fn try_make_directory_sink(
    ini: &Ini,
    event_sender: &channel::Sender<Operation>,
) -> Option<Box<dyn DataSink>> {
    if let Some(ref data_dir) = ini.directory.data_dir {
        Some(Box::new(DirectorySink::new(data_dir, event_sender.clone())))
    } else {
        None
    }
}

///////////////////////////////////////////////////////////////////////////////////////////////////
//
// Alarms and cadences.

fn repeated_event(_whoami: &str, sender: channel::Sender<Operation>, op: Operation, cadence: Dur) {
    // We could maybe use crossbeam::channel::tick here, but apart from possibly reducing the number
    // of live threads it probably doesn't offer any advantages, as we'd still need to deal with the
    // initial delay and worry about clock drift.

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
            master_if: "".to_string(),
            lockdir: None,
            topic_prefix: None,
        },
        #[cfg(feature = "kafka")]
        kafka: KafkaIni {
            broker_address: "".to_string(),
            rest_endpoint: "".to_string(),
            http_proxy: "".to_string(),
            sending_window: Dur::Minutes(5),
            timeout: Dur::Minutes(30),
            ca_file: None,
            sasl_password: None,
            sasl_password_file: None,
        },
        directory: DirectoryIni { data_dir: None },
        debug: DebugIni {
            time_limit: None,
            output_delay: None,
            verbose: false,
            oneshot: false,
        },
        sample: SampleIni {
            cadence: None,
            exclude_system_jobs: true,
            load: true,
            batchless: false,
            rollup: false,
            min_cpu_time: None,
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
            batch_size: None,
        },
        programs: ProgramsIni {
            curl_cmd: None,
            sacct_cmd: None,
            sinfo_cmd: None,
            scontrol_cmd: None,
            topo_svg_cmd: None,
            topo_text_cmd: None,
        },
        cluster: ClusterIni {
            on_startup: true,
            cadence: None,
            domain: None,
        },
    };

    enum Section {
        None,
        Global,
        #[cfg(feature = "kafka")]
        Kafka,
        Directory,
        Debug,
        Programs,
        Sample,
        Sysinfo,
        Jobs,
        Cluster,
    }
    let mut curr_section = Section::None;
    #[cfg(feature = "kafka")]
    let mut have_kafka = false;
    #[cfg(feature = "kafka")]
    let mut have_kafka_broker = false;
    #[cfg(feature = "kafka")]
    let mut have_kafka_rest_endpoint = false;
    let mut have_directory = false;
    let mut have_prefix = false;
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
        if l == "[directory]" {
            curr_section = Section::Directory;
            continue;
        }
        if l == "[debug]" {
            curr_section = Section::Debug;
            continue;
        }
        if l == "[programs]" {
            curr_section = Section::Programs;
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
                    "node" | "master" | "predicated" => {
                        ini.global.role = value;
                    }
                    _ => {
                        return Err(format!(
                            "Invalid global.role value `{value}` - node, master, or predicated required"
                        ))
                    }
                },
                "master-if" => {
                    if let Some(p) = decode_master_predicate(&value) {
                        ini.global.master_if = p;
                    } else {
                        return Err("Invalid master predicate".to_string());
                    }
                }
                "lockdir" | "lock-directory" => {
                    ini.global.lockdir = Some(value);
                }
                "topic-prefix" => {
                    ini.global.topic_prefix = Some(value);
                    have_prefix = true;
                }
                _ => return Err(format!("Invalid [global] setting name `{name}`")),
            },
            #[cfg(feature = "kafka")]
            Section::Kafka => match name.as_str() {
                "broker-address" | "remote-host" => {
                    ini.kafka.broker_address = value;
                    have_kafka_broker = true;
                }
                "rest-endpoint" => {
                    ini.kafka.rest_endpoint = value;
                    have_kafka_rest_endpoint = true;
                }
                "http-proxy" | "rest-proxy" => {
                    ini.kafka.http_proxy = value;
                }
                "sending-window" => {
                    ini.kafka.sending_window =
                        parse_duration("kafka.sending-window", &value, true)?;
                }
                "timeout" => {
                    ini.kafka.timeout = parse_duration("kafka.timeout", &value, true)?;
                }
                "ca-file" => {
                    ini.kafka.ca_file = Some(value);
                }
                "sasl-password" => {
                    ini.kafka.sasl_password = Some(value);
                }
                "sasl-password-file" => {
                    ini.kafka.sasl_password_file = Some(value);
                }
                _ => return Err(format!("Invalid [kafka] setting name `{name}`")),
            },
            Section::Directory => match name.as_str() {
                "data-directory" => {
                    ini.directory.data_dir = Some(value);
                    have_directory = true;
                }
                _ => return Err(format!("Invalid [directory] setting name `{name}`")),
            },
            Section::Debug => match name.as_str() {
                "time-limit" => {
                    ini.debug.time_limit = Some(parse_duration("debug.time-limit", &value, true)?);
                }
                "output-delay" => {
                    ini.debug.output_delay =
                        Some(parse_duration("debug.output-delay", &value, true)?);
                }
                "oneshot" => {
                    ini.debug.oneshot = parse_bool(&value)?;
                }
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
                "rollup" => {
                    ini.sample.rollup = parse_bool(&value)?;
                }
                "min-cpu-time" => {
                    ini.sample.min_cpu_time =
                        Some(parse_duration("sample.min-cpu-time", &value, true)?);
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
                "topo-svg-command" => {
                    ini.programs.topo_svg_cmd = Some(value);
                }
                "topo-text-command" => {
                    ini.programs.topo_text_cmd = Some(value);
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
                "batch-size" => {
                    ini.jobs.batch_size = Some(
                        value
                            .parse::<usize>()
                            .map_err(|_| "Bad jobs.batch-size".to_string())?,
                    );
                }
                _ => return Err(format!("Invalid [jobs] setting name `{name}`")),
            },
            Section::Cluster => match name.as_str() {
                "on-startup" => {
                    ini.cluster.on_startup = parse_bool(&value)?;
                }
                "cadence" => {
                    ini.cluster.cadence = Some(parse_duration("cluster.cadence", &value, false)?);
                }
                "domain" => {
                    let mut xs = value
                        .split(".")
                        .map(|x| x.to_string())
                        .collect::<Vec<String>>();
                    if xs.len() < 2 || xs[0] != "" || xs[1..].iter().any(|x| x == "") {
                        return Err(format!(
                            "Invalid global.domain value `{value}` - form .x.y.z required"
                        ));
                    }
                    // Drop initial, empty element
                    xs.rotate_left(1);
                    xs.pop();
                    ini.cluster.domain = Some(xs);
                }
                _ => return Err(format!("Invalid [cluster] setting name `{name}`")),
            },
            Section::Programs => match name.as_str() {
                "curl-command" => {
                    if value != "" {
                        check_path("curl-command", &value)?;
                    }
                    ini.programs.curl_cmd = Some(value);
                }
                "sacct-command" => {
                    if value != "" {
                        check_path("sacct-command", &value)?;
                    }
                    ini.programs.sacct_cmd = Some(value);
                }
                "scontrol-command" => {
                    if value != "" {
                        check_path("scontrol-command", &value)?;
                    }
                    ini.programs.scontrol_cmd = Some(value);
                }
                "sinfo-command" => {
                    if value != "" {
                        check_path("sinfo-command", &value)?;
                    }
                    ini.programs.sinfo_cmd = Some(value);
                }
                "topo-svg-command" => {
                    ini.programs.topo_svg_cmd = Some(value);
                }
                "topo-text-command" => {
                    ini.programs.topo_text_cmd = Some(value);
                }
                _ => return Err(format!("Invalid [programs] setting name `{name}`")),
            },
        }
    }

    if ini.global.cluster == "" {
        return Err("Missing global.cluster setting".to_string());
    }
    if ini.global.role == "" {
        return Err("Missing global.role setting".to_string());
    }
    if ini.global.role == "predicated" {
        if ini.global.master_if == "" {
            return Err("Missing global.master-if setting".to_string());
        }
    } else {
        if ini.global.master_if != "" {
            return Err("global.master-if setting not valid for this role".to_string());
        }
    }

    let mut sinks = 0;
    if have_directory {
        sinks += 1
    }
    #[cfg(feature = "kafka")]
    if have_kafka {
        sinks += 1
    }
    if sinks > 1 {
        return Err("More than one data sink configured".to_string());
    }

    #[cfg(feature = "kafka")]
    if have_kafka {
        if have_kafka_broker && have_kafka_rest_endpoint {
            return Err("Can't have both kafka.broker-address and kafka.rest-endpoint".to_string());
        }
        if !have_kafka_broker && !have_kafka_rest_endpoint {
            return Err("Missing kafka.broker-address or kafka.rest-endpoint setting".to_string());
        }
        if ini.kafka.sasl_password.is_some() || ini.kafka.sasl_password_file.is_some() {
            if ini.kafka.sasl_password.is_some() && ini.kafka.sasl_password_file.is_some() {
                return Err(
                    "kafka.sasl-password and kafka.sasl-password-file are mutually exclusive"
                        .to_string(),
                );
            }
            // We assume the REST endpoint is https or on localhost, but for raw Kafka it better be
            // a TLS connection if we carry credentials.
            if !have_kafka_rest_endpoint {
                if ini.kafka.ca_file.is_none()
                    || (ini.kafka.sasl_password.is_some()
                        && ini.kafka.sasl_password.as_ref().unwrap() != ""
                        && ini.kafka.ca_file.as_ref().unwrap() == "")
                    || (ini.kafka.sasl_password_file.is_some()
                        && ini.kafka.sasl_password_file.as_ref().unwrap() != ""
                        && ini.kafka.ca_file.as_ref().unwrap() == "")
                {
                    return Err(
                        "kafka.sasl-password and kafka.sasl-password-file require kafka.ca-file"
                            .to_string(),
                    );
                }
            }
        }
    }

    if have_directory && have_prefix {
        return Err("Data directory does not allow a topic prefix".to_string());
    }

    Ok(ini)
}

fn check_path(context: &str, value: &str) -> Result<(), String> {
    // Must be absolute path without .. elements or spaces (literally spaces, other whitespace is
    // allowed for now).
    if !value.starts_with('/') {
        return Err(format!("{context} must be an absolute path"));
    }
    if value.contains("/../") {
        return Err(format!("{context} must not contain .. elements"));
    }
    if value.contains(' ') {
        return Err(format!("{context} must not contain spaces"));
    }
    Ok(())
}

#[test]
pub fn test_check_path() {
    assert!(check_path("", "/a/b/c").is_ok());
    assert!(check_path("", "a/b/c").is_err());
    assert!(check_path("", "/a/b/../c").is_err());
    assert!(check_path("", "/a/b /c").is_err());
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
    assert!(ini.global.lockdir == Some("/root".to_string()));
    assert!(ini.global.topic_prefix == Some("zappa".to_string()));

    assert!(ini.kafka.broker_address == "localhost:0000".to_string());
    assert!(ini.kafka.sending_window == Dur::Hours(1));
    assert!(ini.kafka.timeout == Dur::Minutes(1));
    assert!(ini.kafka.ca_file == Some("test-ca".to_string()));
    assert!(ini.kafka.sasl_password_file == Some("test-pass".to_string()));

    assert!(ini.debug.time_limit == Some(Dur::Minutes(10)));
    assert!(ini.debug.output_delay == Some(Dur::Minutes(1)));
    assert!(ini.debug.verbose == true);

    assert!(ini.sample.cadence == Some(Dur::Minutes(5)));
    assert!(ini.sample.batchless);
    assert!(!ini.sample.load);
    assert!(ini.sample.rollup);
    assert!(ini.sample.min_cpu_time == Some(Dur::Minutes(1)));
    assert!(!ini.sample.exclude_system_jobs);
    let xusers = vec!["bob", "alice"];
    assert!(ini.sample.exclude_users.len() == xusers.len());
    for i in 0..xusers.len() {
        assert!(&ini.sample.exclude_users[i] == xusers[i]);
    }
    let xcmds = vec!["ls", "runuser"];
    assert!(ini.sample.exclude_commands.len() == xcmds.len());
    for i in 0..xcmds.len() {
        assert!(&ini.sample.exclude_commands[i] == xcmds[i]);
    }

    // The topo commands come from the sysinfo section
    assert!(ini.programs.topo_svg_cmd == Some("hello".to_string()));
    assert!(ini.programs.topo_text_cmd == Some("goodbye".to_string()));
    assert!(ini.programs.sinfo_cmd == None);
    assert!(ini.programs.sacct_cmd == Some("/home/zappa/bin/sacct".to_string()));
    assert!(ini.programs.scontrol_cmd == Some("/home/zappa/bin/scontrol".to_string()));

    assert!(ini.sysinfo.cadence == Some(Dur::Hours(24)));
    assert!(!ini.sysinfo.on_startup);

    assert!(ini.jobs.cadence == Some(Dur::Hours(1)));
    assert!(ini.jobs.window == Some(Dur::Minutes(90)));
    assert!(ini.jobs.batch_size == Some(17));
    assert!(ini.jobs.uncompleted);

    assert!(ini.cluster.cadence == Some(Dur::Minutes(15)));
    assert!(!ini.cluster.on_startup);
    assert!(ini.cluster.domain == Some(vec!["fox".to_string(), "nux".to_string()]));

    let ini = parse_config("src/testdata/daemon-stdio-config2.txt").unwrap();

    assert!(ini.global.cluster == "mlx.hpc.uio.no");
    assert!(ini.global.role == "master");

    assert!(ini.kafka.broker_address == "localhost:0000".to_string());
    assert!(ini.kafka.ca_file == Some("myfile".to_string()));
    assert!(ini.kafka.sasl_password == Some("qumquat".to_string()));

    let ini = parse_config("src/testdata/daemon-stdio-config3.txt").unwrap();

    assert!(ini.global.cluster == "fox.educloud.no");
    assert!(ini.global.role == "master");

    assert!(ini.directory.data_dir == Some("/dev/null/your/data/here".to_string()));
}
