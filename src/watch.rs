use crate::config::AppConfig;
use crate::pipeline::{run_batch_parallel, BatchOptions};
use crate::report::print_batch_report;
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::PathBuf;
use std::sync::mpsc::channel;
use std::time::Duration;

pub fn watch_folder(
    path: PathBuf,
    profile_name: String,
    config: &AppConfig,
    options: BatchOptions,
) -> Result<(), Box<dyn std::error::Error>> {
    let profile = config
        .profiles
        .get(&profile_name)
        .cloned()
        .ok_or("Profile not found")?;

    println!("Watching {} for new media files...", path.display());

    let (tx, rx) = channel();
    let mut watcher = RecommendedWatcher::new(tx, notify::Config::default())?;
    watcher.watch(&path, RecursiveMode::Recursive)?;

    loop {
        match rx.recv_timeout(Duration::from_secs(1)) {
            Ok(Ok(event)) => {
                if matches!(event.kind, EventKind::Create(_)) {
                    for file in event.paths {
                        if file.is_file() {
                            println!("\nNew file detected: {}", file.display());
                            let report = run_batch_parallel(vec![file], &profile, &profile_name, options);
                            print_batch_report(&report);
                        }
                    }
                }
            }
            Ok(Err(err)) => eprintln!("watch error: {err}"),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }

    Ok(())
}
