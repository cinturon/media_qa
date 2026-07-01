mod analyze;
mod config;
mod metadata;
mod pipeline;
mod probe;
mod report;
mod scanner;
mod validate;

use clap::{Args, Parser, Subcommand};
use config::{load_config, Profile};
use pipeline::run_batch;
use report::{print_batch_report, print_json_report, BatchReport};
use scanner::scan;
use std::path::{Path, PathBuf};

const CONFIG_PATH: &str = "src/sample.toml";

#[derive(Parser)]
#[command(name = "mediaqa", about = "Media QA preflight checker")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Check(CheckCommand),
    Probe(ProbeCommand),
}

#[derive(Args)]
struct CheckCommand {
    path: PathBuf,
    #[arg(long, default_value = "youtube")]
    profile: String,
    #[arg(long)]
    json: bool,
}

#[derive(Args)]
struct ProbeCommand {
    path: PathBuf,
    #[arg(long, default_value = "youtube")]
    profile: String,
    #[arg(long)]
    json: bool,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("Error: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let config = load_config(Path::new(CONFIG_PATH))?;
    let cli = Cli::parse();

    let exit_code = match cli.command {
        Commands::Check(check) => run_check(&config, check)?,
        Commands::Probe(probe) => run_probe(&config, probe)?,
    };

    std::process::exit(exit_code);
}

fn run_check(
    config: &config::AppConfig,
    check: CheckCommand,
) -> Result<i32, Box<dyn std::error::Error>> {
    if !check.path.exists() {
        return Err("Path does not exist".into());
    }

    let profile = load_profile(config, &check.profile)?;
    let media_files = scan(&check.path)?;

    if media_files.is_empty() {
        println!("No media files found");
        return Ok(0);
    }

    let report = run_batch(media_files, &profile, &check.profile);
    render_report(&report, check.json);
    Ok(report.exit_code())
}

fn run_probe(
    config: &config::AppConfig,
    probe: ProbeCommand,
) -> Result<i32, Box<dyn std::error::Error>> {
    if !probe.path.exists() {
        return Err("Path does not exist".into());
    }

    if !probe.path.is_file() {
        return Err("Probe requires a file path".into());
    }

    let profile = load_profile(config, &probe.profile)?;
    let report = run_batch(vec![probe.path], &profile, &probe.profile);
    render_report(&report, probe.json);
    Ok(report.exit_code())
}

fn load_profile(
    config: &config::AppConfig,
    name: &str,
) -> Result<Profile, Box<dyn std::error::Error>> {
    config
        .profiles
        .get(name)
        .cloned()
        .ok_or_else(|| format!("Profile not found: {name}").into())
}

fn render_report(report: &BatchReport, json: bool) {
    if json {
        print_json_report(report);
    } else {
        print_batch_report(report);
    }
}
