use crate::providers::models::ProviderKind;
use std::path::PathBuf;

pub const SUBTITLE_FILE_TTL_SECS: u64 = 30 * 24 * 60 * 60; // 30 days
pub const SEARCH_CACHE_TTL_SECS: u64 = 7 * 24 * 60 * 60;   // 7 days

pub fn subtitle_root() -> PathBuf {
    let mut path = dirs::cache_dir().unwrap_or_else(std::env::temp_dir);
    path.push("moviebox-tui");
    path.push("subtitles");
    path
}

pub fn hash_key(parts: &[&str]) -> String {
    use md5::{Digest, Md5};
    let mut hasher = Md5::new();
    hasher.update(parts.join("|").as_bytes());
    let digest = hasher.finalize();
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn subtitle_path(
    provider: ProviderKind,
    subject_id: &str,
    season: usize,
    episode: usize,
    file_id: u32,
    lang: &str,
    ext: &str,
) -> PathBuf {
    let key = hash_key(&[
        provider.cache_key(),
        subject_id,
        &season.to_string(),
        &episode.to_string(),
        &file_id.to_string(),
        lang,
        env!("CARGO_PKG_VERSION"),
    ]);
    subtitle_root().join(format!("{key}.{ext}"))
}

/// Best-known extension for a downloaded subtitle file, derived from the
/// original file name (e.g. `.vtt`). Falls back to `.srt` when unknown.
pub fn subtitle_extension(file_name: Option<&str>) -> &'static str {
    let Some(name) = file_name else {
        return "srt";
    };
    let Some(ext) = name.rsplit('.').next() else {
        return "srt";
    };
    match ext.to_ascii_lowercase().as_str() {
        "srt" => "srt",
        "vtt" => "vtt",
        "ass" => "ass",
        "ssa" => "ssa",
        "sub" => "sub",
        _ => "srt",
    }
}

pub fn get_cached_subtitle_path(path: &std::path::Path) -> Option<PathBuf> {
    let mut candidate = path.to_path_buf();
    if !candidate.exists()
        && let (Some(stem), Some(parent)) = (path.file_stem(), path.parent())
    {
        // The file may have been cached with its original extension (e.g. `.vtt`).
        // The cache key is extension-independent, so fall back to any sibling
        // file that shares the same hash stem.
        if let Ok(entries) = std::fs::read_dir(parent) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_file() && p.file_stem().is_some_and(|s| s == stem) {
                    candidate = p;
                    break;
                }
            }
        }
    }
    if !candidate.exists() {
        return None;
    }
    if let Ok(metadata) = std::fs::metadata(&candidate) {
        if let Ok(modified) = metadata.modified() {
            if let Ok(elapsed) = modified.elapsed() {
                if elapsed.as_secs() <= SUBTITLE_FILE_TTL_SECS {
                    return Some(candidate);
                }
            }
        }
    }
    let _ = std::fs::remove_file(&candidate);
    None
}

pub fn set_subtitle_cache(path: &PathBuf, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, bytes)
}

pub fn search_cache_path(query_key: &str) -> PathBuf {
    subtitle_root().join("search").join(format!("{query_key}.json"))
}

pub fn search_query_key(ctx: &super::SubtitleContext, languages: &str) -> String {
    hash_key(&[
        ctx.imdb_id.as_deref().unwrap_or(""),
        &ctx.title,
        ctx.year.as_deref().unwrap_or(""),
        &ctx.season.unwrap_or(0).to_string(),
        &ctx.episode.unwrap_or(0).to_string(),
        languages,
    ])
}

pub fn get_search_cache(key: &str) -> Option<serde_json::Value> {
    let path = search_cache_path(key);
    if !path.exists() {
        return None;
    }
    if let Ok(metadata) = std::fs::metadata(&path) {
        if let Ok(modified) = metadata.modified() {
            if let Ok(elapsed) = modified.elapsed() {
                if elapsed.as_secs() > SEARCH_CACHE_TTL_SECS {
                    let _ = std::fs::remove_file(&path);
                    return None;
                }
            }
        }
    }
    let content = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str(&content).ok()
}

