use crate::suggestions::{suggestions_for_findings, FixSuggestion};
use crate::validate::{Finding, Status, status_from_findings};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewState {
    Pass,
    Warn,
    NeedsHumanAttention,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileReport {
    pub path: PathBuf,
    pub status: Status,
    pub review_state: ReviewState,
    pub findings: Vec<Finding>,
    pub suggestions: Vec<FixSuggestion>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub video_codec: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_size_bytes: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BatchSummary {
    pub total: usize,
    pub passed: usize,
    pub warned: usize,
    pub failed: usize,
    pub errors: usize,
    pub needs_human_attention: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BatchReport {
    pub profile: String,
    pub files: Vec<FileReport>,
    pub summary: BatchSummary,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub batch_findings: Vec<Finding>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiProfileReport {
    pub profiles: Vec<String>,
    pub reports: Vec<BatchReport>,
}

impl MultiProfileReport {
    pub fn new(profiles: Vec<String>, reports: Vec<BatchReport>) -> Self {
        Self { profiles, reports }
    }

    pub fn from_single(report: BatchReport) -> Self {
        let profile = report.profile.clone();
        Self {
            profiles: vec![profile],
            reports: vec![report],
        }
    }

    pub fn overall_status(&self) -> Status {
        let mut status = Status::Pass;
        for report in &self.reports {
            status = worst_status(status, report.overall_status());
        }
        status
    }

    pub fn exit_code(&self) -> i32 {
        match self.overall_status() {
            Status::Pass => 0,
            Status::Warn => 2,
            Status::Fail => 1,
        }
    }
}

fn worst_status(current: Status, next: Status) -> Status {
    match (current, next) {
        (Status::Fail, _) | (_, Status::Fail) => Status::Fail,
        (Status::Warn, _) | (_, Status::Warn) => Status::Warn,
        _ => Status::Pass,
    }
}

impl FileReport {
    pub fn error(path: PathBuf, error: String) -> Self {
        let file_size_bytes = crate::batch::file_size_bytes(&path);
        Self {
            path,
            status: Status::Fail,
            review_state: ReviewState::NeedsHumanAttention,
            findings: vec![],
            suggestions: vec![],
            error: Some(error),
            width: None,
            height: None,
            video_codec: None,
            file_size_bytes,
        }
    }
}

impl BatchReport {
    pub fn from_files(profile: String, files: Vec<FileReport>) -> Self {
        Self::from_files_with_batch_findings(profile, files, Vec::new())
    }

    pub fn from_files_with_batch_findings(
        profile: String,
        files: Vec<FileReport>,
        batch_findings: Vec<Finding>,
    ) -> Self {
        let mut passed = 0;
        let mut warned = 0;
        let mut failed = 0;
        let mut errors = 0;
        let mut needs_human_attention = 0;

        for file in &files {
            if file.error.is_some() {
                errors += 1;
            }
            match file.status {
                Status::Pass => passed += 1,
                Status::Warn => warned += 1,
                Status::Fail => failed += 1,
            }
            if file.review_state == ReviewState::NeedsHumanAttention {
                needs_human_attention += 1;
            }
        }

        Self {
            profile,
            summary: BatchSummary {
                total: files.len(),
                passed,
                warned,
                failed,
                errors,
                needs_human_attention,
            },
            files,
            batch_findings,
        }
    }

    pub fn overall_status(&self) -> Status {
        if self.summary.errors > 0
            || self.summary.failed > 0
            || self
                .batch_findings
                .iter()
                .any(|finding| finding.severity == crate::validate::Severity::Fail)
        {
            Status::Fail
        } else if self.summary.warned > 0
            || self
                .batch_findings
                .iter()
                .any(|finding| finding.severity == crate::validate::Severity::Warn)
        {
            Status::Warn
        } else {
            Status::Pass
        }
    }

    pub fn exit_code(&self) -> i32 {
        match self.overall_status() {
            Status::Pass => 0,
            Status::Warn => 2,
            Status::Fail => 1,
        }
    }
}

pub fn file_report_from_findings(
    path: PathBuf,
    findings: Vec<Finding>,
    error: Option<String>,
) -> FileReport {
    let status = status_from_findings(&findings);
    let review_state = review_state_from(status);
    let suggestions = suggestions_for_findings(&findings);

    FileReport {
        path,
        status,
        review_state,
        findings,
        suggestions,
        error,
        width: None,
        height: None,
        video_codec: None,
        file_size_bytes: None,
    }
}

pub fn review_state_from(status: Status) -> ReviewState {
    match status {
        Status::Pass => ReviewState::Pass,
        Status::Warn => ReviewState::Warn,
        Status::Fail => ReviewState::NeedsHumanAttention,
    }
}

pub fn print_batch_report(report: &BatchReport) {
    println!(
        "Batch report for profile '{}' ({} file(s))",
        report.profile, report.summary.total
    );

    for file in &report.files {
        println!("\n{}", file.path.display());
        if let Some(error) = &file.error {
            println!("  error: {error}");
        }
        print_file_status(file);
    }

    println!("\nSummary:");
    println!("  passed: {}", report.summary.passed);
    println!("  warned: {}", report.summary.warned);
    println!("  failed: {}", report.summary.failed);
    println!("  errors: {}", report.summary.errors);
    println!(
        "  needs human attention: {}",
        report.summary.needs_human_attention
    );

    if !report.batch_findings.is_empty() {
        println!("\nBatch findings:");
        for finding in &report.batch_findings {
            println!(
                "  [{:?}] {} — {}",
                finding.severity, finding.code, finding.message
            );
        }
    }

    println!("  overall: {:?}", report.overall_status());
}

fn print_file_status(file: &FileReport) {
    println!("  status: {:?}", file.status);
    println!("  review: {:?}", file.review_state);

    if file.findings.is_empty() {
        println!("  no findings");
    } else {
        for finding in &file.findings {
            println!(
                "  [{:?}] {}: {}",
                finding.severity, finding.code, finding.message
            );
        }
    }

    if !file.suggestions.is_empty() {
        println!("  suggestions:");
        for suggestion in &file.suggestions {
            println!("    - {}: {}", suggestion.code, suggestion.suggestion);
        }
    }
}

pub fn print_json_report(report: &BatchReport) {
    println!(
        "{}",
        serde_json::to_string_pretty(report).expect("batch report should serialize")
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validate::{Finding, Severity};

    #[test]
    fn multi_profile_report_uses_worst_exit_code() {
        let pass = BatchReport::from_files("youtube".into(), vec![FileReport {
            path: PathBuf::from("a.mp4"),
            status: Status::Pass,
            review_state: ReviewState::Pass,
            findings: vec![],
            suggestions: vec![],
            error: None,
            width: None,
            height: None,
            video_codec: None,
            file_size_bytes: None,
        }]);
        let warn = BatchReport::from_files("shorts".into(), vec![FileReport {
            path: PathBuf::from("a.mp4"),
            status: Status::Warn,
            review_state: ReviewState::Warn,
            findings: vec![Finding {
                code: "BLACK_FRAMES".into(),
                severity: Severity::Warn,
                message: "black detected".into(),
            }],
            suggestions: vec![],
            error: None,
            width: None,
            height: None,
            video_codec: None,
            file_size_bytes: None,
        }]);

        let multi = MultiProfileReport::new(
            vec!["youtube".into(), "shorts".into()],
            vec![pass, warn],
        );
        assert_eq!(multi.overall_status(), Status::Warn);
        assert_eq!(multi.exit_code(), 2);
    }

    #[test]
    fn batch_summary_counts_statuses() {
        let files = vec![
            FileReport {
                path: PathBuf::from("a.mp4"),
                status: Status::Pass,
                review_state: ReviewState::Pass,
                findings: vec![],
                suggestions: vec![],
                error: None,
                width: None,
                height: None,
                video_codec: None,
                file_size_bytes: None,
            },
            FileReport {
                path: PathBuf::from("b.mp4"),
                status: Status::Fail,
                review_state: ReviewState::NeedsHumanAttention,
                findings: vec![Finding {
                    code: "RESOLUTION_MISMATCH".into(),
                    severity: Severity::Fail,
                    message: "bad".into(),
                }],
                suggestions: vec![],
                error: None,
                width: None,
                height: None,
                video_codec: None,
                file_size_bytes: None,
            },
            FileReport {
                path: PathBuf::from("c.mp4"),
                status: Status::Fail,
                review_state: ReviewState::NeedsHumanAttention,
                findings: vec![],
                suggestions: vec![],
                error: Some("probe failed".into()),
                width: None,
                height: None,
                video_codec: None,
                file_size_bytes: None,
            },
        ];

        let report = BatchReport::from_files("youtube".into(), files);
        assert_eq!(report.summary.total, 3);
        assert_eq!(report.summary.passed, 1);
        assert_eq!(report.summary.failed, 2);
        assert_eq!(report.summary.errors, 1);
        assert_eq!(report.summary.needs_human_attention, 2);
        assert_eq!(report.overall_status(), Status::Fail);
        assert_eq!(report.exit_code(), 1);
    }

    #[test]
    fn warn_only_batch_exits_with_code_2() {
        let files = vec![FileReport {
            path: PathBuf::from("a.mp4"),
            status: Status::Warn,
            review_state: ReviewState::Warn,
            findings: vec![Finding {
                code: "BLACK_FRAMES".into(),
                severity: Severity::Warn,
                message: "black detected".into(),
            }],
            suggestions: vec![],
            error: None,
            width: None,
            height: None,
            video_codec: None,
            file_size_bytes: None,
        }];

        let report = BatchReport::from_files("youtube".into(), files);
        assert_eq!(report.overall_status(), Status::Warn);
        assert_eq!(report.exit_code(), 2);
    }

    #[test]
    fn file_report_includes_suggestions() {
        let findings = vec![Finding {
            code: "AUDIO_REQUIRED".into(),
            severity: Severity::Fail,
            message: "missing".into(),
        }];
        let report = file_report_from_findings(PathBuf::from("clip.mp4"), findings, None);
        assert_eq!(report.review_state, ReviewState::NeedsHumanAttention);
        assert!(!report.suggestions.is_empty());
    }
}
