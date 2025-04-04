// TODO in this file, all marked:
//
//  - implementation of deluge filtering logic (medium pri)
//  - the alarm threads should be more careful about re-syncing with the proper points on the
//    clock to avoid drift (medium pri, postprocessing will want this)
//  - lock file (low pri)
//  - signal handling to deal properly with with lock file (low pri)
//  - reload config under signal + remote control (low pri, exiting and restarting via systemd is
//    just fine for now)
//  - maybe pause / restart remote control (low pri)
//  - more flexible cadence computation (low pri)
//  - more test cases for the cadence computation (low pri)

// In the "daemon mode", Sonar stays memory-resident and pushes data to a network sink.  In this
// mode, the only command line parameter is the name of a config file.
//
// The daemon is a multi-threaded system that performs system sampling, communicates with a Kafka
// broker, and handles signals and lock files.
//
//
// CONFIG FILE.
//
// The config file is an ini-type file.  Blank lines and lines starting with '#' are ignored.  Each
// section has a [section-name] header on a line by itself.  Within the sections, there are
// name=value pairs where names are simple identifiers matching /[a-zA-Z_][-a-zA-Z_0-9]*/ and values
// may be quoted with ', ", or `; these quotes are stripped.  Blanks before and after names and
// values are stripped.
//
// bool values are true or false.  Duration values express a time value using the syntax __h, __m,
// or __s, denoting hour, minute, or second values (uppercase HMS also allowed).  Values must be
// nonzero. For cadences, second values must divide a minute evently and be < 60, minute values must
// divide an hour evenly and < 60, and hour values must divide a day evenly or be a positive
// multiple of 24.  (Some sensible cadences such as 90m aka 1h30m are not currently expressible.)
//
// The config file has [global] and [debug] sections that control general operation; a section for
// the transport type chosen, currently only [kafka]; and a section each for the sonar operations,
// controlling their cadence and operation in the same way as normal command line switches.  For the
// Sonar operations, the cadence setting is required for the operation to be run, the command will
// be run at a time that is zero mod the cadence.
//
// [global] section:
//
//   cluster = <canonical cluster name>
//   role = node | master
//   lockdir = <string>                              # default none
//
//   The cluster name is required, eg fox.educloud.no.
//
//   The role determines how this daemon responds to control messages from a remote controller.  It
//   must be defined.  Only the string values listed are accepted.  A `node` typically provides
//   sample and sysinfo data only, a `master` often only slurm and cluster data.
//
//   If there is a lockdir then a lockfile is acquired when the daemon runs for the daemon's
//   lifetime, though if the daemon is reloaded by remote command the lock is relinquished
//   temporarily (and the restarted config file may name a different lockdir).
//
// [kafka] section (preliminary):
//
//   remote-host = <hostname and port>
//   poll-interval = <duration value>                # default 5m
//   cert-file = <path>
//   key-file = <path>
//   ca-file = <path>
//   password-file = <path>
//
//   The remote-host is required.  For Kafka it's usually host:port, eg localhost:9092 for a local
//   broker on the standard port.
//
//   cert-file, key-file and ca-file have to be used together and if present will force a TLS
//   connection.  password-file is the password for the user identity on the form user:password, and
//   if present will be used for authentication, in which case the username will be the cluster
//   name.  Authentication can be used without TLS, though it's rarely a good idea.
//
// [sample] section aka [ps] section:
//
//   cadence = <duration value>
//   exclude-system-jobs = <bool>                    # default true
//   load = <bool>                                   # default true
//   batchless = <bool>                              # default false
//   exclude-users = <comma-separated strings>       # default []
//   exclude-commands = <comma-separated strings>    # default []
//
// [sysinfo] section:
//
//   cadence = <duration value>
//   on-startup = <bool>                             # default true
//
//   If on-startup is true then a sysinfo operation will be executed every time the daemon is
//   started.
//
// [jobs] section aka [slurm] section:
//
//   cadence = <duration value>
//   window = <duration value>                       # default 2*cadence
//   incomplete = <bool>                             # default false
//
//   The window is the sacct time window used for looking for data.
//
//   The `incomplete` option triggers the inclusion of data about pending and running jobs.
//   Transmission of these data may or may not be optimized - redundant data may be omitted in
//   subsequent transmissions (for example, if a PENDING record has the same contents as one already
//   sent because no settings have changed, then the second record may not be sent at all).  What is
//   deemed redundant is up for discussion for both PENDING and RUNNING jobs.
//
// [cluster] section:
//
//   cadence = <duration value>
//
// [debug] section:
//
//   dump = bool                                     # default false
//   verbose = bool                                  # default false
//
// Data messages: These are sent under topics <cluster>.<data-type> where cluster is as configured
// in the [global] section and data-type is `sample`, `sysinfo`, and `jobs`.  The payload is always
// JSON.
//
// Control messages: These are sent under topics <cluster>.<role> where cluster is as configured
// in the [global] section and role is `node` or `master`.  These will have key and value as follows:
//
//   Key     Value      Meaning
//   ------- ---------- -------------------------------------------
//   exit    (none)     Terminate sonar immediately
//   dump    <boolean>  Enable or disable data dump (for debugging)
//
// Example compute-node file:
//
//  [global]
//  cluster = mlx.hpc.uio.no
//  role = node
//
//  [debug]
//  verbose = true
//
//  [kafka]
//  remote-host = naic-monitor.uio.no:12345
//
//  [sample]
//  cadence = 5m
//  batchless = true
//
//  [sysinfo]
//  cadence = 24h
//
//
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
#[cfg(feature = "kafka-kafka")]
use crate::kafka;
use crate::jobsapi;
use crate::ps;
#[cfg(feature = "kafka-rdkafka")]
use crate::rdkafka;
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
}

