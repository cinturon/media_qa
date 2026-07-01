#[derive(Debug, Clone)]
pub struct MediaMetadata {
    pub duration_secs: f64,
    pub width: u32,
    pub height: u32,
    pub has_audio: bool,
    pub format_name: String,
    pub video_codec: Option<String>,
}
