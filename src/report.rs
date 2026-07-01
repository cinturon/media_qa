use crate::validate::{Finding, Status, status_from_findings};
use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FileReport {
    pub path: PathBuf,
    pub status: Status,
    pub findings: Vec<Finding>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BatchSummary {
    pub total: usize,
    pub passed: usize,
    pub warned: usize,
    pub failed: usize,
    pub errors: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BatchReport {
    pub profile: String,
    pub files: Vec<FileReport>,
    pub summary: BatchSummary,
}

impl BatchReport {
    pub fn from_files(profile: String, files: Vec<FileReport>) -> Self {
        let mut passed = 0;
        let mut warned = 0;
        let mut failed = 0;
        let mut errors = 0;

        for file in &files {
            if file.error.is_some() {
                errors += 1;
            }
            match file.status {
                Status::Pass => passed += 1,
                Status::Warn => warned += 1,
                Status::Fail => failed += 1,
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
            },
            files,
        }
    }

    pub fn overall_status(&self) -> Status {
        if self.summary.errors > 0 || self.summary.failed > 0 {
            Status::Fail
        } else if self.summary.warned > 0 {
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
    FileReport {
        path,
        status: status_from_findings(&findings),
        findings,
        error,
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
    println!("  overall: {:?}", report.overall_status());
}

fn print_file_status(file: &FileReport) {
    println!("  status: {:?}", file.status);

    if file.findings.is_empty() {
        println!("  no findings");
        return;
    }

    for finding in &file.findings {
        println!(
            "  [{:?}] {}: {}",
            finding.severity, finding.code, finding.message
        );
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
    fn batch_summary_counts_statuses() {
        let files = vec![
            FileReport {
                path: PathBuf::from("a.mp4"),
                status: Status::Pass,
                findings: vec![],
                error: None,
            },
            FileReport {
                path: PathBuf::from("b.mp4"),
                status: Status::Fail,
                findings: vec![Finding {
                    code: "RESOLUTION_MISMATCH",
                    severity: Severity::Fail,
                    message: "bad".into(),
                }],
                error: None,
            },
            FileReport {
                path: PathBuf::from("c.mp4"),
                status: Status::Fail,
                findings: vec![],
                error: Some("probe failed".into()),
            },
        ];

        let report = BatchReport::from_files("youtube".into(), files);
        assert_eq!(report.summary.total, 3);
        assert_eq!(report.summary.passed, 1);
        assert_eq!(report.summary.failed, 2);
        assert_eq!(report.summary.errors, 1);
        assert_eq!(report.overall_status(), Status::Fail);
        assert_eq!(report.exit_code(), 1);
    }

    #[test]
    fn warn_only_batch_exits_with_code_2() {
        let files = vec![FileReport {
            path: PathBuf::from("a.mp4"),
            status: Status::Warn,
            findings: vec![Finding {
                code: "BLACK_FRAMES",
                severity: Severity::Warn,
                message: "black detected".into(),
            }],
            error: None,
        }];

        let report = BatchReport::from_files("youtube".into(), files);
        assert_eq!(report.overall_status(), Status::Warn);
        assert_eq!(report.exit_code(), 2);
    }
}
