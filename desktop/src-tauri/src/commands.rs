use mediaqa::pipeline::{run_multi_profile_sequential_with_progress, BatchOptions};
use mediaqa::report::MultiProfileReport;
use mediaqa::scanner::scan;
use mediaqa::watch::{watch_folder_cancellable_multi, WatchOptions};
use mediaqa::{
    create_user_profile, delete_user_profile, list_profile_summaries, load_default_config,
    CreateProfileInput, ProfileSummary,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter, State};

#[derive(Default)]
pub struct WatchState {
    inner: Mutex<Option<WatchRuntime>>,
}

struct WatchRuntime {
    cancel: Arc<AtomicBool>,
    path: String,
    profiles: Vec<String>,
    debounce_secs: u64,
    scan_existing: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchStatus {
    pub active: bool,
    pub path: Option<String>,
    pub profiles: Vec<String>,
    pub debounce_secs: Option<u64>,
    pub scan_existing: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LocalQcResult {
    pub exit_code: i32,
    pub report_json: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct QcProgressPayload {
    pub phase: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
}

fn emit_qc_progress(app: &AppHandle, payload: QcProgressPayload) {
    let _ = app.emit_to("main", "qc-progress", payload);
}

#[tauri::command]
pub async fn run_local_qc(
    path: String,
    profiles: Vec<String>,
    app: AppHandle,
) -> Result<LocalQcResult, String> {
    tauri::async_runtime::spawn_blocking(move || run_local_qc_blocking(path, profiles, app))
        .await
        .map_err(|err| err.to_string())?
}

fn run_local_qc_blocking(
    path: String,
    profiles: Vec<String>,
    app: AppHandle,
) -> Result<LocalQcResult, String> {
    if profiles.is_empty() {
        return Err("Select at least one delivery profile".into());
    }

    emit_qc_progress(
        &app,
        QcProgressPayload {
            phase: "scan".into(),
            message: "Scanning folder for media files…".into(),
            current: None,
            total: None,
            profile: None,
        },
    );

    let config = load_default_config().map_err(|e| e.to_string())?;

    let media_files = scan(std::path::Path::new(&path)).map_err(|e| e.to_string())?;
    if media_files.is_empty() {
        return Err("No media files found".into());
    }

    emit_qc_progress(
        &app,
        QcProgressPayload {
            phase: "scanned".into(),
            message: format!("Found {} media file(s)", media_files.len()),
            current: None,
            total: Some(media_files.len()),
            profile: None,
        },
    );

    let app_handle = app.clone();
    let multi = run_multi_profile_sequential_with_progress(
        media_files,
        &config,
        &profiles,
        |progress, profile_name| {
            emit_qc_progress(
                &app_handle,
                QcProgressPayload {
                    phase: progress.phase.to_string(),
                    message: progress.message,
                    current: if progress.total > 0 {
                        Some(progress.current)
                    } else {
                        None
                    },
                    total: if progress.total > 0 {
                        Some(progress.total)
                    } else {
                        None
                    },
                    profile: Some(profile_name.to_string()),
                },
            );
        },
    )?;

    emit_qc_progress(
        &app,
        QcProgressPayload {
            phase: "done".into(),
            message: "Check complete".into(),
            current: Some(multi.reports.first().map(|r| r.summary.total).unwrap_or(0)),
            total: Some(multi.reports.first().map(|r| r.summary.total).unwrap_or(0)),
            profile: None,
        },
    );

    let report_json = serde_json::to_string_pretty(&multi).map_err(|e| e.to_string())?;
    Ok(LocalQcResult {
        exit_code: multi.exit_code(),
        report_json,
    })
}

#[derive(Debug, Clone, Serialize)]
struct WatchStatusEvent {
    message: String,
}

#[derive(Debug, Clone, Serialize)]
struct WatchReportEvent {
    report_json: String,
    exit_code: i32,
    trigger: String,
}

fn emit_watch_status(app: &AppHandle, message: impl Into<String>) {
    let _ = app.emit_to(
        "main",
        "watch-status",
        WatchStatusEvent {
            message: message.into(),
        },
    );
}

fn emit_watch_report(app: &AppHandle, report: &MultiProfileReport, trigger: &str) {
    let report_json = serde_json::to_string_pretty(report).unwrap_or_else(|_| "{}".into());
    let _ = app.emit_to(
        "main",
        "watch-report",
        WatchReportEvent {
            report_json,
            exit_code: report.exit_code(),
            trigger: trigger.to_string(),
        },
    );
}

#[tauri::command]
pub fn list_profiles() -> Result<Vec<ProfileSummary>, String> {
    list_profile_summaries().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn create_profile(input: CreateProfileInput) -> Result<ProfileSummary, String> {
    create_user_profile(input).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_profile(name: String) -> Result<String, String> {
    delete_user_profile(&name).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn start_watch_folder(
    path: String,
    profiles: Vec<String>,
    debounce_secs: u64,
    scan_existing: bool,
    app: AppHandle,
    state: State<'_, WatchState>,
) -> Result<WatchStatus, String> {
    if profiles.is_empty() {
        return Err("Select at least one delivery profile".into());
    }

    stop_watch_internal(&state)?;

    let watch_path = PathBuf::from(&path);
    if !watch_path.is_dir() {
        return Err("Watch path must be an existing folder".into());
    }

    let cancel = Arc::new(AtomicBool::new(false));
    let cancel_flag = cancel.clone();
    let app_handle = app.clone();
    let profile_names = profiles.clone();

    std::thread::spawn(move || {
        let config = match load_default_config() {
            Ok(config) => config,
            Err(err) => {
                emit_watch_status(&app_handle, format!("Watch failed to start: {err}"));
                return;
            }
        };

        let options = WatchOptions {
            debounce: Duration::from_secs(debounce_secs.max(1)),
            batch_options: BatchOptions { parallel: true },
            scan_existing,
        };

        if let Err(err) = watch_folder_cancellable_multi(
            watch_path,
            profile_names,
            &config,
            options,
            cancel_flag,
            |report| emit_watch_report(&app_handle, report, "drop"),
            |message| emit_watch_status(&app_handle, message),
        ) {
            emit_watch_status(&app_handle, format!("Watch error: {err}"));
        }
    });

    *state.inner.lock().unwrap() = Some(WatchRuntime {
        cancel,
        path,
        profiles,
        debounce_secs,
        scan_existing,
    });

    Ok(current_watch_status(&state))
}

#[tauri::command]
pub fn stop_watch_folder(state: State<'_, WatchState>) -> Result<WatchStatus, String> {
    stop_watch_internal(&state)?;
    Ok(current_watch_status(&state))
}

#[tauri::command]
pub fn get_watch_status(state: State<'_, WatchState>) -> WatchStatus {
    current_watch_status(&state)
}

fn stop_watch_internal(state: &WatchState) -> Result<(), String> {
    let runtime = state.inner.lock().unwrap().take();
    if let Some(runtime) = runtime {
        runtime.cancel.store(true, Ordering::Relaxed);
    }
    Ok(())
}

fn current_watch_status(state: &WatchState) -> WatchStatus {
    let guard = state.inner.lock().unwrap();
    match guard.as_ref() {
        Some(runtime) => WatchStatus {
            active: true,
            path: Some(runtime.path.clone()),
            profiles: runtime.profiles.clone(),
            debounce_secs: Some(runtime.debounce_secs),
            scan_existing: runtime.scan_existing,
        },
        None => WatchStatus {
            active: false,
            path: None,
            profiles: Vec::new(),
            debounce_secs: None,
            scan_existing: false,
        },
    }
}
