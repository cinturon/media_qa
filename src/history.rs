use crate::report::BatchReport;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredReport {
    pub id: String,
    pub created_at: DateTime<Utc>,
    pub report: BatchReport,
}

pub fn save_report(report: &BatchReport, dir: &Path) -> Result<StoredReport, String> {
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let id = Utc::now().format("%Y%m%d-%H%M%S").to_string();
    let stored = StoredReport {
        id: id.clone(),
        created_at: Utc::now(),
        report: report.clone(),
    };
    let path = dir.join(format!("{id}.json"));
    let json = serde_json::to_string_pretty(&stored).map_err(|e| e.to_string())?;
    fs::write(path, json).map_err(|e| e.to_string())?;
    Ok(stored)
}

pub fn load_report(path: &Path) -> Result<StoredReport, String> {
    let contents = fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str(&contents).map_err(|e| e.to_string())
}

pub fn list_reports(dir: &Path) -> Result<Vec<PathBuf>, String> {
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut files: Vec<PathBuf> = fs::read_dir(dir)
        .map_err(|e| e.to_string())?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("json"))
        .collect();

    files.sort();
    Ok(files)
}
