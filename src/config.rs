use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

#[derive(Clone)]
pub struct AppConfig {
    pub profiles: HashMap<String, Profile>,
}

#[derive(Clone, Debug, Deserialize)]
struct ProfileEntry {
    inherits: Option<String>,
    extension: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    require_audio: Option<bool>,
    min_duration_secs: Option<f64>,
    max_silence_secs: Option<f64>,
    min_mean_volume_db: Option<f64>,
    require_captions: Option<bool>,
    require_thumbnail: Option<bool>,
}

#[derive(Clone, Debug, Deserialize)]
struct RawConfig {
    profiles: HashMap<String, ProfileEntry>,
}

#[derive(Clone, Debug)]
pub struct Profile {
    pub extension: String,
    pub width: u32,
    pub height: u32,
    pub require_audio: bool,
    pub min_duration_secs: f64,
    pub max_silence_secs: f64,
    pub min_mean_volume_db: f64,
    pub require_captions: bool,
    pub require_thumbnail: bool,
}

pub fn load_config(path: &Path) -> Result<AppConfig, Box<dyn std::error::Error>> {
    let contents = std::fs::read_to_string(path)?;
    let raw: RawConfig = toml::from_str(&contents)?;
    let profiles = resolve_profiles(raw.profiles)?;
    Ok(AppConfig { profiles })
}

fn resolve_profiles(
    entries: HashMap<String, ProfileEntry>,
) -> Result<HashMap<String, Profile>, Box<dyn std::error::Error>> {
    let mut resolved: HashMap<String, Profile> = HashMap::new();

    for name in entries.keys() {
        let profile = resolve_one(name, &entries, &resolved)?;
        resolved.insert(name.clone(), profile);
    }

    Ok(resolved)
}

fn resolve_one(
    name: &str,
    entries: &HashMap<String, ProfileEntry>,
    cache: &HashMap<String, Profile>,
) -> Result<Profile, Box<dyn std::error::Error>> {
    if let Some(profile) = cache.get(name) {
        return Ok(profile.clone());
    }

    let entry = entries
        .get(name)
        .ok_or_else(|| format!("profile not found: {name}"))?;

    let mut base = Profile {
        extension: "mp4".into(),
        width: 1920,
        height: 1080,
        require_audio: true,
        min_duration_secs: 1.0,
        max_silence_secs: 2.0,
        min_mean_volume_db: -50.0,
        require_captions: false,
        require_thumbnail: false,
    };

    if let Some(parent_name) = &entry.inherits {
        base = resolve_one(parent_name, entries, cache)?;
    }

    Ok(Profile {
        extension: entry.extension.clone().unwrap_or(base.extension),
        width: entry.width.unwrap_or(base.width),
        height: entry.height.unwrap_or(base.height),
        require_audio: entry.require_audio.unwrap_or(base.require_audio),
        min_duration_secs: entry
            .min_duration_secs
            .unwrap_or(base.min_duration_secs),
        max_silence_secs: entry.max_silence_secs.unwrap_or(base.max_silence_secs),
        min_mean_volume_db: entry
            .min_mean_volume_db
            .unwrap_or(base.min_mean_volume_db),
        require_captions: entry.require_captions.unwrap_or(base.require_captions),
        require_thumbnail: entry
            .require_thumbnail
            .unwrap_or(base.require_thumbnail),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_inheritance_merges_parent_defaults() {
        let mut entries = HashMap::new();
        entries.insert(
            "base".into(),
            ProfileEntry {
                inherits: None,
                extension: Some("mp4".into()),
                width: None,
                height: None,
                require_audio: Some(true),
                min_duration_secs: Some(5.0),
                max_silence_secs: None,
                min_mean_volume_db: None,
                require_captions: None,
                require_thumbnail: None,
            },
        );
        entries.insert(
            "youtube".into(),
            ProfileEntry {
                inherits: Some("base".into()),
                extension: None,
                width: Some(1920),
                height: Some(1080),
                require_audio: None,
                min_duration_secs: None,
                max_silence_secs: None,
                min_mean_volume_db: None,
                require_captions: None,
                require_thumbnail: None,
            },
        );

        let profiles = resolve_profiles(entries).expect("profiles resolve");
        let youtube = profiles.get("youtube").expect("youtube profile");
        assert_eq!(youtube.extension, "mp4");
        assert_eq!(youtube.width, 1920);
        assert_eq!(youtube.min_duration_secs, 5.0);
    }

    #[test]
    fn sample_config_loads_from_disk() {
        let config = load_config(Path::new("src/sample.toml")).expect("config loads");
        assert!(config.profiles.contains_key("youtube"));
        let youtube = config.profiles.get("youtube").expect("youtube");
        assert_eq!(youtube.width, 1920);
    }
}
