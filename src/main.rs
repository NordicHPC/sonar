use clap::{Parser, Subcommand};

mod command;
mod nvidia;
mod process;
mod ps;
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
        #[arg(long, default_value_t = 0.5)]
        cpu_cutoff_percent: f64,
        #[arg(long, default_value_t = 5.0)]
        mem_cutoff_percent: f64,
    },
    /// Not yet implemented
    Analyze {},
}

fn main() {
    let cli = Cli::parse();

    match &cli.command {
        Commands::PS {
            cpu_cutoff_percent,
            mem_cutoff_percent,
        } => {
            ps::create_snapshot(*cpu_cutoff_percent, *mem_cutoff_percent);
        }
        Commands::Analyze {} => {
            println!("sonar analyze not yet completed");
        }
    }
}
