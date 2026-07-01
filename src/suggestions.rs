use crate::validate::Finding;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FixSuggestion {
    pub code: String,
    pub suggestion: String,
}

pub fn suggestions_for_findings(findings: &[Finding]) -> Vec<FixSuggestion> {
    findings
        .iter()
        .filter_map(|finding| suggestion_for_code(&finding.code).map(|text| FixSuggestion {
            code: finding.code.clone(),
            suggestion: text.to_string(),
        }))
        .collect()
}

fn suggestion_for_code(code: &str) -> Option<&'static str> {
    match code {
        "RESOLUTION_MISMATCH" => Some("Re-export at the profile resolution before delivery."),
        "AUDIO_REQUIRED" => Some("Add an audio track or disable require_audio for this profile."),
        "DURATION_TOO_SHORT" => Some("Extend the clip or lower min_duration_secs in the profile."),
        "EXTENSION_MISMATCH" => Some("Re-wrap or re-export to the required container extension."),
        "BLACK_FRAMES" => Some("Trim black sections at the listed times, then re-export."),
        "FROZEN_FRAMES" => Some("Re-export or cut around the frozen picture segments."),
        "LOUDNESS_TOO_HIGH" => Some("Lower integrated loudness before delivery."),
        "TRUE_PEAK_TOO_HIGH" => Some("Reduce true peak level with a limiter, then re-export."),
        "DURATION_TOO_LONG" => Some("Shorten the clip or choose a profile with a higher limit."),
        "VIDEO_CODEC_MISMATCH" => Some("Re-encode to the profile's required video codec."),
        "FRAME_RATE_MISMATCH" => Some("Re-export at the profile frame rate."),
        "VIDEO_BITRATE_TOO_LOW" => Some("Increase export bitrate to avoid blocky delivery."),
        "VIDEO_BITRATE_TOO_HIGH" => Some("Lower export bitrate to meet delivery limits."),
        "AUDIO_CODEC_MISMATCH" => Some("Re-encode audio to the profile's required codec."),
        "AUDIO_SAMPLE_RATE_MISMATCH" => Some("Export audio at the required sample rate."),
        "AUDIO_CHANNELS_MISMATCH" => Some("Mix or export the required channel layout."),
        "CAPTIONS_EMPTY" => Some("Add cues to the caption sidecar or regenerate captions."),
        "CAPTIONS_SHORT" => Some("Extend captions to cover the full program length."),
        "CAPTIONS_MALFORMED" => Some("Fix caption timing syntax in the sidecar file."),
        "FILE_EMPTY" => Some("Replace the zero-byte export with a valid media file."),
        "FILE_TOO_LARGE" => Some("Re-encode or split the export to reduce file size."),
        "DUPLICATE_BASENAME" => Some("Rename files so each basename is unique in the batch."),
        "BATCH_MIXED_RESOLUTION" => Some("Re-export the batch at a single resolution."),
        "BATCH_MIXED_CODEC" => Some("Standardize the batch on one video codec."),
        "SILENCE_DETECTED" => Some("Remove dead air at the head/tail or adjust audio levels."),
        "AUDIO_TOO_QUIET" => Some("Normalize dialogue loudness before re-exporting."),
        "CAPTIONS_MISSING" => Some("Add an .srt or .vtt file with the same basename as the video."),
        "THUMBNAIL_MISSING" => Some("Add a .jpg or .png thumbnail with the same basename as the video."),
        "DECODE_FAILED" => Some("Re-encode the source or inspect the file for corruption."),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validate::Severity;

    #[test]
    fn maps_resolution_mismatch_to_suggestion() {
        let findings = vec![Finding {
            code: "RESOLUTION_MISMATCH".into(),
            severity: Severity::Fail,
            message: "bad".into(),
        }];
        let suggestions = suggestions_for_findings(&findings);
        assert_eq!(suggestions.len(), 1);
        assert!(suggestions[0].suggestion.contains("resolution"));
    }
}
