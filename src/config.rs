use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

pub const DEFAULT_CONFIG_RELATIVE: &str = "src/sample.toml";
pub const USER_PROFILES_RELATIVE: &str = ".mediaqa/profiles.toml";

/// Profile names that exist only in the user config file (not the bundled defaults).
pub const BUILTIN_PROFILE_NAMES: &[&str] = &[
    "base",
    "vertical",
    "shorts",
    "instagram_reels",
    "tiktok",
    "youtube",
    "youtube_4k",
    "vimeo",
    "web_embed",
    "client_review",
    "podcast_audiogram",
    "broadcast_hd",
    "broadcast_uhd",
];

#[derive(Clone)]
pub struct AppConfig {
    pub profiles: HashMap<String, Profile>,
}

#[derive(Clone, Debug, Deserialize, Default, Serialize)]
struct ProfileEntry {
    inherits: Option<String>,
    extension: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    require_audio: Option<bool>,
    min_duration_secs: Option<f64>,
    max_duration_secs: Option<f64>,
    max_silence_secs: Option<f64>,
    min_mean_volume_db: Option<f64>,
    require_captions: Option<bool>,
    require_thumbnail: Option<bool>,
    video_codec: Option<String>,
    frame_rate: Option<f64>,
    frame_rate_tolerance: Option<f64>,
    min_video_bitrate_kbps: Option<u64>,
    max_video_bitrate_kbps: Option<u64>,
    audio_codec: Option<String>,
    audio_sample_rate: Option<u32>,
    audio_channels: Option<u32>,
    max_integrated_lufs: Option<f64>,
    max_true_peak_db: Option<f64>,
    max_file_size_mb: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
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
    pub max_duration_secs: Option<f64>,
    pub max_silence_secs: f64,
    pub min_mean_volume_db: f64,
    pub require_captions: bool,
    pub require_thumbnail: bool,
    pub video_codec: Option<String>,
    pub frame_rate: Option<f64>,
    pub frame_rate_tolerance: f64,
    pub min_video_bitrate_kbps: Option<u64>,
    pub max_video_bitrate_kbps: Option<u64>,
    pub audio_codec: Option<String>,
    pub audio_sample_rate: Option<u32>,
    pub audio_channels: Option<u32>,
    pub max_integrated_lufs: Option<f64>,
    pub max_true_peak_db: Option<f64>,
    pub max_file_size_mb: Option<u64>,
}

impl Default for Profile {
    fn default() -> Self {
        Self {
            extension: "mp4".into(),
            width: 1920,
            height: 1080,
            require_audio: true,
            min_duration_secs: 1.0,
            max_duration_secs: None,
            max_silence_secs: 2.0,
            min_mean_volume_db: -50.0,
            require_captions: false,
            require_thumbnail: false,
            video_codec: None,
            frame_rate: None,
            frame_rate_tolerance: 0.05,
            min_video_bitrate_kbps: None,
            max_video_bitrate_kbps: None,
            audio_codec: None,
            audio_sample_rate: None,
            audio_channels: None,
            max_integrated_lufs: None,
            max_true_peak_db: None,
            max_file_size_mb: None,
        }
    }
}

pub fn resolve_config_path() -> PathBuf {
    let relative = Path::new(DEFAULT_CONFIG_RELATIVE);
    if relative.exists() {
        return relative.to_path_buf();
    }

    if let Ok(mut dir) = std::env::current_dir() {
        for _ in 0..8 {
            let candidate = dir.join(DEFAULT_CONFIG_RELATIVE);
            if candidate.is_file() {
                return candidate;
            }
            if !dir.pop() {
                break;
            }
        }
    }

    relative.to_path_buf()
}

pub fn user_profiles_path() -> PathBuf {
    locate_existing_user_profiles_path().unwrap_or_else(default_user_profiles_path)
}

fn default_user_profiles_path() -> PathBuf {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(|home| PathBuf::from(home).join(".mediaqa").join("profiles.toml"))
        .unwrap_or_else(|| PathBuf::from(USER_PROFILES_RELATIVE))
}

