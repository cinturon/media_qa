use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteJob {
    pub id: String,
    pub workspace_id: String,
    pub upload_id: String,
    pub profile: String,
    pub status: JobStatus,
    pub progress_pct: u8,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub report_id: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    Queued,
    Running,
    Completed,
    Failed,
}

pub fn queue_job(workspace_id: &str, upload_id: &str, profile: &str) -> RemoteJob {
    let now = Utc::now();
    RemoteJob {
        id: format!("job_{workspace_id}_{}", now.timestamp()),
        workspace_id: workspace_id.to_string(),
        upload_id: upload_id.to_string(),
        profile: profile.to_string(),
        status: JobStatus::Queued,
        progress_pct: 0,
        created_at: now,
        updated_at: now,
        report_id: None,
        error: None,
    }
}

pub fn advance_job(job: &mut RemoteJob) {
    job.updated_at = Utc::now();
    match job.status {
        JobStatus::Queued => {
            job.status = JobStatus::Running;
            job.progress_pct = 25;
        }
        JobStatus::Running if job.progress_pct < 90 => {
            job.progress_pct = (job.progress_pct + 25).min(90);
        }
        JobStatus::Running => {
            job.status = JobStatus::Completed;
            job.progress_pct = 100;
            job.report_id = Some(format!("{}-report", job.id));
        }
        _ => {}
    }
}
