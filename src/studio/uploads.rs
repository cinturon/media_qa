use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadRecord {
    pub id: String,
    pub workspace_id: String,
    pub filename: String,
    pub stored_path: PathBuf,
    pub bytes: u64,
    pub created_at: DateTime<Utc>,
}

pub fn create_upload(
    workspace_id: &str,
    filename: &str,
    stored_path: PathBuf,
    bytes: u64,
) -> UploadRecord {
    UploadRecord {
        id: format!("upload_{}_{}", workspace_id, Utc::now().timestamp()),
        workspace_id: workspace_id.to_string(),
        filename: filename.to_string(),
        stored_path,
        bytes,
        created_at: Utc::now(),
    }
}
