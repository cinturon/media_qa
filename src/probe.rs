use crate::metadata::MediaMetadata;
use serde::Deserialize;
use serde_json::from_slice;
use std::convert::TryFrom;
use std::path::Path;
use std::process::Command;

#[derive(Deserialize)]
struct FfprobeOutput {
    streams: Vec<FfprobeStream>,
    format: FfprobeFormat,
}

#[derive(Deserialize)]
struct FfprobeStream {
    codec_type: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    codec_name: Option<String>,
}

#[derive(Deserialize)]
struct FfprobeFormat {
    duration: Option<String>,
    format_name: Option<String>,
}

impl TryFrom<FfprobeOutput> for MediaMetadata {
    type Error = String;

    fn try_from(raw: FfprobeOutput) -> Result<Self, Self::Error> {
        let video = raw
            .streams
            .iter()
            .find(|stream| stream.codec_type.as_deref() == Some("video"))
            .ok_or("No video stream found")?;

        let has_audio = raw
            .streams
            .iter()
            .any(|stream| stream.codec_type.as_deref() == Some("audio"));

        let duration_secs = raw
            .format
            .duration
            .as_deref()
            .ok_or("No duration found")?
            .parse::<f64>()
            .map_err(|e| e.to_string())?;

        Ok(MediaMetadata {
            duration_secs,
            width: video.width.ok_or("No width found")?,
            height: video.height.ok_or("No height found")?,
            has_audio,
            format_name: raw
                .format
                .format_name
                .unwrap_or_else(|| "unknown".into()),
            video_codec: video.codec_name.clone(),
        })
    }
}

pub fn probe_media(path: &Path) -> Result<MediaMetadata, String> {
    let output = Command::new("ffprobe")
        .arg("-v")
        .arg("quiet")
        .arg("-print_format")
        .arg("json")
        .arg("-show_format")
        .arg("-show_streams")
        .arg(path)
        .output()
        .map_err(|e| format!("failed to run ffprobe: {e}"))?;

    if !output.status.success() {
        return Err(format!(
            "ffprobe failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let raw_output: FfprobeOutput =
        from_slice(&output.stdout).map_err(|e| format!("failed to parse ffprobe JSON: {e}"))?;

    MediaMetadata::try_from(raw_output)
}
