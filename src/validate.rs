use crate::config::Profile;
use crate::metadata::MediaMetadata;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Warn,
    Fail,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Pass,
    Warn,
    Fail,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Finding {
    pub code: String,
    pub severity: Severity,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationResult {
    pub findings: Vec<Finding>,
    pub status: Status,
}

impl ValidationResult {
    pub fn from_findings(findings: Vec<Finding>) -> Self {
        let status = status_from_findings(&findings);
        Self { findings, status }
    }
}

pub fn status_from_findings(findings: &[Finding]) -> Status {
    if findings.iter().any(|f| f.severity == Severity::Fail) {
        Status::Fail
    } else if findings.iter().any(|f| f.severity == Severity::Warn) {
        Status::Warn
    } else {
        Status::Pass
    }
}

pub fn validate(path: &Path, metadata: &MediaMetadata, profile: &Profile) -> ValidationResult {
    let mut findings = Vec::new();

    let extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("");

    if !extension.eq_ignore_ascii_case(&profile.extension) {
        findings.push(Finding {
            code: "EXTENSION_MISMATCH".into(),
            severity: Severity::Fail,
            message: format!("expected .{}, got .{}", profile.extension, extension),
        });
    }

    if metadata.width != profile.width || metadata.height != profile.height {
        findings.push(Finding {
            code: "RESOLUTION_MISMATCH".into(),
            severity: Severity::Fail,
            message: format!(
                "expected {}x{}, got {}x{}",
                profile.width, profile.height, metadata.width, metadata.height
            ),
        });
    }

    if profile.require_audio && !metadata.has_audio {
        findings.push(Finding {
            code: "AUDIO_REQUIRED".into(),
            severity: Severity::Fail,
            message: "profile requires an audio track".into(),
        });
    }

    if metadata.duration_secs < profile.min_duration_secs {
        findings.push(Finding {
            code: "DURATION_TOO_SHORT".into(),
            severity: Severity::Fail,
            message: format!(
                "duration {:.2}s is below minimum {:.2}s",
                metadata.duration_secs, profile.min_duration_secs
            ),
        });
    }

    ValidationResult::from_findings(findings)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata::MediaMetadata;

    fn sample_profile() -> Profile {
        Profile {
            extension: "mp4".into(),
            width: 1920,
            height: 1080,
            require_audio: true,
            min_duration_secs: 5.0,
            max_silence_secs: 2.0,
            min_mean_volume_db: -50.0,
            require_captions: false,
            require_thumbnail: false,
        }
    }

    fn sample_metadata() -> MediaMetadata {
        MediaMetadata {
            duration_secs: 10.0,
            width: 1920,
            height: 1080,
            has_audio: true,
            format_name: "mp4".into(),
            video_codec: Some("h264".into()),
        }
    }

    #[test]
    fn validate_passes_when_metadata_matches_profile() {
        let path = Path::new("clip.mp4");
        let result = validate(path, &sample_metadata(), &sample_profile());
        assert_eq!(result.status, Status::Pass);
        assert!(result.findings.is_empty());
    }

    #[test]
    fn validate_flags_extension_mismatch() {
        let result = validate(Path::new("clip.mov"), &sample_metadata(), &sample_profile());
        assert_eq!(result.status, Status::Fail);
        assert!(result
            .findings
            .iter()
            .any(|f| f.code == "EXTENSION_MISMATCH"));
    }

    #[test]
    fn validate_flags_resolution_mismatch() {
        let mut metadata = sample_metadata();
        metadata.width = 1280;
        metadata.height = 720;
        let result = validate(Path::new("clip.mp4"), &metadata, &sample_profile());
        assert_eq!(result.status, Status::Fail);
    }

    #[test]
    fn validate_flags_missing_required_audio() {
        let mut metadata = sample_metadata();
        metadata.has_audio = false;
        let result = validate(Path::new("clip.mp4"), &metadata, &sample_profile());
        assert!(result.findings.iter().any(|f| f.code == "AUDIO_REQUIRED"));
    }

    #[test]
    fn validate_flags_short_duration() {
        let mut metadata = sample_metadata();
        metadata.duration_secs = 2.0;
        let result = validate(Path::new("clip.mp4"), &metadata, &sample_profile());
        assert!(result
            .findings
            .iter()
            .any(|f| f.code == "DURATION_TOO_SHORT"));
    }
}
