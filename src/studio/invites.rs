use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Invite {
    pub token: String,
    pub workspace_id: String,
    pub email: String,
    pub created_at: DateTime<Utc>,
    pub accepted: bool,
}

pub fn create_invite(workspace_id: &str, email: &str) -> Invite {
    Invite {
        token: format!("invite_{workspace_id}_{}", slug(email)),
        workspace_id: workspace_id.to_string(),
        email: email.to_string(),
        created_at: Utc::now(),
        accepted: false,
    }
}

pub fn accept_invite(invite: &mut Invite) {
    invite.accepted = true;
}

fn slug(input: &str) -> String {
    input
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect()
}
