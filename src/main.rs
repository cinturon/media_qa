use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "mediaqa", about = "Media QA preflight checker")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    Check(CheckCommand),
}

#[derive(Args)]
pub struct CheckCommand {
    pub path: PathBuf,
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Check(check_command) => {
            if !check_command.path.is_file() {
                eprintln!("Path does not exist: {}", check_command.path.display());
                std::process::exit(1);
            } else {
                println!("Path exists: {}", check_command.path.display());
            }
        }
    }
}
