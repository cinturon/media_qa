use clap::{Args, Parser, Subcommand};
use serde::Deserialize;
use std::collections::HashMap;
use std::convert::TryFrom;
use std::path::PathBuf;
use std::process::Command;
use serde_json::from_slice;
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
    Probe(ProbeCommand),
}

#[derive(Args)]
pub struct CheckCommand {
    pub path: PathBuf,
    #[arg(long, default_value = "youtube")]
    pub profile: String,
}

#[derive(Args)]
pub struct ProbeCommand {
    pub path: PathBuf,
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

#[derive(Deserialize)]
pub struct FfprobeOutput {
    pub streams: Vec<FfprobeStream>,
    pub format: FfprobeFormat,
}

#[derive(Deserialize)]
pub struct FfprobeStream {
    pub index: u32,
    pub codec_type: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub duration: Option<String>,
    pub bit_rate: Option<String>,
    pub codec_name: Option<String>,
}

#[derive(Deserialize)]
pub struct FfprobeFormat {
    pub duration: Option<String>,
    pub bit_rate: Option<String>,
    pub format_name: Option<String>,
    pub start_time: Option<String>,
    pub size: Option<String>,
    pub probe_score: Option<u32>,
}

#[derive(Debug)]
pub struct MediaMetadata {
    pub duration_secs: f64,
    pub width: u32,
    pub height: u32,
    pub has_audio: bool,
    pub format_name: String,
    pub video_codec: Option<String>,
}

impl TryFrom<FfprobeOutput> for MediaMetadata {
    type Error = String;

    fn try_from(raw: FfprobeOutput) -> Result<Self, Self::Error> {
        let video = raw
            .streams
            .iter()
            .find(|stream| stream.codec_type == Some("video".into()))
            .ok_or("No video stream found")?;

        let has_audio = raw
            .streams
            .iter()
            .any(|stream| stream.codec_type == Some("audio".into()));

        let duration_secs = raw
            .format
            .duration
            .as_deref()
            .ok_or("No duration found")?
            .parse::<f64>()
            .map_err(|e| e.to_string())?;

        Ok(MediaMetadata {
            duration_secs,
            width: video.width.ok_or("No width found")?,
            height: video.height.ok_or("No height found")?,
            has_audio,
            format_name: raw.format.format_name.unwrap_or_else(|| "unknown".into()),
            video_codec: video.codec_name.clone(),
        })
    }
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
            } else {
                println!("Found {} media files", media_files.len());
                for file in media_files {
                    println!("Candidate media file: {}", file.display());
                }
            }
        }
        Commands::Probe(probe_command) => {
            if !probe_command.path.exists() {
                eprintln!("Path does not exist: {}", probe_command.path.display());
                return Err("Path does not exist".into());
            } else {
                println!("Path exists: {}", probe_command.path.display());
            }

            let metadata = probe_media(probe_command.path)?;
            println!("Media metadata: {:?}", metadata);
        }
    }
    Ok(())
}
const MEDIA_EXTENSIONS: &[&str] = &["mp4", "mov", "mkv"];

fn scan_directory(path: PathBuf) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
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

fn probe_media(path: PathBuf) -> Result<MediaMetadata, Box<dyn std::error::Error>> {
    let output = Command::new("ffprobe")
        .arg("-v")
        .arg("quiet")
        .arg("-print_format")
        .arg("json")
        .arg("-show_format")
        .arg("-show_streams")
        .arg(path)
        .output()?;
    

    if !output.status.success() {
        return Err(format!("Failed to probe media: {}", String::from_utf8_lossy(&output.stderr)).into());
    }

    let raw_output: FfprobeOutput = from_slice(&output.stdout)?;
    let metadata = MediaMetadata::try_from(raw_output)?;

    Ok(metadata)
}
