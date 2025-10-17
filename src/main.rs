mod cluster;
mod command;
#[cfg(feature = "daemon")]
mod daemon;
#[cfg(feature = "daemon")]
mod datasink;
mod gpu;
mod hostname;
mod jobsapi;
mod json_tags;
mod linux;
mod log;
//#[cfg(test)]
//mod mockjobs;
mod nodelist;
mod output;
#[cfg(test)]
mod output_test;
mod ps;
mod ps_newfmt;
mod ps_oldfmt;
#[cfg(test)]
mod ps_test;
mod slurmjobs;
mod sysinfo;
#[cfg(test)]
mod sysinfo_test;
mod systemapi;
mod time;
mod types;
mod users;
mod util;

use std::io;

const USAGE_ERROR: i32 = 2; // clap, Python, Go

const OUTPUT_FORMAT: u64 = 0;

enum Commands {
    /// Enter daemon mode.
    #[cfg(feature = "daemon")]
    Daemon {
        config_file: String,
    },
    /// Take a snapshot of the currently running processes
    PS {
        /// Merge process records that have the same job ID and command name
        rollup: bool,

        /// Include records for jobs that have on average used at least this percentage of CPU,
        /// note this is nonmonotonic [default: none]
        min_cpu_percent: Option<f64>,

        /// Include records for jobs that presently use at least this percentage of real memory,
        /// note this is nonmonotonic [default: none]
        min_mem_percent: Option<f64>,

        /// Include records for jobs that have used at least this much CPU time (in seconds)
        /// [default: none]
        min_cpu_time: Option<usize>,

        /// Exclude records for system jobs (uid < 1000)
        exclude_system_jobs: bool,

        /// Exclude records whose users match these comma-separated names [default: none]
        exclude_users: Option<String>,

        /// Exclude records whose commands start with these comma-separated names [default: none]
        exclude_commands: Option<String>,

        /// Create a per-host lockfile in this directory and exit early if the file exists on
        /// startup [default: none]
        lockdir: Option<String>,

        /// One output record per Sonar invocation will contain a load= field with an encoding of
        /// the per-cpu usage since boot.
        load: bool,

        /// Output old CSV, not new JSON
        csv: bool,

        /// Cluster name
        cluster: Option<String>,
    },
    /// Extract cluster information
    Cluster {
        /// Cluster name
        cluster: Option<String>,
    },
    /// Extract node information
    Sysinfo {
        /// Output CSV, not old JSON
        csv: bool,

        /// Output old JSON, not new JSON
        oldjson: bool,

        /// Cluster name
        cluster: Option<String>,

        /// Command to execute for the topo_svg field
        topo_svg_cmd: Option<String>,

        /// Command to execute for the topo_text field
        topo_text_cmd: Option<String>,
    },
    /// Extract slurm job information
    Slurmjobs {
        /// Set the sacct start time to now-`window` and the end time to now, and dump records that
        /// are relevant for that interval.  Normally the running interval of `sonar slurm` should
        /// be less than the window.  Precludes -span.
        window: Option<u32>,

        /// From-to dates on the form yyyy-mm-dd,yyyy-mm-dd (with the comma); from is inclusive,
        /// to is exclusive.  Precludes -window.
        span: Option<String>,

        /// Output old CSV, not new JSON
        csv: bool,

        /// Include PENDING and RUNNING jobs
        deluge: bool,

        /// Cluster name
        cluster: Option<String>,
    },
    Version {},
}

