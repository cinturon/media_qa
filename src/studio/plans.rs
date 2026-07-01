use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanId {
    Free,
    Pro,
    Studio,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanFeatures {
    pub max_runs_per_month: u32,
    pub retention_days: u32,
    pub parallel_batch: bool,
    pub webhooks: bool,
    pub remote_jobs: bool,
    pub team_seats: u32,
    pub watch_mode: bool,
    pub html_reports: bool,
}

impl PlanId {
    pub fn features(self) -> PlanFeatures {
        match self {
            PlanId::Free => PlanFeatures {
                max_runs_per_month: 25,
                retention_days: 7,
                parallel_batch: false,
                webhooks: false,
                remote_jobs: false,
                team_seats: 1,
                watch_mode: false,
                html_reports: false,
            },
            PlanId::Pro => PlanFeatures {
                max_runs_per_month: 500,
                retention_days: 30,
                parallel_batch: true,
                webhooks: true,
                remote_jobs: true,
                team_seats: 5,
                watch_mode: true,
                html_reports: true,
            },
            PlanId::Studio => PlanFeatures {
                max_runs_per_month: 10_000,
                retention_days: 90,
                parallel_batch: true,
                webhooks: true,
                remote_jobs: true,
                team_seats: 25,
                watch_mode: true,
                html_reports: true,
            },
        }
    }
}

pub fn gate_feature(plan: PlanId, feature: &str) -> bool {
    let features = plan.features();
    match feature {
        "parallel_batch" => features.parallel_batch,
        "webhooks" => features.webhooks,
        "remote_jobs" => features.remote_jobs,
        "watch_mode" => features.watch_mode,
        "html_reports" => features.html_reports,
        _ => false,
    }
}

pub fn within_run_limit(plan: PlanId, runs_this_month: u32) -> bool {
    runs_this_month < plan.features().max_runs_per_month
}

#[cfg(test)]
mod tests {
    use super::{gate_feature, within_run_limit, PlanId};

    #[test]
    fn pro_plan_allows_parallel_batch() {
        assert!(gate_feature(PlanId::Pro, "parallel_batch"));
        assert!(!gate_feature(PlanId::Free, "parallel_batch"));
    }

    #[test]
    fn run_limit_blocks_free_tier_overflow() {
        assert!(within_run_limit(PlanId::Free, 24));
        assert!(!within_run_limit(PlanId::Free, 25));
    }
}
