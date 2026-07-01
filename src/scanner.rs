use std::path::{Path, PathBuf};
use walkdir::WalkDir;

const MEDIA_EXTENSIONS: &[&str] = &["mp4", "mov", "mkv"];

pub fn scan(path: &Path) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    let entries: Vec<PathBuf> = WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_file())
        .filter(|e| is_media_candidate(e.path()))
        .map(|e| e.into_path())
        .collect();

    Ok(entries)
}

fn is_media_candidate(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| MEDIA_EXTENSIONS.contains(&ext))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_media_candidate_accepts_mp4() {
        assert!(is_media_candidate(Path::new("clip.mp4")));
    }

    #[test]
    fn is_media_candidate_rejects_txt() {
        assert!(!is_media_candidate(Path::new("notes.txt")));
    }
}
