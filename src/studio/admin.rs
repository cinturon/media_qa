use serde::Serialize;

use super::billing::usage_summary;
use super::plans::PlanId;
use super::store::StudioData;

#[derive(Debug, Serialize)]
pub struct AdminOverview {
    pub workspaces: Vec<WorkspaceSummary>,
    pub total_runs: u32,
    pub total_storage_bytes: u64,
}

#[derive(Debug, Serialize)]
pub struct WorkspaceSummary {
    pub id: String,
    pub name: String,
    pub plan: PlanId,
    pub runs_this_month: u32,
    pub storage_bytes: u64,
    pub open_jobs: usize,
    pub pending_invites: usize,
}

pub fn build_admin_overview(data: &StudioData) -> AdminOverview {
    let usage = usage_summary(&data.usage);
    let workspaces = data
        .workspaces
        .iter()
        .map(|workspace| WorkspaceSummary {
            id: workspace.id.clone(),
            name: workspace.name.clone(),
            plan: workspace.plan,
            runs_this_month: usage.runs_this_month,
            storage_bytes: usage.storage_bytes,
            open_jobs: data
                .jobs
                .iter()
                .filter(|job| job.workspace_id == workspace.id)
                .filter(|job| {
                    matches!(
                        job.status,
                        super::jobs::JobStatus::Queued | super::jobs::JobStatus::Running
                    )
                })
                .count(),
            pending_invites: data
                .invites
                .iter()
                .filter(|invite| invite.workspace_id == workspace.id && !invite.accepted)
                .count(),
        })
        .collect();

    AdminOverview {
        workspaces,
        total_runs: usage.runs_this_month,
        total_storage_bytes: usage.storage_bytes,
    }
}

pub fn support_snapshot(data: &StudioData, workspace_id: &str) -> Option<SupportSnapshot> {
    let workspace = data.workspace(workspace_id)?;
    Some(SupportSnapshot {
        workspace_id: workspace.id.clone(),
        plan: workspace.plan,
        usage: usage_summary(&data.usage),
        uploads: data
            .uploads
            .iter()
            .filter(|upload| upload.workspace_id == workspace_id)
            .count(),
        jobs: data
            .jobs
            .iter()
            .filter(|job| job.workspace_id == workspace_id)
            .count(),
    })
}

#[derive(Debug, Serialize)]
pub struct SupportSnapshot {
    pub workspace_id: String,
    pub plan: PlanId,
    pub usage: super::billing::UsageSummary,
    pub uploads: usize,
    pub jobs: usize,
}
