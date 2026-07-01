use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UsageLedger {
    pub runs_this_month: u32,
    pub storage_bytes: u64,
    pub events: Vec<UsageEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageEvent {
    pub at: DateTime<Utc>,
    pub workspace_id: String,
    pub kind: UsageKind,
    pub amount: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UsageKind {
    QcRun,
    StorageBytes,
    RemoteJob,
}

impl UsageLedger {
    pub fn record_run(&mut self, workspace_id: &str) {
        self.runs_this_month += 1;
        self.events.push(UsageEvent {
            at: Utc::now(),
            workspace_id: workspace_id.to_string(),
            kind: UsageKind::QcRun,
            amount: 1,
        });
    }

    pub fn record_storage(&mut self, workspace_id: &str, bytes: u64) {
        self.storage_bytes += bytes;
        self.events.push(UsageEvent {
            at: Utc::now(),
            workspace_id: workspace_id.to_string(),
            kind: UsageKind::StorageBytes,
            amount: bytes,
        });
    }
}

pub fn usage_summary(ledger: &UsageLedger) -> UsageSummary {
    UsageSummary {
        runs_this_month: ledger.runs_this_month,
        storage_bytes: ledger.storage_bytes,
        estimated_cost_usd: (ledger.runs_this_month as f64 * 0.05)
            + (ledger.storage_bytes as f64 / 1_073_741_824.0 * 0.10),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageSummary {
    pub runs_this_month: u32,
    pub storage_bytes: u64,
    pub estimated_cost_usd: f64,
}