fn locate_existing_user_profiles_path() -> Option<PathBuf> {
    let mut candidates = Vec::new();
    candidates.push(PathBuf::from(USER_PROFILES_RELATIVE));

    if let Ok(mut dir) = std::env::current_dir() {
        for _ in 0..10 {
            candidates.push(dir.join(USER_PROFILES_RELATIVE));
            if !dir.pop() {
                break;
            }
        }
    }

    if let Some(home) = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")) {
        candidates.push(PathBuf::from(home).join(".mediaqa").join("profiles.toml"));
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Some(mut dir) = exe.parent().map(Path::to_path_buf) {
            for _ in 0..8 {
                candidates.push(dir.join(USER_PROFILES_RELATIVE));
                if !dir.pop() {
                    break;
                }
            }
        }
    }

    let mut seen = HashSet::new();
    for candidate in candidates {
        if seen.insert(candidate.clone()) && candidate.is_file() {
            return Some(candidate);
        }
    }

    None
}

pub fn load_default_config() -> Result<AppConfig, Box<dyn std::error::Error>> {
    load_merged_config()
}

fn load_raw_config(path: &Path) -> Result<RawConfig, Box<dyn std::error::Error>> {
    let contents = std::fs::read_to_string(path)?;
    Ok(toml::from_str(&contents)?)
}

pub fn load_merged_config() -> Result<AppConfig, Box<dyn std::error::Error>> {
    let base_path = resolve_config_path();
    let mut entries = load_raw_config(&base_path)?.profiles;
    let user_path = user_profiles_path();
    if user_path.is_file() {
        let user_entries = load_raw_config(&user_path)?.profiles;
        for (name, entry) in user_entries {
            entries.insert(name, entry);
        }
    }
    let profiles = resolve_profiles(entries)?;
    Ok(AppConfig { profiles })
}

