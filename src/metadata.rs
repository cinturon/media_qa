#[derive(Debug, Clone)]
pub struct MediaMetadata {
    pub duration_secs: f64,
    pub width: u32,
    pub height: u32,
    pub has_audio: bool,
    pub format_name: String,
    pub video_codec: Option<String>,
    pub frame_rate: Option<f64>,
    pub video_bitrate_kbps: Option<u64>,
    pub audio_codec: Option<String>,
    pub audio_sample_rate: Option<u32>,
    pub audio_channels: Option<u32>,
}
