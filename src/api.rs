use crate::history::{list_reports, load_report, save_report, StoredReport};
use crate::pipeline::{run_batch, BatchOptions};
use crate::report::BatchReport;
use crate::studio::admin::{build_admin_overview, support_snapshot};
use crate::studio::auth::{login_with_token, switch_workspace};
use crate::studio::billing::usage_summary;
use crate::studio::experiments::assign_variant;
use crate::studio::invites::{accept_invite, create_invite};
use crate::studio::jobs::{advance_job, queue_job};
use crate::studio::plans::{gate_feature, within_run_limit};
use crate::studio::retention::apply_retention;
use crate::studio::store::StudioStore;
use crate::studio::uploads::create_upload;
use crate::config::{load_config, Profile};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread;
use tiny_http::{Header, Method, Request, Response, Server, StatusCode};

pub struct ServeOptions {
    pub history_dir: PathBuf,
    pub studio_dir: PathBuf,
    pub upload_dir: PathBuf,
    pub port: u16,
    pub config_path: PathBuf,
}

impl Default for ServeOptions {
    fn default() -> Self {
        Self {
            history_dir: PathBuf::from(crate::DEFAULT_HISTORY_DIR),
            studio_dir: PathBuf::from(crate::DEFAULT_STUDIO_DIR),
            upload_dir: PathBuf::from(".mediaqa/uploads"),
            port: 8787,
            config_path: PathBuf::from(crate::DEFAULT_CONFIG_PATH),
        }
    }
}

pub fn serve(options: ServeOptions) -> Result<(), String> {
    let server = Server::http(format!("127.0.0.1:{}", options.port)).map_err(|e| e.to_string())?;
    let state = Arc::new(AppState::new(options)?);

    println!("Studio API listening on http://127.0.0.1:{}", state.options.port);
    println!("  Auth: POST /auth/login");
    println!("  Workspaces: GET /workspaces, POST /workspaces/switch");
    println!("  Reports: GET /reports, GET /reports/{{id}}");
    println!("  Uploads: POST /uploads, Jobs: POST /jobs, GET /jobs/{{id}}");
    println!("  Billing: GET /usage, Plans: GET /plans");
    println!("  Admin: GET /admin/overview, GET /admin/support/{{workspace}}");

    for request in server.incoming_requests() {
        let state = Arc::clone(&state);
        thread::spawn(move || handle_request(request, state));
    }

    Ok(())
}

struct AppState {
    options: ServeOptions,
    store: StudioStore,
}

impl AppState {
    fn new(options: ServeOptions) -> Result<Self, String> {
        fs::create_dir_all(&options.upload_dir).map_err(|e| e.to_string())?;
        Ok(Self {
            store: StudioStore::open(&options.studio_dir)?,
            options,
        })
    }
}

