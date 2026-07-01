use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

#[derive(Clone, Deserialize)]
pub struct AppConfig {
    pub profiles: HashMap<String, Profile>,
}

#[derive(Clone, Deserialize)]
pub struct Profile {
    pub extension: String,
    pub width: u32,
    pub height: u32,
    pub require_audio: bool,
    pub min_duration_secs: f64,
}

pub fn load_config(path: &Path) -> Result<AppConfig, Box<dyn std::error::Error>> {
    let contents = std::fs::read_to_string(path)?;
    Ok(toml::from_str(&contents)?)
}
