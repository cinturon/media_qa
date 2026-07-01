use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use super::auth::{Session, User};
use super::billing::UsageLedger;
use super::experiments::ExperimentState;
use super::invites::Invite;
use super::jobs::RemoteJob;
use super::plans::PlanId;
use super::uploads::UploadRecord;
use super::workspace::Workspace;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StudioData {
    pub users: Vec<User>,
    pub workspaces: Vec<Workspace>,
    pub sessions: Vec<Session>,
    pub invites: Vec<Invite>,
    pub uploads: Vec<UploadRecord>,
    pub jobs: Vec<RemoteJob>,
    pub usage: UsageLedger,
    pub experiments: ExperimentState,
}

pub struct StudioStore {
    path: PathBuf,
    data: Mutex<StudioData>,
}

impl StudioStore {
    pub fn open(dir: &Path) -> Result<Self, String> {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        let path = dir.join("studio.json");
        let data = if path.exists() {
            let raw = fs::read_to_string(&path).map_err(|e| e.to_string())?;
            serde_json::from_str(&raw).unwrap_or_default()
        } else {
            let data = StudioData::seed();
            let json = serde_json::to_string_pretty(&data).map_err(|e| e.to_string())?;
            fs::write(&path, json).map_err(|e| e.to_string())?;
            data
        };

        Ok(Self {
            path,
            data: Mutex::new(data),
        })
    }

    pub fn with_data<T>(&self, f: impl FnOnce(&mut StudioData) -> Result<T, String>) -> Result<T, String> {
        let mut guard = self.lock()?;
        let result = f(&mut guard)?;
        self.flush(&guard)?;
        Ok(result)
    }

    pub fn read<T>(&self, f: impl FnOnce(&StudioData) -> T) -> Result<T, String> {
        let guard = self.lock()?;
        Ok(f(&guard))
    }

    fn lock(&self) -> Result<MutexGuard<'_, StudioData>, String> {
        self.data.lock().map_err(|_| "studio store lock poisoned".into())
    }

    fn flush(&self, data: &StudioData) -> Result<(), String> {
        let json = serde_json::to_string_pretty(data).map_err(|e| e.to_string())?;
        fs::write(&self.path, json).map_err(|e| e.to_string())
    }
}

impl StudioData {
    fn seed() -> Self {
        let workspace = Workspace::new("demo-studio", "Demo Studio", PlanId::Pro);
        let user = User::new("operator@mediaqa.local", "demo-token-change-me");
        let session = Session::new(&user.id, &workspace.id);
        Self {
            users: vec![user],
            workspaces: vec![workspace],
            sessions: vec![session],
            invites: Vec::new(),
            uploads: Vec::new(),
            jobs: Vec::new(),
            usage: UsageLedger::default(),
            experiments: ExperimentState::default(),
        }
    }

    pub fn workspace(&self, id: &str) -> Option<&Workspace> {
        self.workspaces.iter().find(|w| w.id == id)
    }

    pub fn user_by_token(&self, token: &str) -> Option<&User> {
        self.users.iter().find(|u| u.api_token == token)
    }

    pub fn session_by_token(&self, token: &str) -> Option<&Session> {
        self.sessions.iter().find(|s| s.token == token)
    }
}
