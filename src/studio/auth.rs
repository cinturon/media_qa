use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub email: String,
    pub api_token: String,
}

impl User {
    pub fn new(email: &str, api_token: &str) -> Self {
        Self {
            id: slug(email),
            email: email.to_string(),
            api_token: api_token.to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub token: String,
    pub user_id: String,
    pub workspace_id: String,
}

impl Session {
    pub fn new(user_id: &str, workspace_id: &str) -> Self {
        Self {
            token: format!("sess_{user_id}_{workspace_id}"),
            user_id: user_id.to_string(),
            workspace_id: workspace_id.to_string(),
        }
    }
}

pub fn login_with_token(
    users: &[User],
    sessions: &[Session],
    api_token: &str,
) -> Option<Session> {
    let user = users.iter().find(|u| u.api_token == api_token)?;
    sessions
        .iter()
        .find(|s| s.user_id == user.id)
        .cloned()
        .or_else(|| Some(Session::new(&user.id, "demo-studio")))
}

pub fn switch_workspace(session: &mut Session, workspace_id: &str) {
    session.workspace_id = workspace_id.to_string();
    session.token = format!("sess_{}_{workspace_id}", session.user_id);
}

fn slug(input: &str) -> String {
    input
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect()
}
