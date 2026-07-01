use crate::config::Profile;
use crate::validate::{Finding, Severity};
use std::path::Path;
use std::process::Command;

pub fn analyze_decode(path: &Path) -> Vec<Finding> {
    let output = match Command::new("ffmpeg")
        .args(["-v", "error", "-i"])
        .arg(path)
        .args(["-f", "null", "-"])
        .output()
    {
        Ok(output) => output,
        Err(err) => {
            return vec![Finding {
                code: "DECODE_ERROR".into(),
                severity: Severity::Fail,
                message: format!("failed to run ffmpeg decode check: {err}"),
            }];
        }
    };

    if output.status.success() {
        return Vec::new();
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let message = if stderr.is_empty() {
        "ffmpeg decode check failed".into()
    } else {
        stderr
    };

    vec![Finding {
        code: "DECODE_FAILED".into(),
        severity: Severity::Fail,
        message,
    }]
}

pub fn analyze_black_frames(path: &Path) -> Vec<Finding> {
    let output = match Command::new("ffmpeg")
        .arg("-i")
        .arg(path)
        .args(["-vf", "blackdetect=d=0.1:pix_th=0.10", "-an", "-f", "null", "-"])
        .output()
    {
        Ok(output) => output,
        Err(err) => {
            return vec![Finding {
                code: "BLACK_FRAME_ERROR".into(),
                severity: Severity::Fail,
                message: format!("failed to run black frame analysis: {err}"),
            }];
        }
    };

    parse_blackdetect_stderr(&String::from_utf8_lossy(&output.stderr))
}

pub fn analyze_frozen_frames(path: &Path) -> Vec<Finding> {
    let output = match Command::new("ffmpeg")
        .arg("-i")
        .arg(path)
        .args(["-vf", "freezedetect=n=0.003:d=2", "-an", "-f", "null", "-"])
        .output()
    {
        Ok(output) => output,
        Err(err) => {
            return vec![Finding {
                code: "FROZEN_FRAME_ERROR".into(),
                severity: Severity::Fail,
                message: format!("failed to run frozen frame analysis: {err}"),
            }];
        }
    };

    parse_freezedetect_stderr(&String::from_utf8_lossy(&output.stderr))
}

pub fn analyze_integrated_loudness(path: &Path, profile: &Profile) -> Vec<Finding> {
    if profile.max_integrated_lufs.is_none() && profile.max_true_peak_db.is_none() {
        return Vec::new();
    }

    let output = match Command::new("ffmpeg")
        .arg("-i")
        .arg(path)
        .args(["-af", "ebur128=peak=true", "-f", "null", "-"])
        .output()
    {
        Ok(output) => output,
        Err(err) => {
            return vec![Finding {
                code: "LOUDNESS_ERROR".into(),
                severity: Severity::Fail,
                message: format!("failed to run loudness analysis: {err}"),
            }];
        }
    };

    let Some((integrated_lufs, true_peak_db)) =
        parse_ebur128_stderr(&String::from_utf8_lossy(&output.stderr))
    else {
        return vec![Finding {
            code: "LOUDNESS_ERROR".into(),
            severity: Severity::Warn,
            message: "could not parse integrated loudness results".into(),
        }];
    };

    let mut findings = Vec::new();

    if let Some(max_lufs) = profile.max_integrated_lufs {
        if integrated_lufs > max_lufs {
            findings.push(Finding {
                code: "LOUDNESS_TOO_HIGH".into(),
                severity: Severity::Warn,
                message: format!(
                    "integrated loudness is {integrated_lufs:.1} LUFS, above target of {max_lufs:.1} LUFS"
                ),
            });
        }
    }

    if let Some(max_peak) = profile.max_true_peak_db {
        if true_peak_db > max_peak {
            findings.push(Finding {
                code: "TRUE_PEAK_TOO_HIGH".into(),
                severity: Severity::Warn,
                message: format!(
                    "true peak is {true_peak_db:.1} dBFS, above limit of {max_peak:.1} dBFS"
                ),
            });
        }
    }

    findings
}

pub fn analyze_audio(path: &Path, profile: &Profile) -> Vec<Finding> {
    let mut findings = Vec::new();
    findings.extend(analyze_silence(path, profile.max_silence_secs));
    findings.extend(analyze_loudness(path, profile.min_mean_volume_db));
    findings
}

fn analyze_silence(path: &Path, max_silence_secs: f64) -> Vec<Finding> {
    let output = match Command::new("ffmpeg")
        .arg("-i")
        .arg(path)
        .args([
            "-af",
            &format!("silencedetect=noise=-50dB:d={max_silence_secs}"),
            "-f",
            "null",
            "-",
        ])
        .output()
    {
        Ok(output) => output,
        Err(err) => {
            return vec![Finding {
                code: "AUDIO_ANALYSIS_ERROR".into(),
                severity: Severity::Fail,
                message: format!("failed to run silence analysis: {err}"),
            }];
        }
    };

    let stderr = String::from_utf8_lossy(&output.stderr);
    let mut findings = Vec::new();

    for line in stderr.lines() {
        if line.contains("silence_duration:") {
            findings.push(Finding {
                code: "SILENCE_DETECTED".into(),
                severity: Severity::Warn,
                message: line.trim().to_string(),
            });
        }
    }

    findings
}

fn analyze_loudness(path: &Path, min_mean_volume_db: f64) -> Vec<Finding> {
    let output = match Command::new("ffmpeg")
        .args(["-i"])
        .arg(path)
        .args(["-af", "volumedetect", "-f", "null", "-"])
        .output()
    {
        Ok(output) => output,
        Err(err) => {
            return vec![Finding {
                code: "AUDIO_ANALYSIS_ERROR".into(),
                severity: Severity::Fail,
                message: format!("failed to run loudness analysis: {err}"),
            }];
        }
    };

    let stderr = String::from_utf8_lossy(&output.stderr);
    for line in stderr.lines() {
        if let Some(value) = line.strip_prefix("mean_volume: ") {
            if let Ok(mean_db) = value.trim_end_matches(" dB").parse::<f64>() {
                if mean_db < min_mean_volume_db {
                    return vec![Finding {
                        code: "AUDIO_TOO_QUIET".into(),
                        severity: Severity::Warn,
                        message: format!(
                            "mean volume {mean_db:.1} dB is below minimum {min_mean_volume_db:.1} dB"
                        ),
                    }];
                }
            }
            break;
        }
    }

    Vec::new()
}

fn parse_freezedetect_stderr(stderr: &str) -> Vec<Finding> {
    let segments = stderr
        .lines()
        .filter_map(parse_freezedetect_line)
        .collect::<Vec<_>>();

    if segments.is_empty() {
        return Vec::new();
    }

    vec![Finding {
        code: "FROZEN_FRAMES".into(),
        severity: Severity::Warn,
        message: format_frozen_frame_report(&segments),
    }]
}

fn parse_freezedetect_line(line: &str) -> Option<(f64, f64, f64)> {
    let mut start = None;
    let mut end = None;
    let mut duration = None;

    for part in line.split_whitespace() {
        if let Some(value) = part.strip_prefix("freeze_start:") {
            start = value.parse().ok();
        } else if let Some(value) = part.strip_prefix("freeze_end:") {
            end = value.parse().ok();
        } else if let Some(value) = part.strip_prefix("freeze_duration:") {
            duration = value.parse().ok();
        }
    }

    match (start, end, duration) {
        (Some(start), Some(end), Some(duration)) => Some((start, end, duration)),
        _ => None,
    }
}

fn format_frozen_frame_report(segments: &[(f64, f64, f64)]) -> String {
    let total_duration: f64 = segments.iter().map(|(_, _, duration)| duration).sum();

    if segments.len() == 1 {
        let (start, end, duration) = segments[0];
        return format!(
            "Frozen picture from {} to {} — {}",
            format_timestamp(start),
            format_timestamp(end),
            format_duration(duration)
        );
    }

    let lines = segments
        .iter()
        .map(|(start, end, duration)| {
            format!(
                "  - {} to {} — {}",
                format_timestamp(*start),
                format_timestamp(*end),
                format_duration(*duration)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        "{} frozen segments found ({} total):\n{}",
        segments.len(),
        format_duration(total_duration),
        lines
    )
}

fn parse_ebur128_stderr(stderr: &str) -> Option<(f64, f64)> {
    let mut integrated = None;
    let mut peak = None;

    for line in stderr.lines() {
        let trimmed = line.trim();
        if let Some(value) = trimmed.strip_prefix("I:") {
            integrated = value
                .trim()
                .strip_suffix("LUFS")
                .and_then(|v| v.trim().parse().ok());
        }
        if let Some(value) = trimmed.strip_prefix("Peak:") {
            peak = value
                .trim()
                .strip_suffix("dBFS")
                .and_then(|v| v.trim().parse().ok());
        }
    }

    match (integrated, peak) {
        (Some(integrated_lufs), Some(true_peak_db)) => Some((integrated_lufs, true_peak_db)),
        _ => None,
    }
}

fn parse_blackdetect_stderr(stderr: &str) -> Vec<Finding> {
    let segments = stderr
        .lines()
        .filter_map(parse_blackdetect_line)
        .collect::<Vec<_>>();

    if segments.is_empty() {
        return Vec::new();
    }

    vec![Finding {
        code: "BLACK_FRAMES".into(),
        severity: Severity::Warn,
        message: format_black_frame_report(&segments),
    }]
}

fn parse_blackdetect_line(line: &str) -> Option<(f64, f64, f64)> {
    let mut start = None;
    let mut end = None;
    let mut duration = None;

    for part in line.split_whitespace() {
        if let Some(value) = part.strip_prefix("black_start:") {
            start = value.parse().ok();
        } else if let Some(value) = part.strip_prefix("black_end:") {
            end = value.parse().ok();
        } else if let Some(value) = part.strip_prefix("black_duration:") {
            duration = value.parse().ok();
        }
    }

    match (start, end, duration) {
        (Some(start), Some(end), Some(duration)) => Some((start, end, duration)),
        _ => None,
    }
}

fn format_black_frame_report(segments: &[(f64, f64, f64)]) -> String {
    let total_duration: f64 = segments.iter().map(|(_, _, duration)| duration).sum();

    if segments.len() == 1 {
        let (start, end, duration) = segments[0];
        return format_single_black_segment(start, end, duration);
    }

    let lines = segments
        .iter()
        .map(|(start, end, duration)| format!("  - {}", describe_black_segment(*start, *end, *duration)))
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        "{} black segments found ({} total):\n{}",
        segments.len(),
        format_duration(total_duration),
        lines
    )
}

fn format_single_black_segment(start: f64, end: f64, duration: f64) -> String {
    if start <= 0.05 {
        return format!(
            "Black frames at the start of the video — {}",
            format_duration(duration)
        );
    }

    format!(
        "Black frames from {} to {} — {}",
        format_timestamp(start),
        format_timestamp(end),
        format_duration(duration)
    )
}

fn describe_black_segment(start: f64, end: f64, duration: f64) -> String {
    if start <= 0.05 {
        format!("Start of video — {}", format_duration(duration))
    } else {
        format!(
            "{} to {} — {}",
            format_timestamp(start),
            format_timestamp(end),
            format_duration(duration)
        )
    }
}

fn format_timestamp(secs: f64) -> String {
    if secs < 60.0 {
        format_seconds(secs)
    } else {
        let total = secs.max(0.0).round() as u64;
        let minutes = total / 60;
        let seconds = total % 60;
        format!("{minutes}:{seconds:02}")
    }
}

fn format_duration(secs: f64) -> String {
    let rounded = (secs * 10.0).round() / 10.0;
    if (rounded - 1.0).abs() < 0.05 {
        "1 second".to_string()
    } else if rounded.fract() < 0.05 {
        format!("{} seconds", rounded.round() as i64)
    } else {
        format!("{rounded:.1} seconds")
    }
}

fn format_seconds(secs: f64) -> String {
    let rounded = (secs * 10.0).round() / 10.0;
    if rounded.fract() < 0.05 {
        format!("{}s", rounded.round() as i64)
    } else {
        format!("{rounded:.1}s")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn analyze_black_frames_parses_blackdetect_lines() {
        let findings = parse_blackdetect_stderr(
            "[blackdetect @ 0x1] black_start:0 black_end:1.5 black_duration:1.5\n",
        );
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].code, "BLACK_FRAMES");
        assert_eq!(
            findings[0].message,
            "Black frames at the start of the video — 1.5 seconds"
        );
    }

    #[test]
    fn analyze_black_frames_formats_mid_clip_segment() {
        let findings = parse_blackdetect_stderr(
            "[blackdetect @ 0x1] black_start:10.2 black_end:10.8 black_duration:0.6\n",
        );
        assert_eq!(findings.len(), 1);
        assert_eq!(
            findings[0].message,
            "Black frames from 10.2s to 10.8s — 0.6 seconds"
        );
    }

    #[test]
    fn analyze_black_frames_summarizes_multiple_segments() {
        let findings = parse_blackdetect_stderr(
            "[blackdetect @ 0x1] black_start:0 black_end:1.5 black_duration:1.5\n\
             [blackdetect @ 0x1] black_start:10.2 black_end:10.8 black_duration:0.6\n",
        );
        assert_eq!(findings.len(), 1);
        assert!(findings[0].message.starts_with("2 black segments found"));
        assert!(findings[0].message.contains("Start of video"));
        assert!(findings[0].message.contains("10.2s to 10.8s"));
    }

    #[test]
    fn analyze_black_frames_returns_empty_when_none_found() {
        let findings = parse_blackdetect_stderr("frame= 30 fps=0.0 q=-0.0 Lsize=-0KiB\n");
        assert!(findings.is_empty());
    }

    #[test]
    fn analyze_frozen_frames_formats_segment() {
        let findings = parse_freezedetect_stderr(
            "[freezedetect @ 0x1] freeze_start:4.0 freeze_end:6.5 freeze_duration:2.5\n",
        );
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].code, "FROZEN_FRAMES");
        assert!(findings[0].message.contains("Frozen picture"));
    }

    #[test]
    fn parse_ebur128_extracts_lufs_and_peak() {
        let parsed = parse_ebur128_stderr(
            "  Integrated loudness:\n    I:         -14.2 LUFS\n  True peak:\n    Peak:       -1.1 dBFS\n",
        )
        .expect("parsed");
        assert!((parsed.0 + 14.2).abs() < 0.01);
        assert!((parsed.1 + 1.1).abs() < 0.01);
    }
}
