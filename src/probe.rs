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
    r_frame_rate: Option<String>,
    bit_rate: Option<String>,
    sample_rate: Option<String>,
    channels: Option<u32>,
}

#[derive(Deserialize)]
struct FfprobeFormat {
    duration: Option<String>,
    format_name: Option<String>,
    bit_rate: Option<String>,
    size: Option<String>,
}

impl TryFrom<FfprobeOutput> for MediaMetadata {
    type Error = String;

    fn try_from(raw: FfprobeOutput) -> Result<Self, Self::Error> {
        let video = raw
            .streams
            .iter()
            .find(|stream| stream.codec_type.as_deref() == Some("video"))
            .ok_or("No video stream found")?;

        let audio = raw
            .streams
            .iter()
            .find(|stream| stream.codec_type.as_deref() == Some("audio"));

        let has_audio = audio.is_some();

        let duration_secs = raw
            .format
            .duration
            .as_deref()
            .ok_or("No duration found")?
            .parse::<f64>()
            .map_err(|e| e.to_string())?;

        let video_bitrate_kbps = video
            .bit_rate
            .as_deref()
            .or(raw.format.bit_rate.as_deref())
            .and_then(parse_bitrate_kbps);

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
            frame_rate: video
                .r_frame_rate
                .as_deref()
                .and_then(parse_frame_rate),
            video_bitrate_kbps,
            audio_codec: audio.and_then(|stream| stream.codec_name.clone()),
            audio_sample_rate: audio
                .and_then(|stream| stream.sample_rate.as_deref())
                .and_then(|value| value.parse().ok()),
            audio_channels: audio.and_then(|stream| stream.channels),
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

pub fn parse_frame_rate(value: &str) -> Option<f64> {
    if let Some((numerator, denominator)) = value.split_once('/') {
        let numerator: f64 = numerator.parse().ok()?;
        let denominator: f64 = denominator.parse().ok()?;
        if denominator > 0.0 {
            Some(numerator / denominator)
        } else {
            None
        }
    } else {
        value.parse().ok()
    }
}

fn parse_bitrate_kbps(value: &str) -> Option<u64> {
    value
        .parse::<u64>()
        .ok()
        .map(|bits_per_sec| bits_per_sec.div_ceil(1000))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_frame_rate_handles_fractions() {
        let fps = parse_frame_rate("30000/1001").expect("fps");
        assert!((fps - 29.97).abs() < 0.01);
    }

    #[test]
    fn parse_bitrate_kbps_converts_bits_to_kilobits() {
        assert_eq!(parse_bitrate_kbps("5000000"), Some(5000));
    }
}
