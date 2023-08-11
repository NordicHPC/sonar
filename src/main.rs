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
        /// Do not print records for jobs that have not on average used more than this percentage of CPU
        #[arg(long, default_value_t = 0.5)]
        cpu_cutoff_percent: f64,

        /// Do not print records for jobs that do not presently use more than this percentage of real memory
        #[arg(long, default_value_t = 5.0)]
        mem_cutoff_percent: f64,

        /// Synthesize a job ID from the process tree in which a process finds itself
        #[arg(long, default_value_t = false)]
        batchless: bool,

        /// Merge process records that have the same job ID and command name
        #[arg(long, default_value_t = false)]
        rollup: bool,
    },
    /// Not yet implemented
    Analyze {},
}

fn main() {
    env_logger::init();

    let cli = Cli::parse();

    match &cli.command {
        Commands::PS {
            cpu_cutoff_percent,
            mem_cutoff_percent,
            rollup,
            batchless,
        } => {
            if *batchless {
                let mut jm = batchless::BatchlessJobManager::new();
                ps::create_snapshot(&mut jm, *rollup, *cpu_cutoff_percent, *mem_cutoff_percent);
            } else {
                let mut jm = slurm::SlurmJobManager {};
                ps::create_snapshot(&mut jm, *rollup, *cpu_cutoff_percent, *mem_cutoff_percent);
            }
        }
        Commands::Analyze {} => {
            println!("sonar analyze not yet completed");
        }
    }
}
