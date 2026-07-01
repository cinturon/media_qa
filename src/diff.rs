use crate::report::BatchReport;
use crate::suggestions::suggestions_for_findings;
use crate::validate::{Finding, Status};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportDiff {
    pub added_files: Vec<String>,
    pub removed_files: Vec<String>,
    pub changed_files: Vec<FileDiff>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileDiff {
    pub path: String,
    pub before_status: Status,
    pub after_status: Status,
    pub added_findings: Vec<Finding>,
    pub removed_findings: Vec<Finding>,
}

pub fn diff_reports(before: &BatchReport, after: &BatchReport) -> ReportDiff {
    let before_paths: std::collections::HashMap<_, _> = before
        .files
        .iter()
        .map(|file| (file.path.display().to_string(), file))
        .collect();
    let after_paths: std::collections::HashMap<_, _> = after
        .files
        .iter()
        .map(|file| (file.path.display().to_string(), file))
        .collect();

    let added_files = after_paths
        .keys()
        .filter(|path| !before_paths.contains_key(*path))
        .cloned()
        .collect();

    let removed_files = before_paths
        .keys()
        .filter(|path| !after_paths.contains_key(*path))
        .cloned()
        .collect();

    let mut changed_files = Vec::new();
    for (path, after_file) in &after_paths {
        if let Some(before_file) = before_paths.get(path) {
            if before_file.status != after_file.status
                || before_file.findings != after_file.findings
            {
                changed_files.push(FileDiff {
                    path: path.clone(),
                    before_status: before_file.status,
                    after_status: after_file.status,
                    added_findings: finding_delta(&before_file.findings, &after_file.findings),
                    removed_findings: finding_delta(&after_file.findings, &before_file.findings),
                });
            }
        }
    }

    ReportDiff {
        added_files,
        removed_files,
        changed_files,
    }
}

fn finding_delta(left: &[Finding], right: &[Finding]) -> Vec<Finding> {
    right
        .iter()
        .filter(|finding| !left.contains(finding))
        .cloned()
        .collect()
}

pub fn print_report_diff(diff: &ReportDiff) {
    println!("QC run diff");
    if !diff.added_files.is_empty() {
        println!("Added files:");
        for path in &diff.added_files {
            println!("  + {path}");
        }
    }
    if !diff.removed_files.is_empty() {
        println!("Removed files:");
        for path in &diff.removed_files {
            println!("  - {path}");
        }
    }
    for change in &diff.changed_files {
        println!(
            "\n{}: {:?} -> {:?}",
            change.path, change.before_status, change.after_status
        );
        for finding in &change.added_findings {
            println!("  + [{}] {}", finding.code, finding.message);
            for suggestion in suggestions_for_findings(std::slice::from_ref(finding)) {
                println!("    fix: {}", suggestion.suggestion);
            }
        }
        for finding in &change.removed_findings {
            println!("  - [{}] {}", finding.code, finding.message);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::report::{BatchReport, FileReport};
    use crate::validate::Severity;
    use std::path::PathBuf;

    #[test]
    fn diff_reports_detects_status_change() {
        let before = BatchReport::from_files(
            "youtube".into(),
            vec![FileReport {
                path: PathBuf::from("clip.mp4"),
                status: Status::Pass,
                review_state: crate::report::ReviewState::Pass,
                findings: vec![],
                suggestions: vec![],
                error: None,
            }],
        );
        let after = BatchReport::from_files(
            "youtube".into(),
            vec![FileReport {
                path: PathBuf::from("clip.mp4"),
                status: Status::Fail,
                review_state: crate::report::ReviewState::NeedsHumanAttention,
                findings: vec![Finding {
                    code: "AUDIO_REQUIRED".into(),
                    severity: Severity::Fail,
                    message: "missing audio".into(),
                }],
                suggestions: vec![],
                error: None,
            }],
        );

        let diff = diff_reports(&before, &after);
        assert_eq!(diff.changed_files.len(), 1);
        assert_eq!(diff.changed_files[0].after_status, Status::Fail);
    }
}
