use crate::analyze::{analyze_black_frames, analyze_decode};
use crate::config::Profile;
use crate::probe::probe_media;
use crate::report::{file_report_from_findings, BatchReport, FileReport};
use crate::validate::validate;
use std::path::{Path, PathBuf};

pub fn run_batch(paths: Vec<PathBuf>, profile: &Profile, profile_name: &str) -> BatchReport {
    let files = paths
        .into_iter()
        .map(|path| process_file(&path, profile))
        .collect();

    BatchReport::from_files(profile_name.to_string(), files)
}

fn process_file(path: &Path, profile: &Profile) -> FileReport {
    let metadata = match probe_media(path) {
        Ok(metadata) => metadata,
        Err(err) => {
            return FileReport {
                path: path.to_path_buf(),
                status: crate::validate::Status::Fail,
                findings: vec![],
                error: Some(err),
            };
        }
    };

    let mut findings = validate(path, &metadata, profile).findings;
    findings.extend(analyze_decode(path));
    findings.extend(analyze_black_frames(path));

    file_report_from_findings(path.to_path_buf(), findings, None)
}
