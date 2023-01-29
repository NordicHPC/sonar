use clap::{Parser, Subcommand};

mod command;
mod ps;

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
        #[arg(long, default_value_t = 0.5)]
        mem_cutoff_percent: f64,
        #[arg(long, default_value_t = 50.0)]
        mem_cutoff_percent_idle: f64,
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
            mem_cutoff_percent_idle,
        } => {
            ps::create_snapshot(
                *cpu_cutoff_percent,
                *mem_cutoff_percent,
                *mem_cutoff_percent_idle,
            );
        }
        Commands::Analyze {} => {
            println!("sonar analyze not yet completed");
        }
    }
}
