use crate::config::AppConfig;
use crate::pipeline::{run_batch, run_multi_profile_sequential_with_progress, BatchOptions};
use crate::report::{BatchReport, MultiProfileReport};
use crate::scanner::{is_media_candidate, scan};
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::channel;
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct WatchOptions {
    pub debounce: Duration,
    pub batch_options: BatchOptions,
    pub scan_existing: bool,
}

impl Default for WatchOptions {
    fn default() -> Self {
        Self {
            debounce: Duration::from_secs(5),
            batch_options: BatchOptions::default(),
            scan_existing: false,
        }
    }
}

#[derive(Debug, Clone)]
struct PendingWatch {
    last_size: u64,
    stable_since: Option<Instant>,
}

pub fn watch_folder(
    path: PathBuf,
    profile_name: String,
    config: &AppConfig,
    options: WatchOptions,
    mut on_report: impl FnMut(&BatchReport),
) -> Result<(), Box<dyn std::error::Error>> {
    let cancel = Arc::new(AtomicBool::new(false));
    println!("Press Ctrl+C to stop.");
    watch_folder_cancellable(
        path,
        profile_name,
        config,
        options,
        cancel,
        move |report| {
            on_report(report);
        },
        |message| println!("{message}"),
    )
}

pub fn watch_folder_cancellable(
    path: PathBuf,
    profile_name: String,
    config: &AppConfig,
    options: WatchOptions,
    cancel: Arc<AtomicBool>,
    mut on_report: impl FnMut(&BatchReport),
    mut on_status: impl FnMut(&str),
) -> Result<(), Box<dyn std::error::Error>> {
    if !path.exists() {
        return Err(format!("watch path does not exist: {}", path.display()).into());
    }
    if !path.is_dir() {
        return Err(format!("watch path must be a directory: {}", path.display()).into());
    }

    let profile = config
        .profiles
        .get(&profile_name)
        .cloned()
        .ok_or_else(|| format!("profile not found: {profile_name}"))?;

    on_status(&format!(
        "Watching {} for media drops (profile: {}, debounce: {:.0}s)",
        path.display(),
        profile_name,
        options.debounce.as_secs_f64()
    ));

    if options.scan_existing {
        run_existing_scan(&path, &profile, &profile_name, &options, &mut on_report)?;
    }

    let (tx, rx) = channel();
    let mut watcher = RecommendedWatcher::new(tx, notify::Config::default())?;
    watcher.watch(&path, RecursiveMode::Recursive)?;

    let mut pending: HashMap<PathBuf, PendingWatch> = HashMap::new();

    loop {
        if cancel.load(Ordering::Relaxed) {
            on_status("Watch stopped");
            break;
        }

        drain_watch_events(&rx, &mut pending);

        let ready = files_ready_after_debounce(&pending, Instant::now(), options.debounce);
        for ready_path in ready {
            pending.remove(&ready_path);
            on_status(&format!("New export ready: {}", ready_path.display()));
            let report = run_batch(
                vec![ready_path],
                &profile,
                &profile_name,
                options.batch_options,
            );
            on_report(&report);
        }

        std::thread::sleep(Duration::from_millis(250));
    }

    Ok(())
}

pub fn watch_folder_cancellable_multi(
    path: PathBuf,
    profile_names: Vec<String>,
    config: &AppConfig,
    options: WatchOptions,
    cancel: Arc<AtomicBool>,
    mut on_report: impl FnMut(&MultiProfileReport),
    mut on_status: impl FnMut(&str),
) -> Result<(), Box<dyn std::error::Error>> {
    if profile_names.is_empty() {
        return Err("at least one profile is required".into());
    }
    if !path.exists() {
        return Err(format!("watch path does not exist: {}", path.display()).into());
    }
    if !path.is_dir() {
        return Err(format!("watch path must be a directory: {}", path.display()).into());
    }

    let profile_label = profile_names.join(", ");
    on_status(&format!(
        "Watching {} for media drops (profiles: {}, debounce: {:.0}s)",
        path.display(),
        profile_label,
        options.debounce.as_secs_f64()
    ));

    if options.scan_existing {
        run_existing_scan_multi(&path, profile_names.clone(), config, &options, &mut on_report)?;
    }

    let (tx, rx) = channel();
    let mut watcher = RecommendedWatcher::new(tx, notify::Config::default())?;
    watcher.watch(&path, RecursiveMode::Recursive)?;

    let mut pending: HashMap<PathBuf, PendingWatch> = HashMap::new();

    loop {
        if cancel.load(Ordering::Relaxed) {
            on_status("Watch stopped");
            break;
        }

        drain_watch_events(&rx, &mut pending);

        let ready = files_ready_after_debounce(&pending, Instant::now(), options.debounce);
        for ready_path in ready {
            pending.remove(&ready_path);
            on_status(&format!("New export ready: {}", ready_path.display()));
            let report = run_multi_profile_sequential_with_progress(
                vec![ready_path],
                config,
                &profile_names,
                |_, _| {},
            )
            .map_err(|err| format!("{err}"))?;
            on_report(&report);
        }

        std::thread::sleep(Duration::from_millis(250));
    }

    Ok(())
}

