use mediaqa::api::{serve as serve_api, ServeOptions};
use mediaqa::checklist::render_checklist;
use mediaqa::config::{load_config, Profile};
use mediaqa::diff::{diff_reports, print_report_diff};
use mediaqa::history::{load_report, save_report};
use mediaqa::html_report::write_html_report;
use mediaqa::pipeline::{run_batch, BatchOptions};
use mediaqa::report::{print_batch_report, print_json_report, BatchReport};
use mediaqa::scanner::scan;
use mediaqa::watch::watch_folder;
use mediaqa::webhook::post_webhook;
use mediaqa::{DEFAULT_CONFIG_PATH, DEFAULT_HISTORY_DIR, DEFAULT_STUDIO_DIR};
use clap::{Args, Parser, Subcommand};
use std::path::{Path, PathBuf};

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
    #[arg(long, default_value = DEFAULT_HISTORY_DIR)]
    history_dir: PathBuf,
    #[arg(long, default_value = DEFAULT_STUDIO_DIR)]
    studio_dir: PathBuf,
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
    let config = load_config(Path::new(DEFAULT_CONFIG_PATH))?;
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
        Commands::Serve(serve_cmd) => {
            serve_api(ServeOptions {
                history_dir: serve_cmd.history_dir,
                studio_dir: serve_cmd.studio_dir,
                upload_dir: PathBuf::from(".mediaqa/uploads"),
                port: serve_cmd.port,
                config_path: PathBuf::from(DEFAULT_CONFIG_PATH),
            })?;
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
    config: &mediaqa::config::AppConfig,
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
    finish_report(
        &report,
        check.json,
        check.html.as_deref(),
        check.save,
        check.webhook.as_deref(),
    )?;
    Ok(report.exit_code())
}

fn run_probe(
    config: &mediaqa::config::AppConfig,
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
    config: &mediaqa::config::AppConfig,
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
        let stored = save_report(report, Path::new(DEFAULT_HISTORY_DIR))?;
        println!("Saved report {}", stored.id);
    }

    if let Some(url) = webhook {
        post_webhook(url, report)?;
        println!("Posted webhook to {url}");
    }

    Ok(())
}

fn load_profile(
    config: &mediaqa::config::AppConfig,
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
