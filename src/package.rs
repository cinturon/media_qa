use crate::config::Profile;
use crate::validate::{Finding, Severity};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeliveryPackage {
    pub video: PathBuf,
    pub captions: Option<PathBuf>,
    pub thumbnail: Option<PathBuf>,
}

pub fn discover_packages(media_files: &[PathBuf]) -> Vec<DeliveryPackage> {
    media_files
        .iter()
        .map(|video| {
            let stem = video.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            let parent = video.parent().unwrap_or_else(|| Path::new("."));

            let captions = [".srt", ".vtt"]
                .iter()
                .map(|ext| parent.join(format!("{stem}{ext}")))
                .find(|path| path.is_file());

            let thumbnail = [".jpg", ".png"]
                .iter()
                .map(|ext| parent.join(format!("{stem}{ext}")))
                .find(|path| path.is_file());

            DeliveryPackage {
                video: video.clone(),
                captions,
                thumbnail,
            }
        })
        .collect()
}

pub fn validate_package(package: &DeliveryPackage, profile: &Profile) -> Vec<Finding> {
    let mut findings = Vec::new();
    let basename = package
        .video
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("unknown");

    if profile.require_captions && package.captions.is_none() {
        findings.push(Finding {
            code: "CAPTIONS_MISSING".into(),
            severity: Severity::Fail,
            message: format!("expected captions sidecar for {basename}"),
        });
    }

    if profile.require_thumbnail && package.thumbnail.is_none() {
        findings.push(Finding {
            code: "THUMBNAIL_MISSING".into(),
            severity: Severity::Fail,
            message: format!("expected thumbnail sidecar for {basename}"),
        });
    }

    if let (Some(captions), Some(video_stem)) = (
        package.captions.as_ref(),
        package.video.file_stem().and_then(|s| s.to_str()),
    ) {
        let caption_stem = captions.file_stem().and_then(|s| s.to_str());
        if caption_stem != Some(video_stem) {
            findings.push(Finding {
                code: "CAPTIONS_NAME_MISMATCH".into(),
                severity: Severity::Fail,
                message: format!(
                    "captions basename {:?} does not match video {:?}",
                    caption_stem, video_stem
                ),
            });
        }
    }

    findings
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_package_flags_missing_captions() {
        let package = DeliveryPackage {
            video: PathBuf::from("clip.mp4"),
            captions: None,
            thumbnail: Some(PathBuf::from("clip.jpg")),
        };
        let profile = Profile {
            extension: "mp4".into(),
            width: 1920,
            height: 1080,
            require_audio: true,
            min_duration_secs: 1.0,
            max_silence_secs: 2.0,
            min_mean_volume_db: -50.0,
            require_captions: true,
            require_thumbnail: false,
        };

        let findings = validate_package(&package, &profile);
        assert!(findings.iter().any(|f| f.code == "CAPTIONS_MISSING"));
    }
}
