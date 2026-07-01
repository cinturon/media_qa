use mediaqa::config::load_config;
use mediaqa::pipeline::{run_batch, BatchOptions};
use mediaqa::scanner::scan;
use mediaqa::{DEFAULT_CONFIG_PATH, DEFAULT_STUDIO_DIR};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::State;

const API_BASE: &str = "http://127.0.0.1:8787";
const QUEUE_PATH: &str = ".mediaqa/desktop-queue.json";

#[derive(Default)]
pub struct AppContext {
    pub session_token: Mutex<Option<String>>,
    pub api_base: Mutex<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LocalQcResult {
    pub exit_code: i32,
    pub report_json: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Entitlements {
    pub plan: String,
    pub features: serde_json::Value,
    pub usage: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OfflineAction {
    pub id: String,
    pub kind: String,
    pub payload: serde_json::Value,
    pub synced: bool,
}

#[tauri::command]
pub fn run_local_qc(path: String, profile: String) -> Result<LocalQcResult, String> {
    let config = load_config(std::path::Path::new(DEFAULT_CONFIG_PATH)).map_err(|e| e.to_string())?;
    let profile_cfg = config
        .profiles
        .get(&profile)
        .cloned()
        .ok_or_else(|| format!("Profile not found: {profile}"))?;
    let media_files = scan(std::path::Path::new(&path)).map_err(|e| e.to_string())?;
    if media_files.is_empty() {
        return Err("No media files found".into());
    }
    let report = run_batch(media_files, &profile_cfg, &profile, BatchOptions::default());
    let report_json = serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?;
    Ok(LocalQcResult {
        exit_code: report.exit_code(),
        report_json,
    })
}

#[tauri::command]
pub fn login(api_token: String, state: State<'_, AppContext>) -> Result<serde_json::Value, String> {
    let response = ureq::post(&format!("{API_BASE}/auth/login"))
        .set("Content-Type", "application/json")
        .send_json(serde_json::json!({ "api_token": api_token }))
        .map_err(|e| e.to_string())?;
    let session: serde_json::Value = response.into_json().map_err(|e| e.to_string())?;
    if let Some(token) = session.get("token").and_then(|v| v.as_str()) {
        *state.session_token.lock().unwrap() = Some(token.to_string());
    }
    Ok(session)
}

#[tauri::command]
pub fn list_workspaces(state: State<'_, AppContext>) -> Result<serde_json::Value, String> {
    authed_get(state, "/workspaces")
}

#[tauri::command]
pub fn switch_workspace(
    workspace_id: String,
    state: State<'_, AppContext>,
) -> Result<serde_json::Value, String> {
    authed_post(
        state,
        "/workspaces/switch",
        serde_json::json!({ "workspace_id": workspace_id }),
    )
}

#[tauri::command]
pub fn get_entitlements(state: State<'_, AppContext>) -> Result<Entitlements, String> {
    let plans = authed_get(state.clone(), "/plans")?;
    let usage = authed_get(state, "/usage")?;
    Ok(Entitlements {
        plan: plans
            .get("current_plan")
            .and_then(|v| v.as_str())
            .unwrap_or("free")
            .to_string(),
        features: plans
            .get("features")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({})),
        usage,
    })
}

#[tauri::command]
pub fn get_pricing_variant(state: State<'_, AppContext>) -> Result<serde_json::Value, String> {
    authed_get(state, "/experiments/pricing")
}

#[tauri::command]
pub fn submit_remote_job(
    upload_id: String,
    profile: String,
    state: State<'_, AppContext>,
) -> Result<serde_json::Value, String> {
    authed_post(
        state,
        "/jobs",
        serde_json::json!({ "upload_id": upload_id, "profile": profile }),
    )
}

#[tauri::command]
pub fn poll_remote_job(job_id: String, state: State<'_, AppContext>) -> Result<serde_json::Value, String> {
    authed_get(state, &format!("/jobs/{job_id}"))
}

#[tauri::command]
pub fn queue_offline_action(kind: String, payload: serde_json::Value) -> Result<OfflineAction, String> {
    let mut queue = load_queue()?;
    let action = OfflineAction {
        id: format!("offline_{}", queue.len() + 1),
        kind,
        payload,
        synced: false,
    };
    queue.push(action.clone());
    save_queue(&queue)?;
    Ok(action)
}

#[tauri::command]
pub fn get_offline_queue() -> Result<Vec<OfflineAction>, String> {
    load_queue()
}

#[tauri::command]
pub fn sync_offline_queue(state: State<'_, AppContext>) -> Result<Vec<OfflineAction>, String> {
    let mut queue = load_queue()?;
    for action in queue.iter_mut().filter(|a| !a.synced) {
        if action.kind == "remote_job" {
            let upload_id = action
                .payload
                .get("upload_id")
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            let profile = action
                .payload
                .get("profile")
                .and_then(|v| v.as_str())
                .unwrap_or("youtube");
            let _ = submit_remote_job(upload_id.to_string(), profile.to_string(), state.clone());
            action.synced = true;
        }
    }
    save_queue(&queue)?;
    Ok(queue)
}

fn authed_get(state: State<'_, AppContext>, path: &str) -> Result<serde_json::Value, String> {
    let token = session_token(&state)?;
    let base = api_base(&state);
    let response = ureq::get(&format!("{base}{path}"))
        .set("Authorization", &format!("Bearer {token}"))
        .call()
        .map_err(|e| e.to_string())?;
    response.into_json().map_err(|e| e.to_string())
}

fn authed_post(
    state: State<'_, AppContext>,
    path: &str,
    body: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let token = session_token(&state)?;
    let base = api_base(&state);
    let response = ureq::post(&format!("{base}{path}"))
        .set("Authorization", &format!("Bearer {token}"))
        .set("Content-Type", "application/json")
        .send_json(body)
        .map_err(|e| e.to_string())?;
    response.into_json().map_err(|e| e.to_string())
}

fn session_token(state: &State<'_, AppContext>) -> Result<String, String> {
    state
        .session_token
        .lock()
        .unwrap()
        .clone()
        .ok_or_else(|| "Not logged in".to_string())
}

fn api_base(state: &State<'_, AppContext>) -> String {
    let value = state.api_base.lock().unwrap().clone();
    if value.is_empty() {
        API_BASE.to_string()
    } else {
        value
    }
}

fn load_queue() -> Result<Vec<OfflineAction>, String> {
    let path = PathBuf::from(QUEUE_PATH);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let raw = fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str(&raw).map_err(|e| e.to_string())
}

fn save_queue(queue: &[OfflineAction]) -> Result<(), String> {
    fs::create_dir_all(".mediaqa").map_err(|e| e.to_string())?;
    let raw = serde_json::to_string_pretty(queue).map_err(|e| e.to_string())?;
    fs::write(QUEUE_PATH, raw).map_err(|e| e.to_string())
}

// Seed note for studio store path constant usage in docs
const _STUDIO_DIR: &str = DEFAULT_STUDIO_DIR;