pub fn custom_profile_names() -> Result<HashSet<String>, Box<dyn std::error::Error>> {
    let user_path = user_profiles_path();
    if !user_path.is_file() {
        return Ok(HashSet::new());
    }
    let contents = std::fs::read_to_string(&user_path)?;
    let raw: RawConfig = toml::from_str(&contents)?;
    Ok(raw.profiles.keys().cloned().collect())
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileRules {
    pub require_audio: bool,
    pub require_captions: bool,
    pub require_thumbnail: bool,
    pub min_duration_secs: f64,
    pub max_duration_secs: Option<f64>,
    pub max_silence_secs: f64,
    pub min_mean_volume_db: f64,
    pub video_codec: Option<String>,
    pub frame_rate: Option<f64>,
    pub frame_rate_tolerance: f64,
    pub min_video_bitrate_kbps: Option<u64>,
    pub max_video_bitrate_kbps: Option<u64>,
    pub audio_codec: Option<String>,
    pub audio_sample_rate: Option<u32>,
    pub audio_channels: Option<u32>,
    pub max_integrated_lufs: Option<f64>,
    pub max_true_peak_db: Option<f64>,
    pub max_file_size_mb: Option<u64>,
}

impl From<&Profile> for ProfileRules {
    fn from(profile: &Profile) -> Self {
        Self {
            require_audio: profile.require_audio,
            require_captions: profile.require_captions,
            require_thumbnail: profile.require_thumbnail,
            min_duration_secs: profile.min_duration_secs,
            max_duration_secs: profile.max_duration_secs,
            max_silence_secs: profile.max_silence_secs,
            min_mean_volume_db: profile.min_mean_volume_db,
            video_codec: profile.video_codec.clone(),
            frame_rate: profile.frame_rate,
            frame_rate_tolerance: profile.frame_rate_tolerance,
            min_video_bitrate_kbps: profile.min_video_bitrate_kbps,
            max_video_bitrate_kbps: profile.max_video_bitrate_kbps,
            audio_codec: profile.audio_codec.clone(),
            audio_sample_rate: profile.audio_sample_rate,
            audio_channels: profile.audio_channels,
            max_integrated_lufs: profile.max_integrated_lufs,
            max_true_peak_db: profile.max_true_peak_db,
            max_file_size_mb: profile.max_file_size_mb,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileSummary {
    pub name: String,
    pub label: String,
    pub width: u32,
    pub height: u32,
    pub extension: String,
    pub custom: bool,
    pub inherits: Option<String>,
    pub group: String,
    pub rules: ProfileRules,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateProfileInput {
    pub name: String,
    pub inherits: Option<String>,
    pub extension: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub require_audio: Option<bool>,
    pub require_captions: Option<bool>,
    pub require_thumbnail: Option<bool>,
    pub min_duration_secs: Option<f64>,
    pub max_duration_secs: Option<f64>,
    pub max_silence_secs: Option<f64>,
    pub min_mean_volume_db: Option<f64>,
    pub video_codec: Option<String>,
    pub frame_rate: Option<f64>,
    pub frame_rate_tolerance: Option<f64>,
    pub min_video_bitrate_kbps: Option<u64>,
    pub max_video_bitrate_kbps: Option<u64>,
    pub audio_codec: Option<String>,
    pub audio_sample_rate: Option<u32>,
    pub audio_channels: Option<u32>,
    pub max_integrated_lufs: Option<f64>,
    pub max_true_peak_db: Option<f64>,
    pub max_file_size_mb: Option<u64>,
}

pub fn list_profile_summaries() -> Result<Vec<ProfileSummary>, Box<dyn std::error::Error>> {
    let config = load_merged_config()?;
    let custom = custom_profile_names()?;
    let mut summaries: Vec<ProfileSummary> = config
        .profiles
        .keys()
        .filter(|name| !matches!(name.as_str(), "base" | "vertical"))
        .map(|name| profile_summary(name, &config.profiles[name], custom.contains(name)))
        .collect();
    summaries.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(summaries)
}

pub fn create_user_profile(input: CreateProfileInput) -> Result<ProfileSummary, Box<dyn std::error::Error>> {
    let name = normalize_profile_name(&input.name)?;
    let merged = load_merged_config()?;
    if merged.profiles.contains_key(&name) {
        return Err(format!("Profile already exists: {name}").into());
    }

    let inherits = input
        .inherits
        .as_ref()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(str::to_string);

    if let Some(parent) = &inherits {
        if !merged.profiles.contains_key(parent) {
            return Err(format!("Base profile not found: {parent}").into());
        }
    }

    let entry = profile_entry_from_input(&input, inherits);

    let mut raw = load_user_raw_config()?;
    raw.profiles.insert(name.clone(), entry);
    save_user_raw_config(&raw)?;

    let config = load_merged_config()?;
    let profile = config
        .profiles
        .get(&name)
        .ok_or_else(|| format!("Profile not found after save: {name}"))?;
    Ok(profile_summary(&name, profile, true))
}

pub fn delete_user_profile(name: &str) -> Result<String, Box<dyn std::error::Error>> {
    let name = normalize_profile_name(name)?;
    let custom = custom_profile_names()?;
    if !custom.contains(&name) {
        return Err(format!("Only custom profiles can be deleted: {name}").into());
    }

    let mut raw = load_user_raw_config()?;
    if raw.profiles.remove(&name).is_none() {
        return Err(format!("Custom profile not found: {name}").into());
    }

    if raw.profiles.is_empty() {
        let path = user_profiles_path();
        if path.is_file() {
            std::fs::remove_file(path)?;
        }
    } else {
        save_user_raw_config(&raw)?;
    }

    Ok(name)
}

fn profile_summary(name: &str, profile: &Profile, custom: bool) -> ProfileSummary {
    ProfileSummary {
        name: name.to_string(),
        label: format!("{} — {}×{}", humanize_profile_name(name), profile.width, profile.height),
        width: profile.width,
        height: profile.height,
        extension: profile.extension.clone(),
        custom,
        inherits: None,
        group: profile_group(name, custom).to_string(),
        rules: ProfileRules::from(profile),
    }
}

fn profile_entry_from_input(input: &CreateProfileInput, inherits: Option<String>) -> ProfileEntry {
    ProfileEntry {
        inherits,
        extension: normalize_optional_string(input.extension.clone()),
        width: input.width,
        height: input.height,
        require_audio: input.require_audio,
        require_captions: input.require_captions,
        require_thumbnail: input.require_thumbnail,
        min_duration_secs: input.min_duration_secs,
        max_duration_secs: input.max_duration_secs,
        max_silence_secs: input.max_silence_secs,
        min_mean_volume_db: input.min_mean_volume_db,
        video_codec: normalize_optional_string(input.video_codec.clone()),
        frame_rate: input.frame_rate,
        frame_rate_tolerance: input.frame_rate_tolerance,
        min_video_bitrate_kbps: input.min_video_bitrate_kbps,
        max_video_bitrate_kbps: input.max_video_bitrate_kbps,
        audio_codec: normalize_optional_string(input.audio_codec.clone()),
        audio_sample_rate: input.audio_sample_rate,
        audio_channels: input.audio_channels,
        max_integrated_lufs: input.max_integrated_lufs,
        max_true_peak_db: input.max_true_peak_db,
        max_file_size_mb: input.max_file_size_mb,
    }
}

fn normalize_optional_string(value: Option<String>) -> Option<String> {
    value
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
}

fn profile_group(name: &str, custom: bool) -> &'static str {
    if custom {
        return "Custom";
    }
    match name {
        "shorts" | "instagram_reels" | "tiktok" => "Vertical social",
        "youtube" | "youtube_4k" | "vimeo" => "Long-form",
        "web_embed" | "client_review" => "Web & review",
        "podcast_audiogram" | "broadcast_hd" | "broadcast_uhd" => "Other",
        _ => "Custom",
    }
}

fn humanize_profile_name(name: &str) -> String {
    name.split('_')
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn normalize_profile_name(raw: &str) -> Result<String, Box<dyn std::error::Error>> {
    let name = raw
        .trim()
        .to_ascii_lowercase()
        .replace([' ', '-'], "_");
    if name.is_empty() {
        return Err("Profile name is required".into());
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
    {
        return Err(
            "Profile name may only use lowercase letters, numbers, and underscores".into(),
        );
    }
    if name.starts_with(|c: char| c.is_ascii_digit()) {
        return Err("Profile name must start with a letter".into());
    }
    if BUILTIN_PROFILE_NAMES.contains(&name.as_str()) {
        return Err(format!("Profile name is reserved: {name}").into());
    }
    Ok(name)
}

fn load_user_raw_config() -> Result<RawConfig, Box<dyn std::error::Error>> {
    let path = user_profiles_path();
    if !path.is_file() {
        return Ok(RawConfig {
            profiles: HashMap::new(),
        });
    }
    let contents = std::fs::read_to_string(&path)?;
    Ok(toml::from_str(&contents)?)
}

fn save_user_raw_config(raw: &RawConfig) -> Result<(), Box<dyn std::error::Error>> {
    let path = user_profiles_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let contents = toml::to_string_pretty(raw)?;
    std::fs::write(path, contents)?;
    Ok(())
}

pub fn load_config(path: &Path) -> Result<AppConfig, Box<dyn std::error::Error>> {
    let profiles = resolve_profiles(load_raw_config(path)?.profiles)?;
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
        max_duration_secs: None,
        max_silence_secs: 2.0,
        min_mean_volume_db: -50.0,
        require_captions: false,
        require_thumbnail: false,
        video_codec: None,
        frame_rate: None,
        frame_rate_tolerance: 0.05,
        min_video_bitrate_kbps: None,
        max_video_bitrate_kbps: None,
        audio_codec: None,
        audio_sample_rate: None,
        audio_channels: None,
        max_integrated_lufs: None,
        max_true_peak_db: None,
        max_file_size_mb: None,
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
        max_duration_secs: entry.max_duration_secs.or(base.max_duration_secs),
        max_silence_secs: entry.max_silence_secs.unwrap_or(base.max_silence_secs),
        min_mean_volume_db: entry
            .min_mean_volume_db
            .unwrap_or(base.min_mean_volume_db),
        require_captions: entry.require_captions.unwrap_or(base.require_captions),
        require_thumbnail: entry
            .require_thumbnail
            .unwrap_or(base.require_thumbnail),
        video_codec: entry.video_codec.clone().or(base.video_codec),
        frame_rate: entry.frame_rate.or(base.frame_rate),
        frame_rate_tolerance: entry
            .frame_rate_tolerance
            .unwrap_or(base.frame_rate_tolerance),
        min_video_bitrate_kbps: entry
            .min_video_bitrate_kbps
            .or(base.min_video_bitrate_kbps),
        max_video_bitrate_kbps: entry
            .max_video_bitrate_kbps
            .or(base.max_video_bitrate_kbps),
        audio_codec: entry.audio_codec.clone().or(base.audio_codec),
        audio_sample_rate: entry.audio_sample_rate.or(base.audio_sample_rate),
        audio_channels: entry.audio_channels.or(base.audio_channels),
        max_integrated_lufs: entry.max_integrated_lufs.or(base.max_integrated_lufs),
        max_true_peak_db: entry.max_true_peak_db.or(base.max_true_peak_db),
        max_file_size_mb: entry.max_file_size_mb.or(base.max_file_size_mb),
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
                min_duration_secs: Some(5.0),
                require_audio: Some(true),
                ..ProfileEntry::default()
            },
        );
        entries.insert(
            "youtube".into(),
            ProfileEntry {
                inherits: Some("base".into()),
                width: Some(1920),
                height: Some(1080),
                ..ProfileEntry::default()
            },
        );

        let profiles = resolve_profiles(entries).expect("profiles resolve");
        let youtube = profiles.get("youtube").expect("youtube profile");
        assert_eq!(youtube.extension, "mp4");
        assert_eq!(youtube.width, 1920);
        assert_eq!(youtube.min_duration_secs, 5.0);
    }

    #[test]
    fn resolve_config_path_finds_sample_toml_from_repo_root() {
        let path = resolve_config_path();
        assert!(
            path.is_file(),
            "expected config at {}, cwd={:?}",
            path.display(),
            std::env::current_dir().ok()
        );
    }

    #[test]
    fn sample_config_loads_from_disk() {
        let config = load_config(Path::new("src/sample.toml")).expect("config loads");
        assert!(config.profiles.contains_key("youtube"));
        assert!(config.profiles.contains_key("instagram_reels"));
        assert!(config.profiles.contains_key("client_review"));
        let youtube = config.profiles.get("youtube").expect("youtube");
        assert_eq!(youtube.width, 1920);
        let reels = config
            .profiles
            .get("instagram_reels")
            .expect("instagram_reels");
        assert!(reels.require_captions);
        assert_eq!(reels.width, 1080);
        assert_eq!(reels.height, 1920);
    }

    #[test]
    fn merged_config_includes_sample_profiles() {
        let config = load_merged_config().expect("merged config");
        assert!(config.profiles.contains_key("youtube"));
    }

    #[test]
    fn resolve_profiles_supports_custom_profile_without_parent() {
        let mut entries = HashMap::new();
        entries.insert(
            "client_scratch".into(),
            ProfileEntry {
                inherits: None,
                extension: Some("mov".into()),
                width: Some(1280),
                height: Some(720),
                require_audio: Some(false),
                require_captions: Some(true),
                ..ProfileEntry::default()
            },
        );

        let profiles = resolve_profiles(entries).expect("scratch profile resolves");
        let custom = profiles.get("client_scratch").expect("client_scratch");
        assert_eq!(custom.extension, "mov");
        assert_eq!(custom.width, 1280);
        assert_eq!(custom.height, 720);
        assert!(!custom.require_audio);
        assert!(custom.require_captions);
    }

    #[test]
    fn merged_config_resolves_custom_profile_inheriting_builtin() {
        let mut entries = HashMap::new();
        entries.insert(
            "base".into(),
            ProfileEntry {
                extension: Some("mp4".into()),
                width: Some(1920),
                height: Some(1080),
                ..ProfileEntry::default()
            },
        );
        entries.insert(
            "vimeo".into(),
            ProfileEntry {
                inherits: Some("base".into()),
                min_mean_volume_db: Some(-45.0),
                ..ProfileEntry::default()
            },
        );
        entries.insert(
            "client_vimeo".into(),
            ProfileEntry {
                inherits: Some("vimeo".into()),
                width: Some(1280),
                height: Some(720),
                ..ProfileEntry::default()
            },
        );

        let profiles = resolve_profiles(entries).expect("custom profile resolves against builtin parent");
        let custom = profiles.get("client_vimeo").expect("custom profile");
        assert_eq!(custom.width, 1280);
        assert_eq!(custom.height, 720);
        assert_eq!(custom.min_mean_volume_db, -45.0);
    }

    #[test]
    fn list_profile_summaries_hides_internal_parents() {
        let summaries = list_profile_summaries().expect("summaries");
        assert!(!summaries.iter().any(|s| s.name == "base"));
        assert!(!summaries.iter().any(|s| s.name == "vertical"));
        assert!(summaries.iter().any(|s| s.name == "youtube"));
    }

    #[test]
    fn delete_user_profile_rejects_builtin_profiles() {
        assert!(delete_user_profile("youtube").is_err());
    }
}
