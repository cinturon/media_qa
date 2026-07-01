use crate::config::Profile;
use crate::validate::{Finding, Severity};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct BatchProbeSnapshot {
    pub path: PathBuf,
    pub width: u32,
    pub height: u32,
    pub video_codec: String,
    pub file_size_bytes: u64,
}

pub fn file_size_bytes(path: &Path) -> Option<u64> {
    std::fs::metadata(path).ok().map(|meta| meta.len())
}

pub fn analyze_batch_from_reports(files: &[crate::report::FileReport], profile: &Profile) -> Vec<Finding> {
    let snapshots: Vec<BatchProbeSnapshot> = files
        .iter()
        .map(|file| BatchProbeSnapshot {
            path: file.path.clone(),
            width: file.width.unwrap_or(0),
            height: file.height.unwrap_or(0),
            video_codec: file
                .video_codec
                .clone()
                .unwrap_or_else(|| "unknown".into()),
            file_size_bytes: file.file_size_bytes.unwrap_or(0),
        })
        .collect();

    analyze_batch(&snapshots, profile)
}

pub fn analyze_batch(snapshots: &[BatchProbeSnapshot], profile: &Profile) -> Vec<Finding> {
    let mut findings = Vec::new();

    findings.extend(check_file_hygiene(snapshots, profile));
    findings.extend(check_duplicate_basenames(snapshots));
    findings.extend(check_mixed_specs(snapshots));

    findings
}

fn check_file_hygiene(snapshots: &[BatchProbeSnapshot], profile: &Profile) -> Vec<Finding> {
    let mut findings = Vec::new();

    for snapshot in snapshots {
        let basename = snapshot
            .path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("unknown");

        if snapshot.file_size_bytes == 0 {
            findings.push(Finding {
                code: "FILE_EMPTY".into(),
                severity: Severity::Fail,
                message: format!("{basename} is empty (0 bytes)"),
            });
        }

        if let Some(max_mb) = profile.max_file_size_mb {
            let max_bytes = max_mb.saturating_mul(1_048_576);
            if snapshot.file_size_bytes > max_bytes {
                let size_mb = snapshot.file_size_bytes as f64 / 1_048_576.0;
                findings.push(Finding {
                    code: "FILE_TOO_LARGE".into(),
                    severity: Severity::Fail,
                    message: format!(
                        "{basename} is {:.1} MB, above profile limit of {max_mb} MB",
                        size_mb
                    ),
                });
            }
        }
    }

    findings
}

fn check_duplicate_basenames(snapshots: &[BatchProbeSnapshot]) -> Vec<Finding> {
    let mut seen = HashMap::<String, usize>::new();

    for snapshot in snapshots {
        if let Some(name) = snapshot.path.file_name().and_then(|n| n.to_str()) {
            *seen.entry(name.to_ascii_lowercase()).or_default() += 1;
        }
    }

    seen.into_iter()
        .filter(|(_, count)| *count > 1)
        .map(|(name, count)| Finding {
            code: "DUPLICATE_BASENAME".into(),
            severity: Severity::Warn,
            message: format!("{count} files share the basename {name}"),
        })
        .collect()
}

fn check_mixed_specs(snapshots: &[BatchProbeSnapshot]) -> Vec<Finding> {
    let probed: Vec<&BatchProbeSnapshot> = snapshots
        .iter()
        .filter(|snapshot| snapshot.width > 0 && snapshot.height > 0)
        .collect();

    if probed.len() < 2 {
        return Vec::new();
    }

    let mut findings = Vec::new();

    let resolutions: HashSet<(u32, u32)> = probed
        .iter()
        .map(|snapshot| (snapshot.width, snapshot.height))
        .collect();
    if resolutions.len() > 1 {
        let labels: Vec<String> = resolutions
            .iter()
            .map(|(w, h)| format!("{w}x{h}"))
            .collect();
        findings.push(Finding {
            code: "BATCH_MIXED_RESOLUTION".into(),
            severity: Severity::Warn,
            message: format!("batch mixes resolutions: {}", labels.join(", ")),
        });
    }

    let codecs: HashSet<String> = probed
        .iter()
        .filter(|snapshot| snapshot.video_codec != "unknown")
        .map(|snapshot| snapshot.video_codec.clone())
        .collect();
    if codecs.len() > 1 {
        findings.push(Finding {
            code: "BATCH_MIXED_CODEC".into(),
            severity: Severity::Warn,
            message: format!(
                "batch mixes video codecs: {}",
                codecs.into_iter().collect::<Vec<_>>().join(", ")
            ),
        });
    }

    findings
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_mixed_resolutions() {
        let snapshots = vec![
            BatchProbeSnapshot {
                path: PathBuf::from("a.mp4"),
                width: 1920,
                height: 1080,
                video_codec: "h264".into(),
                file_size_bytes: 1_000,
            },
            BatchProbeSnapshot {
                path: PathBuf::from("b.mp4"),
                width: 1280,
                height: 720,
                video_codec: "h264".into(),
                file_size_bytes: 1_000,
            },
        ];

        let findings = check_mixed_specs(&snapshots);
        assert!(findings.iter().any(|f| f.code == "BATCH_MIXED_RESOLUTION"));
    }
}
