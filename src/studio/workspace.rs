use serde::{Deserialize, Serialize};

use super::plans::PlanId;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workspace {
    pub id: String,
    pub name: String,
    pub plan: PlanId,
    pub retention_days: u32,
}

impl Workspace {
    pub fn new(id: &str, name: &str, plan: PlanId) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            plan,
            retention_days: 30,
        }
    }
}