fn handle_request(mut request: Request, state: Arc<AppState>) {
    let method = request.method().clone();
    let path = request.url().to_string();
    let auth = request
        .headers()
        .iter()
        .find(|h| h.field.equiv("Authorization"))
        .and_then(|h| h.value.as_str().strip_prefix("Bearer "))
        .map(str::to_string);
    let body = if method == Method::Post {
        read_body(&mut request)
    } else {
        String::new()
    };

    let response = match (method, path.as_str()) {
        (Method::Post, "/auth/login") => handle_login(&state, &body),
        (Method::Get, "/workspaces") => with_session(&state, auth, handle_list_workspaces),
        (Method::Post, "/workspaces/switch") => {
            with_session_body(&state, auth, body.as_str(), handle_switch_workspace)
        }
        (Method::Get, "/reports") => with_session(&state, auth, |state, session| {
            handle_list_reports(&state.options.history_dir, session)
        }),
        (Method::Get, path) if path.starts_with("/reports/") => {
            let id = path.trim_start_matches("/reports/");
            with_session(&state, auth, |state, _session| {
                handle_load_report(&state.options.history_dir, id)
            })
        }
        (Method::Post, "/uploads") => with_session_body(&state, auth, body.as_str(), handle_upload),
        (Method::Post, "/jobs") => with_session_body(&state, auth, body.as_str(), handle_create_job),
        (Method::Get, path) if path.starts_with("/jobs/") => {
            let id = path.trim_start_matches("/jobs/");
            with_session(&state, auth, |state, _session| handle_get_job(state, id))
        }
        (Method::Post, path) if path.starts_with("/jobs/") && path.ends_with("/advance") => {
            let id = path
                .trim_start_matches("/jobs/")
                .trim_end_matches("/advance");
            with_session(&state, auth, |state, _session| handle_advance_job(state, id))
        }
        (Method::Get, "/usage") => {
            with_session(&state, auth, |state, session| handle_usage(state, session))
        }
        (Method::Get, "/plans") => {
            with_session(&state, auth, |state, session| handle_plans(state, session))
        }
        (Method::Post, "/invites") => {
            with_session_body(&state, auth, body.as_str(), handle_create_invite)
        }
        (Method::Post, "/invites/accept") => {
            with_session_body(&state, auth, body.as_str(), handle_accept_invite)
        }
        (Method::Get, "/experiments/pricing") => {
            with_session(&state, auth, |state, session| handle_pricing_experiment(state, session))
        }
        (Method::Get, "/admin/overview") => {
            with_session(&state, auth, |state, _session| handle_admin_overview(state))
        }
        (Method::Get, path) if path.starts_with("/admin/support/") => {
            let workspace_id = path.trim_start_matches("/admin/support/");
            with_session(&state, auth, |state, _session| {
                handle_admin_support(state, workspace_id)
            })
        }
        (Method::Post, "/retention/apply") => {
            with_session(&state, auth, |state, _session| handle_apply_retention(state))
        }
        _ => json_error("Not Found", StatusCode(404)),
    };

    let _ = request.respond(response);
}

fn read_body(request: &mut Request) -> String {
    use std::io::Read;
    let mut body = String::new();
    let _ = request.as_reader().read_to_string(&mut body);
    body
}

fn with_session(
    state: &AppState,
    auth: Option<String>,
    handler: impl FnOnce(&AppState, &crate::studio::auth::Session) -> Response<std::io::Cursor<Vec<u8>>>,
) -> Response<std::io::Cursor<Vec<u8>>> {
    let token = match auth {
        Some(token) => token,
        None => return json_error("Missing Authorization header", StatusCode(401)),
    };

    let session = match state.store.read(|data| data.session_by_token(&token).cloned()) {
        Ok(Some(session)) => session,
        Ok(None) => return json_error("Invalid session", StatusCode(401)),
        Err(err) => return json_error(&err, StatusCode(500)),
    };

    handler(state, &session)
}

fn with_session_body(
    state: &AppState,
    auth: Option<String>,
    body: &str,
    handler: impl FnOnce(
        &AppState,
        &crate::studio::auth::Session,
        &str,
    ) -> Response<std::io::Cursor<Vec<u8>>>,
) -> Response<std::io::Cursor<Vec<u8>>> {
    let token = match auth {
        Some(token) => token,
        None => return json_error("Missing Authorization header", StatusCode(401)),
    };

    let session = match state.store.read(|data| data.session_by_token(&token).cloned()) {
        Ok(Some(session)) => session,
        Ok(None) => return json_error("Invalid session", StatusCode(401)),
        Err(err) => return json_error(&err, StatusCode(500)),
    };

    handler(state, &session, body)
}

fn handle_login(state: &AppState, body: &str) -> Response<std::io::Cursor<Vec<u8>>> {
    #[derive(serde::Deserialize)]
    struct LoginRequest {
        api_token: String,
    }

    let request: LoginRequest = match serde_json::from_str(body) {
        Ok(value) => value,
        Err(_) => return json_error("Invalid JSON body", StatusCode(400)),
    };

    match state.store.read(|data| login_with_token(&data.users, &data.sessions, &request.api_token)) {
        Ok(Some(session)) => json_response(&session),
        Ok(None) => json_error("Invalid API token", StatusCode(401)),
        Err(err) => json_error(&err, StatusCode(500)),
    }
}

fn handle_list_workspaces(
    state: &AppState,
    _session: &crate::studio::auth::Session,
) -> Response<std::io::Cursor<Vec<u8>>> {
    match state.store.read(|data| data.workspaces.clone()) {
        Ok(workspaces) => json_response(&workspaces),
        Err(err) => json_error(&err, StatusCode(500)),
    }
}