pub struct KafkaIni {
    pub remote_host: String,
    pub poll_interval: Dur,
    pub cert_file: Option<String>,
    pub key_file: Option<String>,
    pub ca_file: Option<String>,
    pub password_file: Option<String>,
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
    pub incomplete: bool,
}

pub struct ClusterIni {
    pub cadence: Option<Dur>,
}

struct Ini {
    global: GlobalIni,
    kafka: KafkaIni,
    debug: DebugIni,
    sample: SampleIni,
    sysinfo: SysinfoIni,
    jobs: JobsIni,
    cluster: ClusterIni,
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

impl Dur {
    pub fn to_seconds(&self) -> u64 {
        match self {
            Dur::Hours(n) => *n * 60 * 60,
            Dur::Minutes(n) => *n * 60,
            Dur::Seconds(n) => *n,
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
        global: &GlobalIni,
        kafka: &KafkaIni,
        debug: &DebugIni,
        client_id: String,
        sender: mpsc::Sender<Operation>,
    ) -> Result<(), String>;
    fn post(&self, topic: String, key: K, value: V, sending_window: u64);
    fn stop(&self);
}

// The daemon_mode() should return under these circumstances *only*:
//
// - it fails to parse the ini file
// - it fails to acquire the lock file
// - it fails to setup an interrupt handler
// - an exit control message is received from the broker
// - a signal is received from the OS or user that signifies an exit condition
//
// There are some problems that are not handled well.  The credentials for ssl are file names that
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

    let system = system.freeze()?;
    let hostname = system.get_hostname();

