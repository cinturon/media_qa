use crate::config::AppConfig;
use crate::report::MultiProfileReport;
use crate::batch::{analyze_batch_from_reports, file_size_bytes};
use crate::checks::{run_media_checks, run_package_checks};
use crate::config::Profile;
use crate::package::{discover_packages, DeliveryPackage};
use crate::probe::probe_media;
use crate::report::{file_report_from_findings, BatchReport, FileReport};
use rayon::prelude::*;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy)]
pub struct BatchOptions {
    pub parallel: bool,
}

impl Default for BatchOptions {
    fn default() -> Self {
        Self { parallel: true }
    }
}

pub fn run_batch(
    paths: Vec<PathBuf>,
    profile: &Profile,
    profile_name: &str,
    options: BatchOptions,
) -> BatchReport {
    if options.parallel {
        run_batch_parallel(paths, profile, profile_name, options)
    } else {
        run_batch_sequential(paths, profile, profile_name)
    }
}

pub fn run_batch_parallel(
    paths: Vec<PathBuf>,
    profile: &Profile,
    profile_name: &str,
    _options: BatchOptions,
) -> BatchReport {
    let packages = discover_packages(&paths);
    let files: Vec<FileReport> = packages
        .par_iter()
        .map(|package| process_package(package, profile))
        .collect();

    let batch_findings = analyze_batch_from_reports(&files, profile);
    BatchReport::from_files_with_batch_findings(profile_name.to_string(), files, batch_findings)
}

fn run_batch_sequential(
    paths: Vec<PathBuf>,
    profile: &Profile,
    profile_name: &str,
) -> BatchReport {
    run_batch_sequential_with_progress(paths, profile, profile_name, |_| {})
}

#[derive(Debug, Clone)]
pub struct BatchProgress {
    pub phase: &'static str,
    pub message: String,
    pub current: usize,
    pub total: usize,
}

pub fn run_batch_sequential_with_progress<F>(
    paths: Vec<PathBuf>,
    profile: &Profile,
    profile_name: &str,
    mut on_progress: F,
) -> BatchReport
where
    F: FnMut(BatchProgress),
{
    on_progress(BatchProgress {
        phase: "discover",
        message: "Grouping delivery packages…".to_string(),
        current: 0,
        total: 0,
    });

    let packages = discover_packages(&paths);
    let total = packages.len();

    on_progress(BatchProgress {
        phase: "start",
        message: if total == 1 {
            "Checking 1 file…".to_string()
        } else {
            format!("Checking {total} files…")
        },
        current: 0,
        total,
    });

    let files = packages
        .iter()
        .enumerate()
        .map(|(index, package)| {
            let label = package
                .video
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("media file")
                .to_string();

            on_progress(BatchProgress {
                phase: "file",
                message: format!("Probing and validating {label}"),
                current: index + 1,
                total,
            });

            process_package(package, profile)
        })
        .collect::<Vec<FileReport>>();

    on_progress(BatchProgress {
        phase: "finish",
        message: "Building report…".to_string(),
        current: total,
        total,
    });

    let batch_findings = analyze_batch_from_reports(&files, profile);
    BatchReport::from_files_with_batch_findings(profile_name.to_string(), files, batch_findings)
}

pub fn run_multi_profile_sequential_with_progress<F>(
    paths: Vec<PathBuf>,
    config: &AppConfig,
    profile_names: &[String],
    mut on_progress: F,
) -> Result<MultiProfileReport, String>
where
    F: FnMut(BatchProgress, &str),
{
    if profile_names.is_empty() {
        return Err("At least one profile is required".into());
    }

    let mut reports = Vec::with_capacity(profile_names.len());
    for profile_name in profile_names {
        let profile = config
            .profiles
            .get(profile_name)
            .cloned()
            .ok_or_else(|| format!("Profile not found: {profile_name}"))?;

        on_progress(
            BatchProgress {
                phase: "profile",
                message: format!("Running {profile_name} profile…"),
                current: 0,
                total: 0,
            },
            profile_name,
        );

        let report = run_batch_sequential_with_progress(
            paths.clone(),
            &profile,
            profile_name,
            |progress| on_progress(progress, profile_name),
        );
        reports.push(report);
    }

    Ok(MultiProfileReport::new(profile_names.to_vec(), reports))
}

fn process_package(package: &DeliveryPackage, profile: &Profile) -> FileReport {
    let path = &package.video;
    let size_bytes = file_size_bytes(path);

    let metadata = match probe_media(path) {
        Ok(metadata) => metadata,
        Err(err) => {
            let mut report = FileReport::error(path.to_path_buf(), err);
            report.file_size_bytes = size_bytes;
            return report;
        }
    };

    let mut findings = run_media_checks(path, &metadata, profile);
    findings.extend(run_package_checks(package, &metadata, profile));

    let mut report = file_report_from_findings(path.to_path_buf(), findings, None);
    report.width = Some(metadata.width);
    report.height = Some(metadata.height);
    report.video_codec = metadata.video_codec.clone();
    report.file_size_bytes = size_bytes;
    report
}