pub fn set_search_cache(key: &str, value: &serde_json::Value) {
    let path = search_cache_path(key);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(content) = serde_json::to_string(value) {
        let _ = std::fs::write(path, content);
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct QuotaInfo {
    pub requests: u32,
    pub remaining: u32,
    pub updated_at: u64,
}

pub fn quota_cache_path() -> PathBuf {
    subtitle_root().join("quota.json")
}

pub fn get_quota_cache() -> Option<QuotaInfo> {
    let path = quota_cache_path();
    if !path.exists() {
        return None;
    }
    let content = std::fs::read_to_string(&path).ok()?;
    let quota: QuotaInfo = serde_json::from_str(&content).ok()?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    if now.saturating_sub(quota.updated_at) > 24 * 60 * 60 {
        let _ = std::fs::remove_file(&path);
        return None;
    }
    Some(quota)
}

pub fn set_quota_cache(quota: &QuotaInfo) {
    let path = quota_cache_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(content) = serde_json::to_string(quota) {
        let _ = std::fs::write(path, content);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_key() {
        let h1 = hash_key(&["a", "b"]);
        let h2 = hash_key(&["a", "b"]);
        let h3 = hash_key(&["a", "c"]);
        assert_eq!(h1, h2);
        assert_ne!(h1, h3);
    }

    #[test]
    fn test_subtitle_cache_roundtrip() {
        let temp_dir = std::env::temp_dir().join("mb_test_sub_dir");
        let file_path = temp_dir.join("test.srt");
        assert!(set_subtitle_cache(&file_path, b"1\n00:00:01 -> 00:00:02\nTest\n").is_ok());
        assert!(get_cached_subtitle_path(&file_path).is_some());
        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_subtitle_path_shape() {
        let path = subtitle_path(ProviderKind::MovieBox, "subj", 1, 2, 123, "id", "srt");
        let s = path.to_string_lossy().to_string();
        assert!(s.contains("moviebox-tui"), "path: {s}");
        assert!(s.contains("subtitles"), "path: {s}");
        assert!(s.ends_with(".srt"), "path: {s}");
    }

    #[test]
    fn test_subtitle_path_differs_by_file_id() {
        let a = subtitle_path(ProviderKind::MovieBox, "subj", 1, 2, 100, "id", "srt");
        let b = subtitle_path(ProviderKind::MovieBox, "subj", 1, 2, 200, "id", "srt");
        assert_ne!(a, b, "different file_id must produce different cache keys");
    }

    #[test]
    fn test_subtitle_path_differs_by_lang() {
        let a = subtitle_path(ProviderKind::MovieBox, "subj", 1, 2, 100, "id", "srt");
        let b = subtitle_path(ProviderKind::MovieBox, "subj", 1, 2, 100, "en", "srt");
        assert_ne!(a, b, "different languages must produce different cache keys");
    }

    #[test]
    fn test_search_query_key_languages() {
        use crate::providers::subtitles::SubtitleContext;
        let ctx = SubtitleContext {
            imdb_id: Some("tt0848228".into()),
            title: "Avengers".into(),
            year: Some("2012".into()),
            season: None,
            episode: None,
            ..Default::default()
        };
        let key_id_en = search_query_key(&ctx, "id,en");
        let key_fr = search_query_key(&ctx, "fr");
        assert_ne!(key_id_en, key_fr, "languages must be part of the search cache key");
        assert_eq!(key_id_en, search_query_key(&ctx, "id,en"), "key must be stable");
    }

    #[test]
    fn test_subtitle_extension() {
        assert_eq!(subtitle_extension(Some("sub.srt")), "srt");
        assert_eq!(subtitle_extension(Some("sub.vtt")), "vtt");
        assert_eq!(subtitle_extension(Some("sub.SRT")), "srt");
        assert_eq!(subtitle_extension(Some("sub.ass")), "ass");
        assert_eq!(subtitle_extension(Some("sub.unknown")), "srt");
        assert_eq!(subtitle_extension(Some("noext")), "srt");
        assert_eq!(subtitle_extension(None), "srt");
    }

    #[test]
    fn test_quota_cache_roundtrip() {
        // Disk roundtrip via set/get_quota_cache is intentionally not tested:
        // those write to the real user cache dir (subtitle_root() is not
        // injectable). Instead verify the struct survives a full
        // serialize/deserialize cycle.
        let quota = QuotaInfo {
            requests: 5,
            remaining: 15,
            updated_at: 1_700_000_000,
        };
        let serialized = serde_json::to_string(&quota).unwrap();
        let back: QuotaInfo = serde_json::from_str(&serialized).unwrap();
        assert_eq!(quota, back);
    }

    #[test]
    fn test_expired_subtitle_file_removed() {
        use std::io::Write;
        let dir = std::env::temp_dir().join(format!(
            "mb_test_expired_{}_{}",
            std::process::id(),
            line!()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("expired.srt");

        // Write via an open handle so we can set an old mtime on it (needs
        // write access on Windows) and drop the handle before removal.
        let old =
            std::time::SystemTime::now() - std::time::Duration::from_secs(31 * 24 * 60 * 60);
        {
            let mut f = std::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(&path)
                .unwrap();
            f.write_all(b"1\n00:00:01,000 --> 00:00:02,000\nTest\n").unwrap();
            f.set_times(std::fs::FileTimes::new().set_modified(old))
                .unwrap();
        }

        assert_eq!(
            get_cached_subtitle_path(&path),
            None,
            "expired file must be ignored"
        );
        assert!(!path.exists(), "expired file should have been removed");

        let _ = std::fs::remove_dir_all(dir);
    }
}
