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

    BatchReport::from_files(profile_name.to_string(), files)
}

fn run_batch_sequential(
    paths: Vec<PathBuf>,
    profile: &Profile,
    profile_name: &str,
) -> BatchReport {
    let packages = discover_packages(&paths);
    let files = packages
        .iter()
        .map(|package| process_package(package, profile))
        .collect();

    BatchReport::from_files(profile_name.to_string(), files)
}

fn process_package(package: &DeliveryPackage, profile: &Profile) -> FileReport {
    let path = &package.video;

    let metadata = match probe_media(path) {
        Ok(metadata) => metadata,
        Err(err) => {
            return FileReport::error(path.to_path_buf(), err);
        }
    };

    let mut findings = run_media_checks(path, &metadata, profile);
    findings.extend(run_package_checks(package, profile));

    file_report_from_findings(path.to_path_buf(), findings, None)
}
