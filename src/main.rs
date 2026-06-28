use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;
use serde::{Deserialize};
use std::collections::HashMap;

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
    #[arg(long, default_value = "youtube")]
    pub profile: String,
}

#[derive(Deserialize)]
pub struct AppConfig {
    pub profiles: HashMap<String, Profile>,
}

#[derive(Deserialize)]
pub struct Profile {
    pub extension: String,
    pub width: u32,
    pub height: u32,
    pub require_audio: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {

    let contents = std::fs::read_to_string("src/sample.toml")?;
    let config: AppConfig = toml::from_str(&contents)?;
    let cli = Cli::parse();

    match cli.command {
        Commands::Check(check_command) => {
            if !check_command.path.is_file() {
                eprintln!("Path does not exist: {}", check_command.path.display());
                return Err("Path does not exist".into());
            } else {
                println!("Path exists: {}", check_command.path.display());
            }

            let profile = config.profiles.get(check_command.profile.as_str()).ok_or("Profile not found")?;
            println!("Loaded profile: {}x{}", profile.width, profile.height);
        }
    }   
    Ok(())
}
