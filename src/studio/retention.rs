use chrono::{DateTime, Utc};
use std::fs;
use std::path::Path;

pub fn apply_retention(history_dir: &Path, retention_days: u32) -> Result<RetentionResult, String> {
    if !history_dir.exists() {
        return Ok(RetentionResult::default());
    }

    let cutoff = Utc::now() - chrono::Duration::days(retention_days as i64);
    let mut deleted = 0usize;
    let mut kept = 0usize;

    for entry in fs::read_dir(history_dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }

        let modified: DateTime<Utc> = entry
            .metadata()
            .map_err(|e| e.to_string())?
            .modified()
            .map_err(|e| e.to_string())?
            .into();

        if modified < cutoff {
            fs::remove_file(&path).map_err(|e| e.to_string())?;
            deleted += 1;
        } else {
            kept += 1;
        }
    }

    Ok(RetentionResult { deleted, kept })
}

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct RetentionResult {
    pub deleted: usize,
    pub kept: usize,
}
