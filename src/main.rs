extern crate env_logger;

use clap::{Parser, Subcommand};

mod amd;
mod batchless;
mod command;
mod jobs;
mod nvidia;
mod process;
mod ps;
mod slurm;
mod util;

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

        /// Include records for jobs that have on average used at least this percentage of CPU, note this is nonmonotonic [default: none]
        #[arg(long)]
        min_cpu_percent: Option<f64>,

        /// Include records for jobs that presently use at least this percentage of real memory, note this is nonmonotonic [default: none]
        #[arg(long)]
        min_mem_percent: Option<f64>,

        /// Include records for jobs that have used at least this much CPU time (in seconds) [default: none]
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
    },
    /// Not yet implemented
    Analyze {},
}

fn main() {
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
        } => {
            let opts = ps::PsOptions {
                rollup: *rollup,
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
            };
            if *batchless {
                let mut jm = batchless::BatchlessJobManager::new();
                ps::create_snapshot(&mut jm, &opts);
            } else {
                let mut jm = slurm::SlurmJobManager {};
                ps::create_snapshot(&mut jm, &opts);
            }
        }
        Commands::Analyze {} => {
            println!("sonar analyze not yet completed");
        }
    }
}