fn main() {
    log::init();

    let mut stdout = io::stdout();
    let writer: &mut dyn io::Write = &mut stdout;
    let system = linux::system::Builder::new();
    let token = "".to_string(); // API token, to be implemented

    #[cfg(debug_assertions)]
    let force_slurm = std::env::var("SONARTEST_ROLLUP").is_ok();

    #[cfg(not(debug_assertions))]
    let force_slurm = false;

    match &command_line() {
        #[cfg(feature = "daemon")]
        Commands::Daemon { config_file } => {
            // This ignores `writer`, as the daemon manages its own I/O.
            //
            // The daemon returns early under specific conditions but once it's running it will only
            // return with an Ok return and only when told to exit by a remote command or a signal.
            match daemon::daemon_mode(config_file, system, force_slurm) {
                Ok(_) => {}
                Err(e) => {
                    log::error(&format!("Daemon returned with error: {e}"));
                }
            }
        }
        Commands::PS {
            rollup,
            min_cpu_percent,
            min_mem_percent,
            min_cpu_time,
            exclude_system_jobs,
            exclude_users,
            exclude_commands,
            lockdir,
            load,
            csv,
            cluster,
        } => {
            let opts = ps::PsOptions {
                rollup: *rollup,
                min_cpu_percent: *min_cpu_percent,
                min_mem_percent: *min_mem_percent,
                min_cpu_time: *min_cpu_time,
                exclude_system_jobs: *exclude_system_jobs,
                load: *load,
                exclude_users: if let Some(s) = exclude_users {
                    s.split(',').map(|x| x.to_string()).collect::<Vec<String>>()
                } else {
                    vec![]
                },
                exclude_commands: if let Some(s) = exclude_commands {
                    s.split(',').map(|x| x.to_string()).collect::<Vec<String>>()
                } else {
                    vec![]
                },
                lockdir: lockdir.clone(),
                fmt: if *csv {
                    ps::Format::CSV
                } else {
                    ps::Format::NewJSON
                },
                cpu_util: true,
                token,
            };

            let system = system.with_jobmanager(Box::new(jobsapi::AnyJobManager::new(force_slurm)));
            let system = if cluster.is_some() {
                system.with_cluster(cluster.as_ref().unwrap())
            } else {
                system
            };
            ps::create_snapshot(
                writer,
                &system.freeze().expect("System initialization"),
                &opts,
            );
        }
        Commands::Sysinfo {
            csv,
            oldjson,
            cluster,
            topo_svg_cmd,
            topo_text_cmd,
        } => {
            let system = if cluster.is_some() {
                system.with_cluster(cluster.as_ref().unwrap())
            } else {
                system
            };
            sysinfo::show_system(
                writer,
                &system.freeze().expect("System initialization"),
                token,
                if *csv {
                    sysinfo::Format::CSV
                } else if *oldjson {
                    sysinfo::Format::OldJSON
                } else {
                    sysinfo::Format::NewJSON
                },
                topo_svg_cmd.clone(),
                topo_text_cmd.clone(),
            );
        }
        Commands::Slurmjobs {
            window,
            span,
            csv,
            deluge,
            cluster,
        } => {
            let system = if cluster.is_some() {
                system.with_cluster(cluster.as_ref().unwrap())
            } else {
                system
            };
            slurmjobs::show_slurm_jobs(
                writer,
                window,
                span,
                *deluge,
                &system.freeze().expect("System initialization"),
                token,
                if *csv {
                    slurmjobs::Format::CSV
                } else {
                    slurmjobs::Format::NewJSON
                },
            );
        }
        Commands::Cluster { cluster } => {
            let system = if cluster.is_some() {
                system.with_cluster(cluster.as_ref().unwrap())
            } else {
                system
            };
            cluster::show_cluster(
                writer,
                &system.freeze().expect("System initialization"),
                token,
            );
        }
        Commands::Version {} => {
            show_version(writer);
        }
    }
    let _ = writer.flush();
}

// For the sake of simplicity:
//  - allow repeated options to overwrite earlier values
//  - all error reporting is via a generic "usage" message, without specificity as to what was wrong
//  - both --json and --csv are accepted to all commands
//
// Note that --json means "new json" everywhere, so --json for `sonar sysinfo` changes the output
// format from the default old JSON encoding.

