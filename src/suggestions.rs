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
        "BLACK_FRAMES" => Some("Trim leading black frames or regenerate the export."),
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
