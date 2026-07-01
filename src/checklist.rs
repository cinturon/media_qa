use crate::config::Profile;

pub fn render_checklist(profile_name: &str, profile: &Profile) -> String {
    let mut lines = vec![
        format!("# Delivery checklist: {profile_name}"),
        String::new(),
        "## Required video".into(),
        format!("- Container extension: `.{}`", profile.extension),
        format!("- Resolution: {}x{}", profile.width, profile.height),
        format!(
            "- Minimum duration: {:.1} seconds",
            profile.min_duration_secs
        ),
        String::new(),
        "## Audio".into(),
    ];

    if profile.require_audio {
        lines.push("- Audio track required".into());
    } else {
        lines.push("- Audio track optional".into());
    }
    lines.push(format!(
        "- Maximum leading/trailing silence: {:.1} seconds",
        profile.max_silence_secs
    ));
    lines.push(format!(
        "- Minimum mean volume: {:.1} dB",
        profile.min_mean_volume_db
    ));

    lines.push(String::new());
    lines.push("## Package".into());
    if profile.require_captions {
        lines.push("- Captions sidecar required (.srt or .vtt)".into());
    }
    if profile.require_thumbnail {
        lines.push("- Thumbnail sidecar required (.jpg or .png)".into());
    }
    if !profile.require_captions && !profile.require_thumbnail {
        lines.push("- No sidecar files required".into());
    }

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checklist_includes_resolution() {
        let profile = Profile::default();
        let text = render_checklist("youtube", &profile);
        assert!(text.contains("1920x1080"));
    }
}
