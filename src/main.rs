mod analyze;
mod api;
mod checklist;
mod checks;
mod config;
mod diff;
mod history;
mod html_report;
mod metadata;
mod package;
mod pipeline;
mod probe;
mod report;
mod scanner;
mod suggestions;
mod validate;
mod watch;
mod webhook;

use clap::{Args, Parser, Subcommand};
use checklist::render_checklist;
use config::{load_config, Profile};
use diff::{diff_reports, print_report_diff};
use history::{load_report, save_report};
use html_report::write_html_report;
use pipeline::{run_batch, BatchOptions};
use report::{print_batch_report, print_json_report, BatchReport};
use scanner::scan;
use std::path::{Path, PathBuf};
use watch::watch_folder;
use webhook::post_webhook;

const CONFIG_PATH: &str = "src/sample.toml";
const HISTORY_DIR: &str = ".mediaqa/history";

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
    Checklist(ChecklistCommand),
    Diff(DiffCommand),
    Serve(ServeCommand),
    Watch(WatchCommand),
}

#[derive(Args)]
struct CheckCommand {
    path: PathBuf,
    #[arg(long, default_value = "youtube")]
    profile: String,
    #[arg(long)]
    json: bool,
    #[arg(long)]
    html: Option<PathBuf>,
    #[arg(long)]
    save: bool,
    #[arg(long)]
    webhook: Option<String>,
    #[arg(long, default_value_t = true)]
    parallel: bool,
}

#[derive(Args)]
struct ProbeCommand {
    path: PathBuf,
    #[arg(long, default_value = "youtube")]
    profile: String,
    #[arg(long)]
    json: bool,
}

#[derive(Args)]
struct ChecklistCommand {
    #[arg(long, default_value = "youtube")]
    profile: String,
    #[arg(long)]
    output: Option<PathBuf>,
}

#[derive(Args)]
struct DiffCommand {
    before: PathBuf,
    after: PathBuf,
}

#[derive(Args)]
struct ServeCommand {
    #[arg(long, default_value_t = 8787)]
    port: u16,
    #[arg(long, default_value = HISTORY_DIR)]
    history_dir: PathBuf,
}

#[derive(Args)]
struct WatchCommand {
    path: PathBuf,
    #[arg(long, default_value = "youtube")]
    profile: String,
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
        Commands::Checklist(checklist) => {
            run_checklist(&config, checklist)?;
            0
        }
        Commands::Diff(diff) => {
            run_diff(diff)?;
            0
        }
        Commands::Serve(serve) => {
            api::serve(&serve.history_dir, serve.port)?;
            0
        }
        Commands::Watch(watch) => {
            watch_folder(
                watch.path,
                watch.profile,
                &config,
                BatchOptions { parallel: true },
            )?;
            0
        }
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

    let options = BatchOptions {
        parallel: check.parallel,
    };
    let report = run_batch(media_files, &profile, &check.profile, options);
    finish_report(&report, check.json, check.html.as_deref(), check.save, check.webhook.as_deref())?;
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
    let report = run_batch(
        vec![probe.path],
        &profile,
        &probe.profile,
        BatchOptions::default(),
    );
    render_report(&report, probe.json);
    Ok(report.exit_code())
}

fn run_checklist(
    config: &config::AppConfig,
    checklist: ChecklistCommand,
) -> Result<(), Box<dyn std::error::Error>> {
    let profile = load_profile(config, &checklist.profile)?;
    let text = render_checklist(&checklist.profile, &profile);

    if let Some(output) = checklist.output {
        std::fs::write(&output, &text)?;
        println!("Wrote checklist to {}", output.display());
    } else {
        println!("{text}");
    }

    Ok(())
}

fn run_diff(diff: DiffCommand) -> Result<(), Box<dyn std::error::Error>> {
    let before = load_report(&diff.before).map_err(|e| e.to_string())?;
    let after = load_report(&diff.after).map_err(|e| e.to_string())?;
    let report_diff = diff_reports(&before.report, &after.report);
    print_report_diff(&report_diff);
    Ok(())
}

fn finish_report(
    report: &BatchReport,
    json: bool,
    html: Option<&Path>,
    save: bool,
    webhook: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    render_report(report, json);

    if let Some(path) = html {
        write_html_report(report, path)?;
        println!("Wrote HTML report to {}", path.display());
    }

    if save {
        let stored = save_report(report, Path::new(HISTORY_DIR))?;
        println!("Saved report {}", stored.id);
    }

    if let Some(url) = webhook {
        post_webhook(url, report)?;
        println!("Posted webhook to {url}");
    }

    Ok(())
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