fn handle_switch_workspace(
    state: &AppState,
    session: &crate::studio::auth::Session,
    body: &str,
) -> Response<std::io::Cursor<Vec<u8>>> {
    #[derive(serde::Deserialize)]
    struct SwitchRequest {
        workspace_id: String,
    }

    let request: SwitchRequest = match serde_json::from_str(body) {
        Ok(value) => value,
        Err(_) => return json_error("Invalid JSON body", StatusCode(400)),
    };

    match state.store.with_data(|data| {
        let exists = data.workspaces.iter().any(|w| w.id == request.workspace_id);
        if !exists {
            return Err("Workspace not found".into());
        }
        let session = data
            .sessions
            .iter_mut()
            .find(|s| s.token == session.token)
            .ok_or_else(|| "Session not found".to_string())?;
        switch_workspace(session, &request.workspace_id);
        Ok(session.clone())
    }) {
        Ok(session) => json_response(&session),
        Err(err) => json_error(&err, StatusCode(400)),
    }
}

fn handle_list_reports(
    history_dir: &Path,
    session: &crate::studio::auth::Session,
) -> Response<std::io::Cursor<Vec<u8>>> {
    match list_reports(history_dir) {
        Ok(paths) => {
            let reports: Vec<StoredReport> = paths
                .iter()
                .filter_map(|path| load_report(path).ok())
                .filter(|stored| stored.report.profile.contains(&session.workspace_id) || true)
                .collect();
            let ids: Vec<String> = reports.into_iter().map(|r| r.id).collect();
            json_response(&ids)
        }
        Err(err) => json_error(&err, StatusCode(500)),
    }
}

fn handle_load_report(history_dir: &Path, id: &str) -> Response<std::io::Cursor<Vec<u8>>> {
    let path = history_dir.join(format!("{id}.json"));
    match load_report(&path) {
        Ok(stored) => json_response(&stored),
        Err(err) => json_error(&err, StatusCode(404)),
    }
}

fn handle_upload(
    state: &AppState,
    session: &crate::studio::auth::Session,
    body: &str,
) -> Response<std::io::Cursor<Vec<u8>>> {
    #[derive(serde::Deserialize)]
    struct UploadRequest {
        filename: String,
        content_base64: String,
    }

    let request: UploadRequest = match serde_json::from_str(body) {
        Ok(value) => value,
        Err(_) => return json_error("Invalid JSON body", StatusCode(400)),
    };

    let bytes = match base64_decode(&request.content_base64) {
        Ok(bytes) => bytes,
        Err(err) => return json_error(&err, StatusCode(400)),
    };

    let workspace_dir = state.options.upload_dir.join(&session.workspace_id);
    if let Err(err) = fs::create_dir_all(&workspace_dir) {
        return json_error(&err.to_string(), StatusCode(500));
    }

    let stored_path = workspace_dir.join(&request.filename);
    if let Err(err) = fs::write(&stored_path, &bytes) {
        return json_error(&err.to_string(), StatusCode(500));
    }

    match state.store.with_data(|data| {
        let workspace = data
            .workspace(&session.workspace_id)
            .ok_or_else(|| "Workspace not found".to_string())?;
        if !gate_feature(workspace.plan, "remote_jobs") {
            return Err("Plan does not include remote uploads".into());
        }
        data.usage
            .record_storage(&session.workspace_id, bytes.len() as u64);
        let upload = create_upload(
            &session.workspace_id,
            &request.filename,
            stored_path,
            bytes.len() as u64,
        );
        data.uploads.push(upload.clone());
        Ok(upload)
    }) {
        Ok(upload) => json_response(&upload),
        Err(err) => json_error(&err, StatusCode(403)),
    }
}

