use clap::{Parser, Subcommand};
use std::collections::HashSet;
use std::fs;

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
        #[arg(long, default_value_t = 5.0)]
        mem_cutoff_percent: f64,
    },
    /// Not yet implemented
    Analyze {
        #[arg(long)]
        file_name: String,
    },
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
        Commands::Analyze { file_name } => {
            let users = read_users(file_name);
            dbg!(users);
        }
    }
}

// later we will move this function to own file
fn read_users(file_name: &str) -> HashSet<String> {
    let error_message = format!("something went wrong reading file {}", file_name);
    let contents = fs::read_to_string(file_name).expect(&error_message);
    let lines = contents.lines();

    let mut users = HashSet::new();

    for line in lines {
        let words: Vec<&str> = line.split(',').collect();
        let user = words[3].parse().unwrap();
        users.insert(user);
    }

    users
}
