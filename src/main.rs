use clap::{Args, Parser, Subcommand};
use serde::Deserialize;
use std::collections::HashMap;
use std::convert::TryFrom;
use std::path::{Path, PathBuf};
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
    #[arg(long, default_value = "youtube")]
    pub profile: String,
}

#[derive(Deserialize)]
pub struct AppConfig {
    pub profiles: HashMap<String, Profile>,
}

#[derive(Clone, Deserialize)]
pub struct Profile {
    pub extension: String,
    pub width: u32,
    pub height: u32,
    pub require_audio: bool,
    pub min_duration_secs: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Warn,
    Fail,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Pass,
    Warn,
    Fail,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub code: &'static str,
    pub severity: Severity,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationResult {
    pub findings: Vec<Finding>,
    pub status: Status,
}

impl ValidationResult {
    pub fn from_findings(findings: Vec<Finding>) -> Self {
        let status = if findings.iter().any(|f| f.severity == Severity::Fail) {
            Status::Fail
        } else if findings.iter().any(|f| f.severity == Severity::Warn) {
            Status::Warn
        } else {
            Status::Pass
        };

        Self { findings, status }
    }
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
            }

            let profile = config
                .profiles
                .get(check_command.profile.as_str())
                .ok_or("Profile not found")?
                .clone();

            let media_files = scan_directory(check_command.path)?;

            if media_files.is_empty() {
                println!("No media files found");
                return Ok(());
            }

            println!(
                "Checking {} file(s) against profile '{}'",
                media_files.len(),
                check_command.profile
            );

            for file in media_files {
                println!("\n{}", file.display());
                match probe_media(file.clone()) {
                    Ok(metadata) => {
                        let result = validate(&file, &metadata, &profile);
                        print_validation_result(&result);
                    }
                    Err(err) => {
                        eprintln!("  probe error: {err}");
                    }
                }
            }
        }
        Commands::Probe(probe_command) => {
            if !probe_command.path.exists() {
                eprintln!("Path does not exist: {}", probe_command.path.display());
                return Err("Path does not exist".into());
            }

            if !probe_command.path.is_file() {
                eprintln!("Probe requires a file path: {}", probe_command.path.display());
                return Err("Probe requires a file path".into());
            }

            let profile = config
                .profiles
                .get(probe_command.profile.as_str())
                .ok_or("Profile not found")?
                .clone();

            let metadata = probe_media(probe_command.path.clone())?;
            println!("Media metadata: {metadata:?}");

            let result = validate(&probe_command.path, &metadata, &profile);
            print_validation_result(&result);
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

fn validate(path: &Path, metadata: &MediaMetadata, profile: &Profile) -> ValidationResult {
    let mut findings = Vec::new();

    let extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("");

    if !extension.eq_ignore_ascii_case(&profile.extension) {
        findings.push(Finding {
            code: "EXTENSION_MISMATCH",
            severity: Severity::Fail,
            message: format!(
                "expected .{}, got .{}",
                profile.extension, extension
            ),
        });
    }

    if metadata.width != profile.width || metadata.height != profile.height {
        findings.push(Finding {
            code: "RESOLUTION_MISMATCH",
            severity: Severity::Fail,
            message: format!(
                "expected {}x{}, got {}x{}",
                profile.width, profile.height, metadata.width, metadata.height
            ),
        });
    }

    if profile.require_audio && !metadata.has_audio {
        findings.push(Finding {
            code: "AUDIO_REQUIRED",
            severity: Severity::Fail,
            message: "profile requires an audio track".into(),
        });
    }

    if metadata.duration_secs < profile.min_duration_secs {
        findings.push(Finding {
            code: "DURATION_TOO_SHORT",
            severity: Severity::Fail,
            message: format!(
                "duration {:.2}s is below minimum {:.2}s",
                metadata.duration_secs, profile.min_duration_secs
            ),
        });
    }

    ValidationResult::from_findings(findings)
}

fn print_validation_result(result: &ValidationResult) {
    println!("  status: {:?}", result.status);

    if result.findings.is_empty() {
        println!("  no findings");
        return;
    }

    for finding in &result.findings {
        println!(
            "  [{}] {}: {}",
            format!("{:?}", finding.severity).to_lowercase(),
            finding.code,
            finding.message
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_profile() -> Profile {
        Profile {
            extension: "mp4".into(),
            width: 1920,
            height: 1080,
            require_audio: true,
            min_duration_secs: 5.0,
        }
    }

    fn sample_metadata() -> MediaMetadata {
        MediaMetadata {
            duration_secs: 10.0,
            width: 1920,
            height: 1080,
            has_audio: true,
            format_name: "mp4".into(),
            video_codec: Some("h264".into()),
        }
    }

    #[test]
    fn validate_passes_when_metadata_matches_profile() {
        let path = Path::new("clip.mp4");
        let result = validate(path, &sample_metadata(), &sample_profile());

        assert_eq!(result.status, Status::Pass);
        assert!(result.findings.is_empty());
    }

    #[test]
    fn validate_flags_extension_mismatch() {
        let path = Path::new("clip.mov");
        let result = validate(path, &sample_metadata(), &sample_profile());

        assert_eq!(result.status, Status::Fail);
        assert!(result
            .findings
            .iter()
            .any(|f| f.code == "EXTENSION_MISMATCH"));
    }

    #[test]
    fn validate_flags_resolution_mismatch() {
        let path = Path::new("clip.mp4");
        let mut metadata = sample_metadata();
        metadata.width = 1280;
        metadata.height = 720;

        let result = validate(path, &metadata, &sample_profile());

        assert_eq!(result.status, Status::Fail);
        assert!(result
            .findings
            .iter()
            .any(|f| f.code == "RESOLUTION_MISMATCH"));
    }

    #[test]
    fn validate_flags_missing_required_audio() {
        let path = Path::new("clip.mp4");
        let mut metadata = sample_metadata();
        metadata.has_audio = false;

        let result = validate(path, &metadata, &sample_profile());

        assert_eq!(result.status, Status::Fail);
        assert!(result.findings.iter().any(|f| f.code == "AUDIO_REQUIRED"));
    }

    #[test]
    fn validate_flags_short_duration() {
        let path = Path::new("clip.mp4");
        let mut metadata = sample_metadata();
        metadata.duration_secs = 2.0;

        let result = validate(path, &metadata, &sample_profile());

        assert_eq!(result.status, Status::Fail);
        assert!(result
            .findings
            .iter()
            .any(|f| f.code == "DURATION_TOO_SHORT"));
    }
}