fn handle_create_job(
    state: &AppState,
    session: &crate::studio::auth::Session,
    body: &str,
) -> Response<std::io::Cursor<Vec<u8>>> {
    #[derive(serde::Deserialize)]
    struct JobRequest {
        upload_id: String,
        profile: String,
    }

    let request: JobRequest = match serde_json::from_str(body) {
        Ok(value) => value,
        Err(_) => return json_error("Invalid JSON body", StatusCode(400)),
    };

    let config_path = state.options.config_path.clone();
    let history_dir = state.options.history_dir.clone();

    match state.store.with_data(|data| {
        let workspace = data
            .workspace(&session.workspace_id)
            .ok_or_else(|| "Workspace not found".to_string())?;
        if !within_run_limit(workspace.plan, data.usage.runs_this_month) {
            return Err("Monthly run limit reached".into());
        }
        if !gate_feature(workspace.plan, "remote_jobs") {
            return Err("Plan does not include remote jobs".into());
        }
        let upload = data
            .uploads
            .iter()
            .find(|upload| upload.id == request.upload_id)
            .ok_or_else(|| "Upload not found".to_string())?;
        let job = queue_job(&session.workspace_id, &upload.id, &request.profile);
        data.jobs.push(job.clone());
        data.usage.record_run(&session.workspace_id);
        Ok((job, upload.stored_path.clone()))
    }) {
        Ok((job, media_path)) => {
            let job_id = job.id.clone();
            if let Ok(report) = run_remote_qc(&config_path, &media_path, &job.profile) {
                let stored = save_report(&report, &history_dir).ok();
                let _ = state.store.with_data(|data| {
                    if let Some(job) = data.jobs.iter_mut().find(|j| j.id == job_id) {
                        advance_job(job);
                        advance_job(job);
                        advance_job(job);
                        if let Some(stored) = &stored {
                            job.report_id = Some(stored.id.clone());
                        }
                    }
                    Ok::<(), String>(())
                });
            }
            json_response(&job)
        }
        Err(err) => json_error(&err, StatusCode(403)),
    }
}

fn handle_get_job(state: &AppState, id: &str) -> Response<std::io::Cursor<Vec<u8>>> {
    match state.store.read(|data| data.jobs.iter().find(|job| job.id == id).cloned()) {
        Ok(Some(job)) => json_response(&job),
        Ok(None) => json_error("Job not found", StatusCode(404)),
        Err(err) => json_error(&err, StatusCode(500)),
    }
}

fn handle_advance_job(state: &AppState, id: &str) -> Response<std::io::Cursor<Vec<u8>>> {
    match state.store.with_data(|data| {
        let job = data
            .jobs
            .iter_mut()
            .find(|job| job.id == id)
            .ok_or_else(|| "Job not found".to_string())?;
        advance_job(job);
        Ok(job.clone())
    }) {
        Ok(job) => json_response(&job),
        Err(err) => json_error(&err, StatusCode(404)),
    }
}

fn handle_usage(
    state: &AppState,
    session: &crate::studio::auth::Session,
) -> Response<std::io::Cursor<Vec<u8>>> {
    match state.store.read(|data| {
        let workspace = data.workspace(&session.workspace_id)?;
        Some((workspace.plan, usage_summary(&data.usage)))
    }) {
        Ok(Some((plan, usage))) => json_response(&serde_json::json!({
            "workspace_id": session.workspace_id,
            "plan": plan,
            "usage": usage,
        })),
        Ok(None) => json_error("Workspace not found", StatusCode(404)),
        Err(err) => json_error(&err, StatusCode(500)),
    }
}

fn handle_plans(
    state: &AppState,
    session: &crate::studio::auth::Session,
) -> Response<std::io::Cursor<Vec<u8>>> {
    match state.store.read(|data| data.workspace(&session.workspace_id).map(|w| w.plan)) {
        Ok(Some(plan)) => json_response(&serde_json::json!({
            "current_plan": plan,
            "features": plan.features(),
        })),
        Ok(None) => json_error("Workspace not found", StatusCode(404)),
        Err(err) => json_error(&err, StatusCode(500)),
    }
}

fn handle_create_invite(
    state: &AppState,
    session: &crate::studio::auth::Session,
    body: &str,
) -> Response<std::io::Cursor<Vec<u8>>> {
    #[derive(serde::Deserialize)]
    struct InviteRequest {
        email: String,
    }

    let request: InviteRequest = match serde_json::from_str(body) {
        Ok(value) => value,
        Err(_) => return json_error("Invalid JSON body", StatusCode(400)),
    };

    match state.store.with_data(|data| {
        let invite = create_invite(&session.workspace_id, &request.email);
        data.invites.push(invite.clone());
        Ok(invite)
    }) {
        Ok(invite) => json_response(&invite),
        Err(err) => json_error(&err, StatusCode(500)),
    }
}

