use crate::history::{list_reports, load_report};
use std::path::Path;
use std::sync::Arc;
use std::thread;
use tiny_http::{Header, Response, Server, StatusCode};

pub fn serve(history_dir: &Path, port: u16) -> Result<(), String> {
    let server = Server::http(format!("127.0.0.1:{port}")).map_err(|e| e.to_string())?;
    let dir = Arc::new(history_dir.to_path_buf());

    println!("Dashboard API listening on http://127.0.0.1:{port}");
    println!("  GET /reports");
    println!("  GET /reports/{{id}}");

    for request in server.incoming_requests() {
        let dir = Arc::clone(&dir);
        thread::spawn(move || handle_request(request, &dir));
    }

    Ok(())
}

fn handle_request(
    request: tiny_http::Request,
    history_dir: &Path,
) {
    let path = request.url().to_string();
    let response = match path.as_str() {
        "/reports" => list_reports_json(history_dir),
        other if other.starts_with("/reports/") => {
            let id = other.trim_start_matches("/reports/");
            load_report_json(history_dir, id)
        }
        _ => Response::from_string("Not Found").with_status_code(StatusCode(404)),
    };

    let _ = request.respond(response);
}

fn list_reports_json(history_dir: &Path) -> Response<std::io::Cursor<Vec<u8>>> {
    match list_reports(history_dir) {
        Ok(paths) => {
            let ids: Vec<String> = paths
                .iter()
                .filter_map(|path| {
                    path.file_stem()
                        .and_then(|stem| stem.to_str())
                        .map(str::to_string)
                })
                .collect();
            json_response(&ids)
        }
        Err(err) => error_response(&err),
    }
}

fn load_report_json(history_dir: &Path, id: &str) -> Response<std::io::Cursor<Vec<u8>>> {
    let path = history_dir.join(format!("{id}.json"));
    match load_report(&path) {
        Ok(stored) => json_response(&stored),
        Err(err) => error_response(&err),
    }
}

fn json_response<T: serde::Serialize>(value: &T) -> Response<std::io::Cursor<Vec<u8>>> {
    match serde_json::to_string_pretty(value) {
        Ok(body) => Response::from_string(body).with_header(
            Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap(),
        ),
        Err(err) => error_response(&err.to_string()),
    }
}

fn error_response(message: &str) -> Response<std::io::Cursor<Vec<u8>>> {
    Response::from_string(message).with_status_code(StatusCode(500))
}
