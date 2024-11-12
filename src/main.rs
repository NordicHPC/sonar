mod amd;
mod batchless;
mod command;
mod gpu;
mod hostname;
mod interrupt;
mod jobs;
mod log;
mod nvidia;
mod procfs;
mod procfsapi;
mod ps;
mod slurm;
mod slurmjobs;
mod sysinfo;
mod time;
mod users;
mod util;

const TIMEOUT_SECONDS: u64 = 5; // For subprocesses
const USAGE_ERROR: i32 = 2; // clap, Python, Go

enum Commands {
    /// Take a snapshot of the currently running processes
    PS {
        /// Synthesize a job ID from the process tree in which a process finds itself
        batchless: bool,

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
    },
    /// Extract system information
    Sysinfo {},
    /// Extract slurm job information
    Slurmjobs {
        /// Set the sacct start time to now-`window` and the end time to now, and dump records that
        /// are relevant for that interval.  Normally the running interval of `sonar slurm` should
        /// be less than the window.  Precludes -span.
        window: Option<u32>,

        /// From-to dates on the form yyyy-mm-dd,yyyy-mm-dd (with the comma); from is inclusive,
        /// to is exclusive.  Precludes -window.
        span: Option<String>,
    },
    Version {},
}

fn main() {
    // Obtain the time stamp early so that it more properly reflects the time the sample was
    // obtained, not the time when reporting was allowed to run.  The latter is subject to greater
    // system effects, and using that timestamp increases the risk that the samples' timestamp order
    // improperly reflects the true order in which they were obtained.  See #100.
    let timestamp = time::now_iso8601();

    log::init();

    match &command_line() {
        Commands::PS {
            rollup,
            batchless,
            min_cpu_percent,
            min_mem_percent,
            min_cpu_time,
            exclude_system_jobs,
            exclude_users,
            exclude_commands,
            lockdir,
            load,
        } => {
            let opts = ps::PsOptions {
                rollup: *rollup,
                always_print_something: true,
                min_cpu_percent: *min_cpu_percent,
                min_mem_percent: *min_mem_percent,
                min_cpu_time: *min_cpu_time,
                exclude_system_jobs: *exclude_system_jobs,
                load: *load,
                exclude_users: if let Some(s) = exclude_users {
                    s.split(',').collect::<Vec<&str>>()
                } else {
                    vec![]
                },
                exclude_commands: if let Some(s) = exclude_commands {
                    s.split(',').collect::<Vec<&str>>()
                } else {
                    vec![]
                },
                lockdir: lockdir.clone(),
            };
            if *batchless {
                let mut jm = batchless::BatchlessJobManager::new();
                ps::create_snapshot(&mut jm, &opts, &timestamp);
            } else {
                let mut jm = slurm::SlurmJobManager {};
                ps::create_snapshot(&mut jm, &opts, &timestamp);
            }
        }
        Commands::Sysinfo {} => {
            sysinfo::show_system(&timestamp);
        }
        Commands::Slurmjobs { window, span } => {
            slurmjobs::show_slurm_jobs(window, span);
        }
        Commands::Version {} => {
            show_version(&mut std::io::stdout());
        }
    }
}

// For the sake of simplicity:
//  - allow repeated options to overwrite earlier values
//  - all error reporting is via a generic "usage" message, without specificity as to what was wrong

fn command_line() -> Commands {
    let args = std::env::args().collect::<Vec<String>>();
    let mut next = 1;
    if next < args.len() {
        let command = args[next].as_ref();
        next += 1;
        match command {
            "ps" => {
                let mut batchless = false;
                let mut rollup = false;
                let mut min_cpu_percent = None;
                let mut min_mem_percent = None;
                let mut min_cpu_time = None;
                let mut exclude_system_jobs = false;
                let mut exclude_users = None;
                let mut exclude_commands = None;
                let mut lockdir = None;
                let mut load = false;
                while next < args.len() {
                    let arg = args[next].as_ref();
                    next += 1;
                    if let Some(new_next) = bool_arg(arg, &args, next, "--batchless") {
                        (next, batchless) = (new_next, true);
                    } else if let Some(new_next) = bool_arg(arg, &args, next, "--rollup") {
                        (next, rollup) = (new_next, true);
                    } else if let Some(new_next) = bool_arg(arg, &args, next, "--load") {
                        (next, load) = (new_next, true);
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
                    } else {
                        usage(true);
                    }
                }

                #[cfg(debug_assertions)]
                let allow_incompatible = std::env::var("SONARTEST_ROLLUP").is_ok();

                #[cfg(not(debug_assertions))]
                let allow_incompatible = false;

                if rollup && batchless && !allow_incompatible {
                    eprintln!("--rollup and --batchless are incompatible");
                    std::process::exit(USAGE_ERROR);
                }

                Commands::PS {
                    batchless,
                    rollup,
                    min_cpu_percent,
                    min_mem_percent,
                    min_cpu_time,
                    exclude_system_jobs,
                    exclude_users,
                    exclude_commands,
                    lockdir,
                    load,
                }
            }
            "sysinfo" => Commands::Sysinfo {},
            "slurm" => {
                let mut window = None;
                let mut span = None;
                while next < args.len() {
                    let arg = args[next].as_ref();
                    next += 1;
                    if let Some((new_next, value)) =
                        numeric_arg::<u32>(arg, &args, next, "--window")
                    {
                        (next, window) = (new_next, Some(value));
                    } else if let Some((new_next, value)) = string_arg(arg, &args, next, "--span") {
                        (next, span) = (new_next, Some(value));
                    } else {
                        usage(true);
                    }
                }
                if window.is_some() && span.is_some() {
                    usage(true);
                }
                Commands::Slurmjobs { window, span }
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
  ps       Take a snapshot of the currently running processes
  sysinfo  Extract system information
  slurm    Extract slurm job information for a [start,end) time interval
  help     Print this message

Options for `ps`:
  --batchless
      Synthesize a job ID from the process tree in which a process finds itself
  --rollup
      Merge process records that have the same job ID and command name (not
      compatible with --batchless)
  --min-cpu-percent percentage
      Include records for jobs that have on average used at least this
      percentage of CPU, note this is nonmonotonic [default: none]
  --min-mem-percent percentage
      Include records for jobs that presently use at least this percentage of
      real memory, note this is nonmonotonic [default: none]
  --min-cpu-time seconds
      Include records for jobs that have used at least this much CPU time
      [default: none]
  --exclude-system-jobs
      Exclude records for system jobs (uid < 1000)
  --exclude-users user,user,...
      Exclude records whose users match these names [default: none]
  --exclude-commands command,command,...
      Exclude records whose commands start with these names [default: none]
  --lockdir directory
      Create a per-host lockfile in this directory and exit early if the file
      exists on startup [default: none]

Options for `slurm`:
  --window minutes
      Set the `start` time to now-minutes [default: 90] and the `end` time to now+1.
      Precludes --span
  --span start,end
      Both `start` and `end` are on the form yyyy-mm-dd.  Mostly useful for seeding a
      database with older data.  Precludes --window
",
    );
    let _ = out.flush();
    std::process::exit(if is_error { USAGE_ERROR } else { 0 });
}

fn show_version(out: &mut dyn std::io::Write) {
    let _ = out.write(b"sonar version ");
    let _ = out.write(env!("CARGO_PKG_VERSION").as_bytes());
    let _ = out.write(b"\n");
}