fn command_line() -> Commands {
    let args = std::env::args().collect::<Vec<String>>();
    let mut next = 1;
    if next < args.len() {
        let command = args[next].as_ref();
        next += 1;
        match command {
            #[cfg(feature = "daemon")]
            "daemon" => {
                if next >= args.len() {
                    usage(true);
                }
                let config_file = args[next].to_string();
                next += 1;
                if next != args.len() {
                    usage(true);
                }
                Commands::Daemon { config_file }
            }
            "ps" => {
                let mut rollup = false;
                let mut min_cpu_percent = None;
                let mut min_mem_percent = None;
                let mut min_cpu_time = None;
                let mut exclude_system_jobs = false;
                let mut exclude_users = None;
                let mut exclude_commands = None;
                let mut lockdir = None;
                let mut load = false;
                let mut json = false;
                let mut csv = false;
                let mut cluster = None;
                while next < args.len() {
                    let arg = args[next].as_ref();
                    next += 1;
                    if let Some(new_next) = bool_arg(arg, &args, next, "--batchless") {
                        // Old argument that has no effect, will remove later
                        next = new_next;
                    } else if let Some(new_next) = bool_arg(arg, &args, next, "--rollup") {
                        (next, rollup) = (new_next, true);
                    } else if let Some(new_next) = bool_arg(arg, &args, next, "--load") {
                        (next, load) = (new_next, true);
                    } else if let Some(new_next) = bool_arg(arg, &args, next, "--json") {
                        (next, json) = (new_next, true);
                    } else if let Some(new_next) = bool_arg(arg, &args, next, "--csv") {
                        (next, csv) = (new_next, true);
                    } else if let Some(new_next) = bool_arg(arg, &args, next, "--oldfmt") {
                        (next, csv) = (new_next, true);
                    } else if let Some(new_next) =
                        bool_arg(arg, &args, next, "--exclude-system-jobs")
                    {
                        (next, exclude_system_jobs) = (new_next, true);
                    } else if let Some((new_next, value)) =
                        string_arg(arg, &args, next, "--exclude-users")
                    {
                        (next, exclude_users) = (new_next, Some(value));
                    } else if let Some((new_next, value)) =
                        string_arg(arg, &args, next, "--exclude-commands")
                    {
                        (next, exclude_commands) = (new_next, Some(value));
                    } else if let Some((new_next, value)) =
                        string_arg(arg, &args, next, "--lockdir")
                    {
                        (next, lockdir) = (new_next, Some(value));
                    } else if let Some((new_next, value)) =
                        numeric_arg::<f64>(arg, &args, next, "--min-cpu-percent")
                    {
                        (next, min_cpu_percent) = (new_next, Some(value));
                    } else if let Some((new_next, value)) =
                        numeric_arg::<f64>(arg, &args, next, "--min-mem-percent")
                    {
                        (next, min_mem_percent) = (new_next, Some(value));
                    } else if let Some((new_next, value)) =
                        numeric_arg::<usize>(arg, &args, next, "--min-cpu-time")
                    {
                        (next, min_cpu_time) = (new_next, Some(value));
                    } else if let Some((new_next, value)) =
                        string_arg(arg, &args, next, "--cluster")
                    {
                        (next, cluster) = (new_next, Some(value));
                    } else {
                        usage(true);
                    }
                }
                if json && csv {
                    eprintln!("--csv and --json are incompatible");
                    std::process::exit(USAGE_ERROR);
                }
                Commands::PS {
                    rollup,
                    min_cpu_percent,
                    min_mem_percent,
                    min_cpu_time,
                    exclude_system_jobs,
                    exclude_users,
                    exclude_commands,
                    lockdir,
                    load,
                    csv,
                    cluster,
                }
            }
            "sysinfo" => {
                let mut json = false;
                let mut oldjson = false;
                let mut csv = false;
                let mut cluster = None;
                let mut topo_svg_cmd = None;
                let mut topo_text_cmd = None;
                while next < args.len() {
                    let arg = args[next].as_ref();
                    next += 1;
                    if let Some(new_next) = bool_arg(arg, &args, next, "--json") {
                        (next, json) = (new_next, true);
                    } else if let Some(new_next) = bool_arg(arg, &args, next, "--oldfmt") {
                        (next, oldjson) = (new_next, true);
                    } else if let Some(new_next) = bool_arg(arg, &args, next, "--csv") {
                        (next, csv) = (new_next, true);
                    } else if let Some((new_next, value)) =
                        string_arg(arg, &args, next, "--cluster")
                    {
                        (next, cluster) = (new_next, Some(value));
                    } else if let Some((new_next, value)) =
                        string_arg(arg, &args, next, "--topo-svg-cmd")
                    {
                        (next, topo_svg_cmd) = (new_next, Some(value));
                    } else if let Some((new_next, value)) =
                        string_arg(arg, &args, next, "--topo-text-cmd")
                    {
                        (next, topo_text_cmd) = (new_next, Some(value));
                    } else {
                        usage(true);
                    }
                }
                if (json || oldjson) && csv {
                    eprintln!("--csv is incompatible with --json and --oldfmt");
                    std::process::exit(USAGE_ERROR);
                }
                if json && oldjson {
                    eprintln!("--json and --oldfmt are incompatible");
                    std::process::exit(USAGE_ERROR);
                }
                Commands::Sysinfo {
                    csv,
                    oldjson,
                    cluster,
                    topo_svg_cmd,
                    topo_text_cmd,
                }
            }
            "slurm" => {
                let mut window = None;
                let mut span = None;
                let mut json = false;
                let mut csv = false;
                let mut deluge = false;
                let mut cluster = None;
                while next < args.len() {
                    let arg = args[next].as_ref();
                    next += 1;
                    if let Some((new_next, value)) =
                        numeric_arg::<u32>(arg, &args, next, "--window")
                    {
                        (next, window) = (new_next, Some(value));
                    } else if let Some((new_next, value)) = string_arg(arg, &args, next, "--span") {
                        (next, span) = (new_next, Some(value));
                    } else if let Some(new_next) = bool_arg(arg, &args, next, "--json") {
                        (next, json) = (new_next, true);
                    } else if let Some(new_next) = bool_arg(arg, &args, next, "--oldfmt") {
                        (next, csv) = (new_next, true);
                    } else if let Some(new_next) = bool_arg(arg, &args, next, "--csv") {
                        (next, csv) = (new_next, true);
                    } else if let Some(new_next) = bool_arg(arg, &args, next, "--deluge") {
                        (next, deluge) = (new_next, true);
                    } else if let Some((new_next, value)) =
                        string_arg(arg, &args, next, "--cluster")
                    {
                        (next, cluster) = (new_next, Some(value));
                    } else {
                        usage(true);
                    }
                }
                if window.is_some() && span.is_some() {
                    usage(true);
                }
                if json && csv {
                    eprintln!("--csv and --json are incompatible");
                    std::process::exit(USAGE_ERROR);
                }
                Commands::Slurmjobs {
                    window,
                    span,
                    csv,
                    cluster,
                    deluge,
                }
            }
            "cluster" => {
                let mut cluster = None;
                while next < args.len() {
                    let arg = args[next].as_ref();
                    next += 1;
                    if let Some(new_next) = bool_arg(arg, &args, next, "--json") {
                        // Ignore, there is only one format
                        next = new_next;
                    } else if let Some(new_next) = bool_arg(arg, &args, next, "--oldfmt") {
                        // Ignore, there is only one format
                        next = new_next;
                    } else if let Some((new_next, value)) =
                        string_arg(arg, &args, next, "--cluster")
                    {
                        (next, cluster) = (new_next, Some(value));
                    } else {
                        usage(true);
                    }
                }
                // The only output format supported is "new JSON", so require `--cluster` always
                Commands::Cluster { cluster }
            }
            "version" => Commands::Version {},
            "help" => {
                usage(false);
            }
            _ => {
                usage(true);
            }
        }
    } else {
        usage(true);
    }
}

