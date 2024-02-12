extern crate env_logger;

use clap::{Parser, Subcommand};

mod amd;
mod batchless;
mod command;
mod jobs;
mod nvidia;
mod procfs;
mod procfsapi;
mod ps;
mod slurm;
mod util;

const TIMEOUT_SECONDS: u64 = 5; // For subprocesses

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Take a snapshot of the currently running processes
    PS {
        /// Synthesize a job ID from the process tree in which a process finds itself
        #[arg(long, default_value_t = false)]
        batchless: bool,

        /// Merge process records that have the same job ID and command name
        #[arg(long, default_value_t = false)]
        rollup: bool,

        /// Include records for jobs that have on average used at least this percentage of CPU,
        /// note this is nonmonotonic [default: none]
        #[arg(long)]
        min_cpu_percent: Option<f64>,

        /// Include records for jobs that presently use at least this percentage of real memory,
        /// note this is nonmonotonic [default: none]
        #[arg(long)]
        min_mem_percent: Option<f64>,

        /// Include records for jobs that have used at least this much CPU time (in seconds)
        /// [default: none]
        #[arg(long)]
        min_cpu_time: Option<usize>,

        /// Exclude records for system jobs (uid < 1000)
        #[arg(long, default_value_t = false)]
        exclude_system_jobs: bool,

        /// Exclude records whose users match these comma-separated names [default: none]
        #[arg(long)]
        exclude_users: Option<String>,

        /// Exclude records whose commands start with these comma-separated names [default: none]
        #[arg(long)]
        exclude_commands: Option<String>,

        /// Create a per-host lockfile in this directory and exit early if the file exists on startup [default: none]
        #[arg(long)]
        lockdir: Option<String>,
    },
    /// Not yet implemented
    Analyze {},
}

fn main() {
    // Obtain the time stamp early so that it more properly reflects the time the sample was
    // obtained, not the time when reporting was allowed to run.  The latter is subject to greater
    // system effects, and using that timestamp increases the risk that the samples' timestamp order
    // improperly reflects the true order in which they were obtained.  See #100.
    let timestamp = util::time_iso8601();

    env_logger::init();

    let cli = Cli::parse();

    match &cli.command {
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
        Commands::Analyze {} => {
            println!("sonar analyze not yet completed");
        }
    }
}