fn run_existing_scan_multi(
    path: &Path,
    profile_names: Vec<String>,
    config: &AppConfig,
    options: &WatchOptions,
    on_report: &mut impl FnMut(&MultiProfileReport),
) -> Result<(), Box<dyn std::error::Error>> {
    let media_files = scan(path)?;
    if media_files.is_empty() {
        println!("No existing media files to check in {}", path.display());
        return Ok(());
    }

    println!(
        "Checking {} existing media file(s) in {}...",
        media_files.len(),
        path.display()
    );
    let report = run_multi_profile_sequential_with_progress(
        media_files,
        config,
        &profile_names,
        |_, _| {},
    )
    .map_err(|err| format!("{err}"))?;
    on_report(&report);
    Ok(())
}

fn run_existing_scan(
    path: &Path,
    profile: &crate::config::Profile,
    profile_name: &str,
    options: &WatchOptions,
    on_report: &mut impl FnMut(&BatchReport),
) -> Result<(), Box<dyn std::error::Error>> {
    let media_files = scan(path)?;
    if media_files.is_empty() {
        println!("No existing media files to check in {}", path.display());
        return Ok(());
    }

    println!(
        "Checking {} existing media file(s) in {}...",
        media_files.len(),
        path.display()
    );
    let report = run_batch(
        media_files,
        profile,
        profile_name,
        options.batch_options,
    );
    on_report(&report);
    Ok(())
}

fn drain_watch_events(
    rx: &std::sync::mpsc::Receiver<Result<notify::Event, notify::Error>>,
    pending: &mut HashMap<PathBuf, PendingWatch>,
) {
    loop {
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(Ok(event)) => {
                if is_relevant_event(&event.kind) {
                    for path in event.paths {
                        note_watch_path(&path, pending);
                    }
                }
            }
            Ok(Err(err)) => eprintln!("watch error: {err}"),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => break,
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
}

fn is_relevant_event(kind: &EventKind) -> bool {
    matches!(
        kind,
        EventKind::Create(_) | EventKind::Modify(_) | EventKind::Any
    )
}

fn note_watch_path(path: &Path, pending: &mut HashMap<PathBuf, PendingWatch>) {
    if !is_media_watch_target(path) {
        return;
    }

    let size = file_size(path).unwrap_or(0);
    let entry = pending.entry(path.to_path_buf()).or_insert(PendingWatch {
        last_size: size,
        stable_since: None,
    });

    if entry.last_size != size {
        entry.last_size = size;
        entry.stable_since = None;
        return;
    }

    if size == 0 {
        entry.stable_since = None;
        return;
    }

    if entry.stable_since.is_none() {
        entry.stable_since = Some(Instant::now());
    }
}

pub fn is_media_watch_target(path: &Path) -> bool {
    path.is_file() && is_media_candidate(path)
}

pub fn files_ready_after_debounce(
    pending: &HashMap<PathBuf, PendingWatch>,
    now: Instant,
    debounce: Duration,
) -> Vec<PathBuf> {
    let mut ready: Vec<PathBuf> = pending
        .iter()
        .filter_map(|(path, state)| {
            let stable_since = state.stable_since?;
            if state.last_size == 0 {
                return None;
            }
            if now.duration_since(stable_since) >= debounce {
                Some(path.clone())
            } else {
                None
            }
        })
        .collect();

    ready.sort();
    ready
}

fn file_size(path: &Path) -> Option<u64> {
    std::fs::metadata(path).ok().map(|meta| meta.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_media_watch_target_requires_media_extension() {
        let dir = std::env::temp_dir().join(format!("mediaqa_watch_{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("dir");
        let media = dir.join("clip.mp4");
        std::fs::write(&media, b"stub").expect("write media");
        let text = dir.join("notes.txt");
        std::fs::write(&text, b"stub").expect("write text");

        assert!(is_media_watch_target(&media));
        assert!(!is_media_watch_target(&text));

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn files_ready_after_debounce_waits_for_stable_window() {
        let path = PathBuf::from("clip.mp4");
        let started = Instant::now();
        let pending = HashMap::from([(
            path.clone(),
            PendingWatch {
                last_size: 1024,
                stable_since: Some(started),
            },
        )]);

        assert!(files_ready_after_debounce(
            &pending,
            started + Duration::from_secs(4),
            Duration::from_secs(5)
        )
        .is_empty());

        let ready = files_ready_after_debounce(
            &pending,
            started + Duration::from_secs(6),
            Duration::from_secs(5),
        );
        assert_eq!(ready, vec![path]);
    }

    #[test]
    fn zero_byte_files_are_not_ready() {
        let pending = HashMap::from([(
            PathBuf::from("incomplete.mp4"),
            PendingWatch {
                last_size: 0,
                stable_since: Some(Instant::now()),
            },
        )]);

        assert!(files_ready_after_debounce(
            &pending,
            Instant::now() + Duration::from_secs(10),
            Duration::from_secs(1)
        )
        .is_empty());
    }
}
