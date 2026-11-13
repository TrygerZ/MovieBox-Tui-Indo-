pub fn detect_audio_language(text: &str) -> Option<String> {
    let lower = text.to_lowercase();
    if lower.contains("hindi") {
        Some("Hindi".to_string())
    } else if lower.contains("bengali") || lower.contains("bangla") {
        Some("Bengali".to_string())
    } else if lower.contains("tamil") {
        Some("Tamil".to_string())
    } else if lower.contains("telugu") {
        Some("Telugu".to_string())
    } else if lower.contains("dual") {
        Some("Dual Audio".to_string())
    } else if lower.contains("multi") {
        Some("Multi Audio".to_string())
    } else if lower.contains("english") {
        Some("English".to_string())
    } else {
        None
    }
}

pub fn detect_resolution(text: &str) -> Option<String> {
    let lower = text.to_lowercase();
    if lower.contains("2160p") || lower.contains("4k") {
        Some("4K".to_string())
    } else if lower.contains("1080p") {
        Some("1080p".to_string())
    } else if lower.contains("720p") {
        Some("720p".to_string())
    } else if lower.contains("480p") {
        Some("480p".to_string())
    } else {
        None
    }
}

pub fn detect_codec(text: &str) -> Option<String> {
    let lower = text.to_lowercase();
    if lower.contains("x265") || lower.contains("hevc") {
        Some("HEVC".to_string())
    } else if lower.contains("x264") || lower.contains("h264") {
        Some("x264".to_string())
    } else if lower.contains("av1") {
        Some("AV1".to_string())
    } else {
        None
    }
}