fn bool_arg(arg: &str, _args: &[String], next: usize, opt_name: &str) -> Option<usize> {
    if arg == opt_name {
        Some(next)
    } else {
        None
    }
}

fn string_arg(arg: &str, args: &[String], next: usize, opt_name: &str) -> Option<(usize, String)> {
    if arg == opt_name {
        if next < args.len() {
            Some((next + 1, args[next].to_string()))
        } else {
            None
        }
    } else if let Some((first, rest)) = arg.split_once('=') {
        if first == opt_name {
            Some((next, rest.to_string()))
        } else {
            None
        }
    } else {
        None
    }
}

fn numeric_arg<T: std::str::FromStr>(
    arg: &str,
    args: &[String],
    next: usize,
    opt_name: &str,
) -> Option<(usize, T)> {
    if let Some((next, strval)) = string_arg(arg, args, next, opt_name) {
        match strval.parse::<T>() {
            Ok(value) => Some((next, value)),
            _ => {
                usage(true);
            }
        }
    } else {
        None
    }
}

fn usage(is_error: bool) -> ! {
    let mut stdout = std::io::stdout();
    let mut stderr = std::io::stderr();
    let out: &mut dyn std::io::Write = if is_error { &mut stderr } else { &mut stdout };

    show_version(out);
    let _ = out.write(env!("CARGO_PKG_REPOSITORY").as_bytes());
    let _ = out.write(
        b"

Usage: sonar <COMMAND>

Commands:
  daemon   Read configuration from file and stay resident
  ps       Print process and load information
  sysinfo  Print system information
  slurm    Print Slurm job information for a [start,end) time interval
  cluster  Print current batch cluster partition information and node status
  help     Print this message

Options for `daemon`:
  filename
      Configuration file from which to read commands, arguments, cadences.

Options for `ps`:
  --rollup
      Merge process records that have the same job ID and command name (on systems
      with stable job IDs only)
  --min-cpu-time seconds
      Include records for jobs that have used at least this much CPU time
      [default: none]
  --exclude-system-jobs
      Exclude records for system jobs (uid < 1000)
  --exclude-users user,user,...
      Exclude records whose users match these names [default: none]
  --exclude-commands command,command,...
      Exclude records whose commands start with these names [default: none]
  --min-cpu-percent percentage
      Include records for jobs that have on average used at least this
      percentage of CPU, NOTE THIS IS NONMONOTONIC [default: none]
  --min-mem-percent percentage
      Include records for jobs that presently use at least this percentage of
      real memory, NOTE THIS IS NONMONOTONIC [default: none]
  --lockdir directory
      Create a per-host lockfile in this directory and exit early if the file
      exists on startup [default: none]
  --load
      Print per-cpu and per-gpu load data
  --csv
      Format output as old CSV, not JSON
  --oldfmt
      Synonym for --csv
  --cluster name
      Optional cluster name with which to tag output

Options for `sysinfo`:
  --csv
      Format output as CSV, not JSON
  --oldfmt
      Format output as the old JSON, not the new JSON
  --cluster name
      Optional cluster name with which to tag output
  --topo-svg-cmd
      Optional command to execute to generate SVG source for the topo_svg field,
      typically '/path/to/lstopo -of svg'
  --topo-text-cmd
      Optional command to execute to generate text for the topo_text field,
      typically '/path/to/hwloc-ls'

Options for `slurm`:
  --window minutes
      Set the `start` time to now-minutes [default: 90] and the `end` time to now+1.
      Precludes --span
  --span start,end
      Both `start` and `end` are on the form yyyy-mm-dd.  Mostly useful for seeding a
      database with older data.  Precludes --window
  --deluge
      Include PENDING and RUNNING jobs in the output, not just completed jobs.
  --csv
      Format output as CSV, not new JSON
  --oldfmt
      Synonym for --csv
  --cluster name
      Optional cluster name with which to tag output

Options for `cluster`:
  --cluster name
      Optional cluster name with which to tag output
",
    );
    let _ = out.flush();
    std::process::exit(if is_error { USAGE_ERROR } else { 0 });
}

// Print the true version, not something parameterized by the systemapi object.
fn show_version(out: &mut dyn std::io::Write) {
    let _ = out.write(b"sonar version ");
    let _ = out.write(env!("CARGO_PKG_VERSION").as_bytes());
    let _ = out.write(b"\n");
}