    let ps_opts = ps::PsOptions {
        rollup: false,
        min_cpu_percent: None,
        min_mem_percent: None,
        min_cpu_time: None,
        exclude_system_jobs: ini.sample.exclude_system_jobs,
        load: ini.sample.load,
        exclude_users: ini.sample.exclude_users,
        exclude_commands: ini.sample.exclude_commands,
        lockdir: ini.global.lockdir.clone(),
        new_json: true,
        cpu_util: true,
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
            repeated_event("sysinfo", sender, Operation::Sysinfo, c);
        });
    }
    if let Some(c) = ini.sample.cadence {
        let sender = event_sender.clone();
        thread::spawn(move || {
            repeated_event("sample", sender, Operation::Sample, c);
        });
    }
    if let Some(c) = ini.jobs.cadence {
        let sender = event_sender.clone();
        thread::spawn(move || {
            repeated_event("slurm", sender, Operation::Jobs, c);
        });
    }
    if let Some(c) = ini.cluster.cadence {
        let sender = event_sender.clone();
        thread::spawn(move || {
            repeated_event("cluster", sender, Operation::Cluster, c);
        });
    }

    #[cfg(feature = "kafka-kafka")]
    let mut kafka = kafka::new_kafka();

    #[cfg(feature = "kafka-rdkafka")]
    let mut kafka = rdkafka::new_kafka();

    // This can fail for a number of reasons, notably cert/key/passwd files may not be found or they
    // may not validate.
    kafka.init(
        &ini.global,
        &ini.kafka,
        &ini.debug,
        ini.global.cluster.clone() + "/" + &hostname,
        event_sender,
    )?;

    if ini.debug.verbose {
        println!("Initialization succeeded");
    }

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
                topic = "sample";
                sending_window = ini.sample.cadence.unwrap().to_seconds();
            }
            Operation::Sysinfo => {
                if ini.debug.verbose {
                    println!("Sysinfo");
                }
                sysinfo::show_system(&mut output, &system, false, true);
                topic = "sysinfo";
            }
            Operation::Jobs => {
                if ini.debug.verbose {
                    println!("Jobs");
                }
                let w = if let Some(c) = ini.jobs.window {
                    Some(c.to_seconds() as u32)
                } else {
                    None
                };
                // FIXME: deluge / incomplete
                //
                // When set, we want to include PENDING / RUNNING in the set of states *but* we want
                // to filter the output so that redundant information is not sent.  Probably there is
                // a filter object or filter function passed here to use in that case, and the flag
                // goes from false/true to None/Some(filter).
                slurmjobs::show_slurm_jobs(&mut output, &w, &None, false, &system, true);
                // FIXME: I may have changed this, need to check
                topic = "jobs";
            }
            Operation::Cluster => {
                if ini.debug.verbose {
                    println!("Cluster");
                }
                cluster::show_cluster(&mut output, &system);
                topic = "cluster";
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
        unix_time_components(time_at_next_cadence_point(now+15, Dur::Seconds(15)));
    assert!(hour == 11);
    assert!(minute == 16);
    assert!(second == 45);

    let (_year, _month, _day, hour, minute, second) =
        unix_time_components(time_at_next_cadence_point(now+30, Dur::Seconds(15)));
    assert!(hour == 11);
    assert!(minute == 17);
    assert!(second == 00);

    let (_year, _month, _day, hour, minute, second) =
        unix_time_components(time_at_next_cadence_point(now+45, Dur::Seconds(15)));
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
        unix_time_components(time_at_next_cadence_point(now+31, Dur::Seconds(2)));
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
        kafka: KafkaIni {
            remote_host: "".to_string(),
            poll_interval: Dur::Minutes(5),
            cert_file: None,
            key_file: None,
            ca_file: None,
            password_file: None,
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
            incomplete: false,
        },
        cluster: ClusterIni {
            cadence: None,
        }
    };

    enum Section {
        None,
        Global,
        Kafka,
        Debug,
        Sample,
        Sysinfo,
        Jobs,
        Cluster,
    }
    let mut curr_section = Section::None;
    let mut have_kafka = false;
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
            Section::Kafka => match name.as_str() {
                "remote-host" => {
                    ini.kafka.remote_host = value;
                    have_kafka_remote = true;
                }
                "poll-interval" => {
                    ini.kafka.poll_interval = parse_duration(&value, false)?;
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
                "password-file" => {
                    ini.kafka.password_file = Some(value);
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
                "incomplete" => {
                    ini.jobs.incomplete = parse_bool(&value)?;
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
        return Err("Illegal property definition".to_string());
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
    if let Some(hours) = l.strip_suffix(&['h', 'H']) {
        if let Ok(k) = hours.parse::<u64>() {
            if k > 0 && (lenient || 24 % k == 0 || k % 24 == 0) {
                return Ok(Dur::Hours(k));
            }
        }
        return Err("Bad duration".to_string());
    }
    if let Some(minutes) = l.strip_suffix(&['m', 'M']) {
        if let Ok(k) = minutes.parse::<u64>() {
            if k > 0 && (lenient || (k < 60 && 60 % k == 0)) {
                return Ok(Dur::Minutes(k));
            }
        }
        return Err("Bad duration".to_string());
    }
    if let Some(seconds) = l.strip_suffix(&['s', 'S']) {
        if let Ok(k) = seconds.parse::<u64>() {
            if k > 0 && (lenient || (k < 60 && 60 % k == 0)) {
                return Ok(Dur::Seconds(k));
            }
        }
        return Err("Bad duration".to_string());
    }
    return Err("Bad duration".to_string());
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
    return std::str::from_utf8(&bs[first..limit]).unwrap();
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
    assert!(ini.kafka.remote_host == "naic-monitor.uio.no:12345");
    assert!(ini.sample.cadence == Some(Dur::Minutes(5)));
    assert!(ini.sample.batchless);
    assert!(!ini.sample.load);
    assert!(ini.sysinfo.cadence == Some(Dur::Hours(24)));
    assert!(ini.jobs.cadence == Some(Dur::Hours(1)));
    assert!(ini.jobs.window == Some(Dur::Minutes(90)));
    // TODO: Test cluster
}
