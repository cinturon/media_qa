use clap::{Args, Parser, Subcommand};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;
use walkdir::WalkDir;

#[derive(Parser)]
#[command(name = "mediaqa", about = "Media QA preflight checker")]
pub struct Cli {
    #[command(subcommand)]
    pub commands: Commands,
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



    match cli.commands {
        Commands::Check(check_command) => {
            if !check_command.path.exists() {
                eprintln!("Path does not exist: {}", check_command.path.display());
                return Err("Path does not exist".into());
            } else {
                println!("Path exists: {}", check_command.path.display());
            }

            let profile = config
                .profiles
                .get(check_command.profile.as_str())
                .ok_or("Profile not found")?;
            println!("Loaded profile: {}x{}", profile.width, profile.height);

            let media_files = scan_directory(check_command.path)?;

            if media_files.is_empty() {
                println!("No media files found");
                return Ok(());
            }else { 

            println!("Found {} media files", media_files.len());
                for file in media_files {
                    println!("Candidate media file: {}", file.display());
                }
            }
        }
    }
    Ok(())
}
const MEDIA_EXTENSIONS: &[&str] = &["mp4", "mov", "mkv"];

fn scan_directory(
    path: PathBuf,
) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {

    let entries = WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_file())
        .filter(|e| {
            e.path()
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| MEDIA_EXTENSIONS.contains(&ext))
        })
        .map(|e| e.into_path())
        .collect();
    
    Ok(entries)
}
