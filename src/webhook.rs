use crate::report::BatchReport;
use std::io::Write;
use std::net::TcpStream;

pub fn post_webhook(url: &str, report: &BatchReport) -> Result<(), String> {
    let payload = serde_json::to_string(report).map_err(|e| e.to_string())?;
    let endpoint = url
        .trim_start_matches("http://")
        .trim_start_matches("https://");
    let (host, path) = endpoint
        .split_once('/')
        .map(|(host, path)| (host, format!("/{path}")))
        .unwrap_or((endpoint, "/".into()));

    let mut stream = TcpStream::connect(host).map_err(|e| format!("webhook connect failed: {e}"))?;
    let request = format!(
        "POST {path} HTTP/1.1\r\nHost: {host}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
        payload.len()
    );
    stream
        .write_all(request.as_bytes())
        .map_err(|e| format!("webhook write failed: {e}"))?;
    Ok(())
}
