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

    if let Some(max_duration) = profile.max_duration_secs {
        if metadata.duration_secs > max_duration {
            findings.push(Finding {
                code: "DURATION_TOO_LONG".into(),
                severity: Severity::Fail,
                message: format!(
                    "duration {:.2}s exceeds maximum {:.2}s",
                    metadata.duration_secs, max_duration
                ),
            });
        }
    }

    if let Some(expected_codec) = &profile.video_codec {
        if let Some(actual_codec) = &metadata.video_codec {
            if !codec_matches(actual_codec, expected_codec) {
                findings.push(Finding {
                    code: "VIDEO_CODEC_MISMATCH".into(),
                    severity: Severity::Fail,
                    message: format!(
                        "expected video codec {expected_codec}, got {actual_codec}"
                    ),
                });
            }
        }
    }

    if let Some(expected_fps) = profile.frame_rate {
        if let Some(actual_fps) = metadata.frame_rate {
            if !frame_rate_matches(actual_fps, expected_fps, profile.frame_rate_tolerance) {
                findings.push(Finding {
                    code: "FRAME_RATE_MISMATCH".into(),
                    severity: Severity::Fail,
                    message: format!(
                        "expected frame rate {:.3} fps, got {:.3} fps",
                        expected_fps, actual_fps
                    ),
                });
            }
        }
    }

    if let Some(min_kbps) = profile.min_video_bitrate_kbps {
        if let Some(actual_kbps) = metadata.video_bitrate_kbps {
            if actual_kbps < min_kbps {
                findings.push(Finding {
                    code: "VIDEO_BITRATE_TOO_LOW".into(),
                    severity: Severity::Warn,
                    message: format!(
                        "video bitrate {actual_kbps} kbps is below minimum {min_kbps} kbps"
                    ),
                });
            }
        }
    }

    if let Some(max_kbps) = profile.max_video_bitrate_kbps {
        if let Some(actual_kbps) = metadata.video_bitrate_kbps {
            if actual_kbps > max_kbps {
                findings.push(Finding {
                    code: "VIDEO_BITRATE_TOO_HIGH".into(),
                    severity: Severity::Warn,
                    message: format!(
                        "video bitrate {actual_kbps} kbps exceeds maximum {max_kbps} kbps"
                    ),
                });
            }
        }
    }

    if let Some(expected_codec) = &profile.audio_codec {
        if metadata.has_audio {
            if let Some(actual_codec) = &metadata.audio_codec {
                if !codec_matches(actual_codec, expected_codec) {
                    findings.push(Finding {
                        code: "AUDIO_CODEC_MISMATCH".into(),
                        severity: Severity::Fail,
                        message: format!(
                            "expected audio codec {expected_codec}, got {actual_codec}"
                        ),
                    });
                }
            }
        }
    }

    if let Some(expected_rate) = profile.audio_sample_rate {
        if let Some(actual_rate) = metadata.audio_sample_rate {
            if actual_rate != expected_rate {
                findings.push(Finding {
                    code: "AUDIO_SAMPLE_RATE_MISMATCH".into(),
                    severity: Severity::Fail,
                    message: format!(
                        "expected audio sample rate {expected_rate} Hz, got {actual_rate} Hz"
                    ),
                });
            }
        }
    }

    if let Some(expected_channels) = profile.audio_channels {
        if let Some(actual_channels) = metadata.audio_channels {
            if actual_channels != expected_channels {
                findings.push(Finding {
                    code: "AUDIO_CHANNELS_MISMATCH".into(),
                    severity: Severity::Warn,
                    message: format!(
                        "expected {expected_channels} audio channels, got {actual_channels}"
                    ),
                });
            }
        }
    }

    ValidationResult::from_findings(findings)
}

fn codec_matches(actual: &str, expected: &str) -> bool {
    actual.eq_ignore_ascii_case(expected)
        || actual.to_ascii_lowercase().contains(&expected.to_ascii_lowercase())
}

fn frame_rate_matches(actual: f64, expected: f64, tolerance: f64) -> bool {
    (actual - expected).abs() <= tolerance
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata::MediaMetadata;

    use crate::config::Profile;

    fn sample_profile() -> Profile {
        Profile {
            min_duration_secs: 5.0,
            ..Profile::default()
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
            frame_rate: Some(30.0),
            video_bitrate_kbps: Some(5000),
            audio_codec: Some("aac".into()),
            audio_sample_rate: Some(48_000),
            audio_channels: Some(2),
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
