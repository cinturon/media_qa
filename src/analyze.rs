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
                code: "DECODE_ERROR",
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
        code: "DECODE_FAILED",
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
                code: "BLACK_FRAME_ERROR",
                severity: Severity::Fail,
                message: format!("failed to run black frame analysis: {err}"),
            }];
        }
    };

    let stderr = String::from_utf8_lossy(&output.stderr);
    let mut findings = Vec::new();

    for line in stderr.lines() {
        if line.contains("black_duration:") {
            findings.push(Finding {
                code: "BLACK_FRAMES",
                severity: Severity::Warn,
                message: line.trim().to_string(),
            });
        }
    }

    findings
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
    }

    #[test]
    fn analyze_black_frames_returns_empty_when_none_found() {
        let findings = parse_blackdetect_stderr("frame= 30 fps=0.0 q=-0.0 Lsize=-0KiB\n");
        assert!(findings.is_empty());
    }

    fn parse_blackdetect_stderr(stderr: &str) -> Vec<Finding> {
        let mut findings = Vec::new();
        for line in stderr.lines() {
            if line.contains("black_duration:") {
                findings.push(Finding {
                    code: "BLACK_FRAMES",
                    severity: Severity::Warn,
                    message: line.trim().to_string(),
                });
            }
        }
        findings
    }
}
