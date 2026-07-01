use crate::analyze::{analyze_audio, analyze_black_frames, analyze_decode, analyze_frozen_frames, analyze_integrated_loudness};
use crate::config::Profile;
use crate::metadata::MediaMetadata;
use crate::package::{validate_package, DeliveryPackage};
use crate::validate::{validate, Finding};
use std::path::Path;

pub fn run_media_checks(path: &Path, metadata: &MediaMetadata, profile: &Profile) -> Vec<Finding> {
    let mut findings = validate(path, metadata, profile).findings;
    findings.extend(analyze_decode(path));
    findings.extend(analyze_black_frames(path));
    findings.extend(analyze_frozen_frames(path));
    findings.extend(analyze_audio(path, profile));
    findings.extend(analyze_integrated_loudness(path, profile));
    findings
}

pub fn run_package_checks(
    package: &DeliveryPackage,
    metadata: &MediaMetadata,
    profile: &Profile,
) -> Vec<Finding> {
    validate_package(package, metadata, profile)
}
