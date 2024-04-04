extern crate env_logger;

mod amd;
mod batchless;
mod command;
mod gpu;
mod jobs;
mod nvidia;
mod procfs;
mod procfsapi;
mod ps;
mod slurm;
mod sysinfo;
mod users;
mod util;

const TIMEOUT_SECONDS: u64 = 5; // For subprocesses

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

        /// Create a per-host lockfile in this directory and exit early if the file exists on startup [default: none]
        lockdir: Option<String>,
    },
    /// Extract system information
    Sysinfo {},
}

fn main() {
    // Obtain the time stamp early so that it more properly reflects the time the sample was
    // obtained, not the time when reporting was allowed to run.  The latter is subject to greater
    // system effects, and using that timestamp increases the risk that the samples' timestamp order
    // improperly reflects the true order in which they were obtained.  See #100.
    let timestamp = util::time_iso8601();

    env_logger::init();

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
        } => {
            let opts = ps::PsOptions {
                rollup: *rollup,
                always_print_something: true,
                min_cpu_percent: *min_cpu_percent,
                min_mem_percent: *min_mem_percent,
                min_cpu_time: *min_cpu_time,
                exclude_system_jobs: *exclude_system_jobs,
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
    }
}

// For the sake of simplicity:
//  - allow repeated options to overwrite earlier values
//  - all error reporting is via a generic "usage" message, without specificity as to what was wrong

fn command_line() -> Commands {
    let mut args = std::env::args();
    let _executable = args.next();
    if let Some(command) = args.next() {
        match command.as_str() {
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
                loop {
                    if let Some(arg) = args.next() {
                        match arg.as_str() {
                            "--batchless" => {
                                batchless = true;
                            }
                            "--rollup" => {
                                rollup = true;
                            }
                            "--exclude-system-jobs" => {
                                exclude_system_jobs = true;
                            }
                            "--exclude-users" => {
                                (args, exclude_users) = string_value(args);
                            }
                            "--exclude-commands" => {
                                (args, exclude_commands) = string_value(args);
                            }
                            "--lockdir" => {
                                (args, lockdir) = string_value(args);
                            }
                            "--min-cpu-percent" => {
                                (args, min_cpu_percent) = parsed_value::<f64>(args);
                            }
                            "--min-mem-percent" => {
                                (args, min_mem_percent) = parsed_value::<f64>(args);
                            }
                            "--min-cpu-time" => {
                                (args, min_cpu_time) = parsed_value::<usize>(args);
                            }
                            _ => {
                                usage(true);
                            }
                        }
                    } else {
                        break;
                    }
                }
                return Commands::PS {
                    batchless,
                    rollup,
                    min_cpu_percent,
                    min_mem_percent,
                    min_cpu_time,
                    exclude_system_jobs,
                    exclude_users,
                    exclude_commands,
                    lockdir,
                }
            }
            "sysinfo" => {
                return Commands::Sysinfo {}
            }
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

fn string_value(mut args: std::env::Args) -> (std::env::Args, Option<String>) {
    if let Some(val) = args.next() {
        (args, Some(val))
    } else {
        usage(true);
    }
}

fn parsed_value<T: std::str::FromStr>(mut args: std::env::Args) -> (std::env::Args, Option<T>) {
    if let Some(val) = args.next() {
        match val.parse::<T>() {
            Ok(value) => {
                (args, Some(value))
            }
            _ => {
                usage(true);
            }
        }
    } else {
        usage(true);
    }
}

fn usage(is_error: bool) -> ! {
    let mut stdout = std::io::stdout();
    let mut stderr = std::io::stderr();
    let out: &mut dyn std::io::Write = if is_error { &mut stderr } else { &mut stdout };
    let _ = out.write(b"Usage: sonar <COMMAND>

Commands:
  ps       Take a snapshot of the currently running processes
  sysinfo  Extract system information
  help     Print this message

Options for `ps`:
  --batchless
      Synthesize a job ID from the process tree in which a process finds itself
  --rollup
      Merge process records that have the same job ID and command name
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
");
    let _ = out.flush();
    std::process::exit(if is_error { 2 } else { 0 });
}
