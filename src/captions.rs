use crate::validate::{Finding, Severity};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CaptionStats {
    pub cue_count: usize,
    pub last_end_secs: f64,
}

pub fn validate_captions(path: &Path, video_duration_secs: f64) -> Result<CaptionStats, Finding> {
    let raw = std::fs::read_to_string(path).map_err(|err| Finding {
        code: "CAPTIONS_UNREADABLE".into(),
        severity: Severity::Fail,
        message: format!("could not read captions file: {err}"),
    })?;

    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(Finding {
            code: "CAPTIONS_EMPTY".into(),
            severity: Severity::Fail,
            message: "caption file is empty".into(),
        });
    }

    let extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    let stats = match extension.as_str() {
        "srt" => parse_srt(trimmed)?,
        "vtt" => parse_vtt(trimmed)?,
        _ => {
            return Err(Finding {
                code: "CAPTIONS_UNSUPPORTED".into(),
                severity: Severity::Warn,
                message: format!("unsupported caption format .{extension}"),
            });
        }
    };

    if stats.cue_count == 0 {
        return Err(Finding {
            code: "CAPTIONS_EMPTY".into(),
            severity: Severity::Fail,
            message: "caption file has no cues".into(),
        });
    }

    if stats.last_end_secs + 0.5 < video_duration_secs {
        return Err(Finding {
            code: "CAPTIONS_SHORT".into(),
            severity: Severity::Warn,
            message: format!(
                "captions end at {:.1}s but video is {:.1}s",
                stats.last_end_secs, video_duration_secs
            ),
        });
    }

    Ok(stats)
}

fn parse_srt(raw: &str) -> Result<CaptionStats, Finding> {
    let mut cue_count = 0;
    let mut last_end_secs: f64 = 0.0;

    for block in raw.split("\n\n") {
        let lines: Vec<&str> = block
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .collect();

        if lines.len() < 2 {
            continue;
        }

        let timing_line = if lines[0].chars().all(|ch| ch.is_ascii_digit()) && lines.len() >= 3 {
            lines[1]
        } else {
            lines[0]
        };

        let (start, end) = parse_timestamp_line(timing_line).ok_or_else(|| Finding {
            code: "CAPTIONS_MALFORMED".into(),
            severity: Severity::Fail,
            message: format!("invalid SRT timing line: {timing_line}"),
        })?;

        if end < start {
            return Err(Finding {
                code: "CAPTIONS_MALFORMED".into(),
                severity: Severity::Fail,
                message: "caption cue ends before it starts".into(),
            });
        }

        cue_count += 1;
        last_end_secs = last_end_secs.max(end);
    }

    Ok(CaptionStats {
        cue_count,
        last_end_secs,
    })
}

fn parse_vtt(raw: &str) -> Result<CaptionStats, Finding> {
    let mut cue_count = 0;
    let mut last_end_secs: f64 = 0.0;

    for line in raw.lines() {
        let line = line.trim();
        if !line.contains("-->") {
            continue;
        }

        let (start, end) = parse_timestamp_line(line).ok_or_else(|| Finding {
            code: "CAPTIONS_MALFORMED".into(),
            severity: Severity::Fail,
            message: format!("invalid VTT timing line: {line}"),
        })?;

        cue_count += 1;
        last_end_secs = last_end_secs.max(end);
    }

    Ok(CaptionStats {
        cue_count,
        last_end_secs,
    })
}

fn parse_timestamp_line(line: &str) -> Option<(f64, f64)> {
    let (start_raw, end_raw) = line.split_once("-->")?;
    let start = parse_timestamp(start_raw.trim())?;
    let end = parse_timestamp(end_raw.trim().split_whitespace().next()?)?;
    Some((start, end))
}

fn parse_timestamp(value: &str) -> Option<f64> {
    let normalized = value.replace(',', ".");
    let (hms, millis) = if let Some((left, right)) = normalized.rsplit_once('.') {
        (left, right.parse::<u32>().ok()?)
    } else {
        (normalized.as_str(), 0)
    };

    let mut parts = hms.split(':');
    let seconds: f64 = parts.next_back()?.parse().ok()?;
    let minutes: f64 = parts.next_back().and_then(|v| v.parse().ok()).unwrap_or(0.0);
    let hours: f64 = parts.next_back().and_then(|v| v.parse().ok()).unwrap_or(0.0);

    Some(hours * 3600.0 + minutes * 60.0 + seconds + millis as f64 / 1000.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_srt_counts_cues() {
        let raw = "1\n00:00:01,000 --> 00:00:03,000\nHello\n\n2\n00:00:04,000 --> 00:00:06,000\nWorld\n";
        let stats = parse_srt(raw).expect("stats");
        assert_eq!(stats.cue_count, 2);
        assert!((stats.last_end_secs - 6.0).abs() < 0.01);
    }

    #[test]
    fn parse_vtt_counts_cues() {
        let raw = "WEBVTT\n\n00:00:01.000 --> 00:00:03.000\nHello\n";
        let stats = parse_vtt(raw).expect("stats");
        assert_eq!(stats.cue_count, 1);
    }

    #[test]
    fn validate_captions_flags_short_coverage() {
        let dir = std::env::temp_dir().join(format!("mediaqa_caps_{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("dir");
        let path = dir.join("clip.srt");
        std::fs::write(&path, "1\n00:00:00,000 --> 00:00:02,000\nHi\n").expect("write");

        let err = validate_captions(&path, 30.0).expect_err("short");
        assert_eq!(err.code, "CAPTIONS_SHORT");

        let _ = std::fs::remove_dir_all(dir);
    }
}