fn handle_accept_invite(
    state: &AppState,
    _session: &crate::studio::auth::Session,
    body: &str,
) -> Response<std::io::Cursor<Vec<u8>>> {
    #[derive(serde::Deserialize)]
    struct AcceptRequest {
        token: String,
    }

    let request: AcceptRequest = match serde_json::from_str(body) {
        Ok(value) => value,
        Err(_) => return json_error("Invalid JSON body", StatusCode(400)),
    };

    match state.store.with_data(|data| {
        let invite = data
            .invites
            .iter_mut()
            .find(|invite| invite.token == request.token)
            .ok_or_else(|| "Invite not found".to_string())?;
        accept_invite(invite);
        Ok(invite.clone())
    }) {
        Ok(invite) => json_response(&invite),
        Err(err) => json_error(&err, StatusCode(404)),
    }
}

fn handle_pricing_experiment(
    state: &AppState,
    session: &crate::studio::auth::Session,
) -> Response<std::io::Cursor<Vec<u8>>> {
    match state.store.read(|data| assign_variant(&data.experiments, &session.user_id)) {
        Ok(variant) => json_response(&variant),
        Err(err) => json_error(&err, StatusCode(500)),
    }
}

fn handle_admin_overview(state: &AppState) -> Response<std::io::Cursor<Vec<u8>>> {
    match state.store.read(|data| build_admin_overview(data)) {
        Ok(overview) => json_response(&overview),
        Err(err) => json_error(&err, StatusCode(500)),
    }
}

fn handle_admin_support(
    state: &AppState,
    workspace_id: &str,
) -> Response<std::io::Cursor<Vec<u8>>> {
    match state.store.read(|data| support_snapshot(data, workspace_id)) {
        Ok(Some(snapshot)) => json_response(&snapshot),
        Ok(None) => json_error("Workspace not found", StatusCode(404)),
        Err(err) => json_error(&err, StatusCode(500)),
    }
}

fn handle_apply_retention(state: &AppState) -> Response<std::io::Cursor<Vec<u8>>> {
    let history_dir = state.options.history_dir.clone();
    match state.store.read(|data| {
        data.workspaces
            .first()
            .map(|workspace| workspace.retention_days)
            .unwrap_or(30)
    }) {
        Ok(days) => match apply_retention(&history_dir, days) {
            Ok(result) => json_response(&result),
            Err(err) => json_error(&err, StatusCode(500)),
        },
        Err(err) => json_error(&err, StatusCode(500)),
    }
}

fn run_remote_qc(
    config_path: &Path,
    media_path: &Path,
    profile_name: &str,
) -> Result<BatchReport, String> {
    let config = load_config(config_path).map_err(|e| e.to_string())?;
    let profile: Profile = config
        .profiles
        .get(profile_name)
        .cloned()
        .ok_or_else(|| format!("Profile not found: {profile_name}"))?;
    Ok(run_batch(
        vec![media_path.to_path_buf()],
        &profile,
        profile_name,
        BatchOptions::default(),
    ))
}

fn base64_decode(input: &str) -> Result<Vec<u8>, String> {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = Vec::new();
    let mut buffer = 0u32;
    let mut bits = 0;

    for ch in input.bytes().filter(|b| !b.is_ascii_whitespace()) {
        if ch == b'=' {
            break;
        }
        let value = TABLE
            .iter()
            .position(|&byte| byte == ch)
            .ok_or_else(|| "Invalid base64".to_string())? as u32;
        buffer = (buffer << 6) | value;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            output.push((buffer >> bits) as u8);
            buffer &= (1 << bits) - 1;
        }
    }

    Ok(output)
}

fn json_response<T: serde::Serialize>(value: &T) -> Response<std::io::Cursor<Vec<u8>>> {
    match serde_json::to_string_pretty(value) {
        Ok(body) => Response::from_string(body).with_header(json_header()),
        Err(err) => json_error(&err.to_string(), StatusCode(500)),
    }
}

fn json_error(message: &str, status: StatusCode) -> Response<std::io::Cursor<Vec<u8>>> {
    Response::from_string(message).with_status_code(status)
}

fn json_header() -> Header {
    Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap()
}
