use crate::providers::models::ProviderKind;
use std::fs;
use std::path::PathBuf;

const CACHE_EXPIRY_SECS: u64 = 24 * 60 * 60;
const STREAM_CACHE_EXPIRY_SECS: u64 = 2 * 60 * 60;
const HOMEPAGE_CACHE_EXPIRY_SECS: u64 = 60 * 60;

fn read_json_cache(path: &PathBuf, expiry_secs: u64) -> Option<serde_json::Value> {
    if path.exists() {
        if let Ok(metadata) = fs::metadata(path) {
            match metadata.modified().ok().and_then(|m| m.elapsed().ok()) {
                Some(elapsed) if elapsed.as_secs() > expiry_secs => {
                    let _ = fs::remove_file(path);
                    return None;
                }
                None => {
                    let _ = fs::remove_file(path);
                    return None;
                }
                _ => {}
            }
        }
        if let Ok(content) = fs::read_to_string(path) {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                if !val.is_null() {
                    return Some(val);
                }
            }
        }
        let _ = fs::remove_file(path);
    }
    None
}

pub fn md5_hex(value: &str) -> String {
    use md5::{Digest, Md5};
    let mut hasher = Md5::new();
    hasher.update(value.as_bytes());
    let result = hasher.finalize();
    let mut safe_query = String::with_capacity(32);
    for b in result {
        use std::fmt::Write;
        let _ = write!(&mut safe_query, "{:02x}", b);
    }
    safe_query
}

pub fn atomic_write_file(path: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temporary = path.with_extension(format!("tmp-{}-{stamp}", std::process::id()));
    std::fs::write(&temporary, bytes)?;
    if std::fs::rename(&temporary, path).is_err() {
        let _ = std::fs::remove_file(path);
        if let Err(error) = std::fs::rename(&temporary, path) {
            let _ = std::fs::remove_file(&temporary);
            return Err(error);
        }
    }
    Ok(())
}

pub async fn atomic_write_file_async(path: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await.ok();
    }
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temporary = path.with_extension(format!("tmp-{}-{stamp}", std::process::id()));
    tokio::fs::write(&temporary, bytes).await?;
    if tokio::fs::rename(&temporary, path).await.is_err() {
        let _ = tokio::fs::remove_file(path).await;
        if let Err(error) = tokio::fs::rename(&temporary, path).await {
            let _ = tokio::fs::remove_file(&temporary).await;
            return Err(error);
        }
    }
    Ok(())
}

fn write_json_cache(path: &PathBuf, data: &serde_json::Value) {
    if data.is_null() {
        return;
    }
    let Ok(content) = serde_json::to_vec(data) else {
        log::warn!(
            "failed to serialize cache for {}",
            crate::logging::sanitize_path(path)
        );
        return;
    };
    if let Err(error) = atomic_write_file(path, &content) {
        log::warn!(
            "failed to commit cache to {}: {error}",
            crate::logging::sanitize_path(path)
        );
    }
}

fn stream_payload_has_results(data: &serde_json::Value) -> bool {
    data.as_array().is_some_and(|streams| {
        !streams.is_empty()
            && streams.iter().any(|stream| {
                stream
                    .get("resourceLink")
                    .and_then(|link| link.as_str())
                    .is_some_and(|link| !link.is_empty())
            })
    })
}

fn hash_key(value: &str) -> String {
    md5_hex(value)
}

pub fn get_provider_cache_dir(provider: ProviderKind, subdir: &str) -> PathBuf {
    crate::config::cache_dir()
        .join(provider.cache_key())
        .join(subdir)
}

pub fn get_provider_stream_path(
    provider: ProviderKind,
    subject_id: &str,
    season: usize,
    episode: usize,
) -> PathBuf {
    let mut path = get_provider_cache_dir(provider, "streams");
    let schema = if provider == ProviderKind::FourKHdHub {
        "v3_"
    } else {
        ""
    };
    let hashed_id = hash_key(subject_id);
    path.push(format!("{schema}{hashed_id}_{season}_{episode}.json"));
    path
}

pub fn get_provider_stream_cache(
    provider: ProviderKind,
    subject_id: &str,
    season: usize,
    episode: usize,
) -> Option<serde_json::Value> {
    let path = get_provider_stream_path(provider, subject_id, season, episode);
    let value = read_json_cache(&path, STREAM_CACHE_EXPIRY_SECS)?;
    if stream_payload_has_results(&value) {
        Some(value)
    } else {
        let _ = fs::remove_file(path);
        None
    }
}

pub fn get_provider_details_path(provider: ProviderKind, subject_id: &str) -> PathBuf {
    let mut path = get_provider_cache_dir(provider, "details");
    let schema = if provider == ProviderKind::FourKHdHub {
        "v2_"
    } else {
        ""
    };
    let hashed_id = hash_key(subject_id);
    path.push(format!("details_{schema}{hashed_id}.json"));
    path
}

pub fn get_provider_details_cache(
    provider: ProviderKind,
    subject_id: &str,
) -> Option<serde_json::Value> {
    read_json_cache(
        &get_provider_details_path(provider, subject_id),
        CACHE_EXPIRY_SECS,
    )
}

pub fn invalidate_provider_details_cache(provider: ProviderKind, subject_id: &str) {
    let path = get_provider_details_path(provider, subject_id);
    let _ = fs::remove_file(path);
}

pub fn get_provider_search_path(provider: ProviderKind, query: &str, page: usize) -> PathBuf {
    let mut path = get_provider_cache_dir(provider, "search");
    let hashed = hash_key(query);
    path.push(format!("{hashed}_{page}.json"));
    path
}

pub fn get_provider_search_cache(
    provider: ProviderKind,
    query: &str,
    page: usize,
) -> Option<serde_json::Value> {
    let path = get_provider_search_path(provider, query, page);
    let value = read_json_cache(&path, CACHE_EXPIRY_SECS)?;
    if search_payload_has_results(&value) {
        Some(value)
    } else {
        let _ = fs::remove_file(path);
        None
    }
}

pub fn set_provider_search_cache(
    provider: ProviderKind,
    query: &str,
    page: usize,
    data: &serde_json::Value,
) {
    let path = get_provider_search_path(provider, query, page);
    if !search_payload_has_results(data) {
        let _ = fs::remove_file(path);
        return;
    }
    write_json_cache(&path, data);
}

fn search_payload_has_results(data: &serde_json::Value) -> bool {
    data.get("results")
        .and_then(|results| results.as_array())
        .and_then(|results| results.first())
        .and_then(|result| result.get("subjects"))
        .and_then(|subjects| subjects.as_array())
        .is_some_and(|subjects| !subjects.is_empty())
}

pub fn set_provider_details_cache(
    provider: ProviderKind,
    subject_id: &str,
    data: &serde_json::Value,
) {
    let path = get_provider_details_path(provider, subject_id);
    write_json_cache(&path, data);
}

pub fn set_provider_stream_cache(
    provider: ProviderKind,
    subject_id: &str,
    season: usize,
    episode: usize,
    data: &serde_json::Value,
) {
    let path = get_provider_stream_path(provider, subject_id, season, episode);
    if stream_payload_has_results(data) {
        write_json_cache(&path, data);
    }
}

pub fn invalidate_provider_stream_cache(
    provider: ProviderKind,
    subject_id: &str,
    season: usize,
    episode: usize,
) {
    let path = get_provider_stream_path(provider, subject_id, season, episode);
    if path.exists() {
        let _ = fs::remove_file(&path);
    }
}

pub fn clear_all_cache() {
    let path = crate::config::cache_dir();
    if path.exists() {
        let _ = fs::remove_dir_all(&path);
    }
    if let Some(data_dir) = crate::config::data_dir() {
        let legacy = data_dir.join("iptv_cache");
        if legacy.exists() {
            let _ = fs::remove_dir_all(&legacy);
        }
    }
}

pub fn clean_old_cache_background() {
    tokio::task::spawn_blocking(|| {
        let path = crate::config::cache_dir();
        if !path.exists() {
            return;
        }
        clean_dir_old_files(&path, 7 * 24 * 60 * 60, &["subtitles"]);
    });
}

/// Recursively remove files older than `max_age_secs` under `root`, skipping
/// whole subdirectories named in `skip_dir_names`. The `subtitles/` dir is
/// skipped because its files intentionally have their own 30-day TTL (see
/// `src/providers/subtitles/cache.rs`); a 7-day sweep would force
/// re-downloads and burn the subtitle provider quota.
fn clean_dir_old_files(root: &std::path::Path, max_age_secs: u64, skip_dir_names: &[&str]) {
    let mut dirs_to_check = vec![root.to_path_buf()];
    while let Some(dir) = dirs_to_check.pop() {
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                if let Ok(metadata) = entry.metadata() {
                    if metadata.is_dir() {
                        if entry
                            .file_name()
                            .to_str()
                            .is_some_and(|n| skip_dir_names.contains(&n))
                        {
                            continue;
                        }
                        dirs_to_check.push(entry.path());
                    } else if metadata.is_file() {
                        if let Ok(modified) = metadata.modified() {
                            if let Ok(elapsed) = modified.elapsed() {
                                if elapsed.as_secs() > max_age_secs {
                                    let _ = fs::remove_file(entry.path());
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn get_homepage_path(tab_id: &str, page: usize) -> PathBuf {
    let mut path = get_provider_cache_dir(ProviderKind::MovieBox, "homepage");
    path.push(format!("{}_{}.json", tab_id, page));
    path
}

pub fn get_homepage_cache(tab_id: &str, page: usize) -> Option<serde_json::Value> {
    read_json_cache(&get_homepage_path(tab_id, page), HOMEPAGE_CACHE_EXPIRY_SECS)
}

pub fn set_homepage_cache(tab_id: &str, page: usize, data: &serde_json::Value) {
    let path = get_homepage_path(tab_id, page);
    write_json_cache(&path, data);
}

pub fn get_addon_catalog_path(manifest_url: &str, r_type: &str, catalog_id: &str) -> PathBuf {
    let mut path = get_provider_cache_dir(ProviderKind::Addons, "catalogs");
    let hashed = hash_key(&format!("{manifest_url}_{r_type}_{catalog_id}"));
    path.push(format!("catalog_{hashed}.json"));
    path
}

pub fn get_addon_catalog_cache(
    manifest_url: &str,
    r_type: &str,
    catalog_id: &str,
) -> Option<serde_json::Value> {
    read_json_cache(
        &get_addon_catalog_path(manifest_url, r_type, catalog_id),
        HOMEPAGE_CACHE_EXPIRY_SECS,
    )
}

pub fn set_addon_catalog_cache(
    manifest_url: &str,
    r_type: &str,
    catalog_id: &str,
    data: &serde_json::Value,
) {
    let path = get_addon_catalog_path(manifest_url, r_type, catalog_id);
    write_json_cache(&path, data);
}

pub fn get_addon_manifest_path(manifest_url: &str) -> PathBuf {
    let mut path = get_provider_cache_dir(ProviderKind::Addons, "manifests");
    let hashed = hash_key(manifest_url);
    path.push(format!("manifest_{hashed}.json"));
    path
}

pub fn get_addon_manifest_cache(manifest_url: &str) -> Option<serde_json::Value> {
    read_json_cache(&get_addon_manifest_path(manifest_url), CACHE_EXPIRY_SECS)
}

pub fn set_addon_manifest_cache(manifest_url: &str, data: &serde_json::Value) {
    let path = get_addon_manifest_path(manifest_url);
    write_json_cache(&path, data);
}

fn get_namespaced_image_path(namespace: &str, id: &str) -> PathBuf {
    let mut path = crate::config::cache_dir();
    path.push(namespace);
    path.push("images");
    let safe_name = hash_key(id);
    path.push(format!("{safe_name}.img"));
    path
}

const IMAGE_CACHE_EXPIRY_SECS: u64 = 30 * 24 * 60 * 60;

fn check_image_file(path: &PathBuf) -> Option<Vec<u8>> {
    if path.exists() {
        if let Ok(metadata) = std::fs::metadata(path) {
            match metadata.modified().ok().and_then(|m| m.elapsed().ok()) {
                Some(elapsed) if elapsed.as_secs() > IMAGE_CACHE_EXPIRY_SECS => {
                    let _ = std::fs::remove_file(path);
                    return None;
                }
                None => {
                    let _ = std::fs::remove_file(path);
                    return None;
                }
                _ => {}
            }
        }
        return std::fs::read(path).ok();
    }
    None
}

pub fn get_namespaced_image_cache(namespace: &str, id: &str) -> Option<Vec<u8>> {
    let path = get_namespaced_image_path(namespace, id);
    if let Some(bytes) = check_image_file(&path) {
        return Some(bytes);
    }
    let fallback_namespaces = [
        "posters",
        "moviebox",
        "fourkhdhub",
        "iptv",
        "circleftp",
        "dhakaflix",
        "addons",
    ];
    for fb in fallback_namespaces {
        if fb != namespace {
            let fb_path = get_namespaced_image_path(fb, id);
            if let Some(bytes) = check_image_file(&fb_path) {
                return Some(bytes);
            }
        }
    }
    None
}

pub fn set_namespaced_image_cache(namespace: &str, id: &str, bytes: &[u8]) {
    let path = get_namespaced_image_path(namespace, id);
    if let Err(error) = atomic_write_file(&path, bytes) {
        log::warn!(
            "failed to commit image cache to {}: {error}",
            crate::logging::sanitize_path(&path)
        );
    }
}

pub fn get_captions_path(subject_id: &str, resource_id: &str) -> PathBuf {
    let mut path = get_provider_cache_dir(ProviderKind::MovieBox, "captions");
    let hashed_id = hash_key(&format!("{}_{}", subject_id, resource_id));
    path.push(format!("captions_{}.json", hashed_id));
    path
}

pub fn get_captions_cache(subject_id: &str, resource_id: &str) -> Option<serde_json::Value> {
    read_json_cache(
        &get_captions_path(subject_id, resource_id),
        CACHE_EXPIRY_SECS,
    )
}

pub fn set_captions_cache(subject_id: &str, resource_id: &str, data: &serde_json::Value) {
    write_json_cache(&get_captions_path(subject_id, resource_id), data);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Write `name` under `dir` with an mtime older than 7 days.
    fn write_old_file(dir: &std::path::Path, name: &str) -> std::path::PathBuf {
        use std::io::Write;
        let path = dir.join(name);
        let old = std::time::SystemTime::now() - std::time::Duration::from_secs(10 * 24 * 60 * 60);
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&path)
            .unwrap();
        f.write_all(b"{}").unwrap();
        f.set_times(std::fs::FileTimes::new().set_modified(old))
            .unwrap();
        path
    }

    #[test]
    fn test_clean_dir_old_files_skips_subtitles() {
        let dir = std::env::temp_dir().join(format!(
            "mb_test_cleaner_{}_{}",
            std::process::id(),
            line!()
        ));
        let sub = dir.join("subtitles");
        std::fs::create_dir_all(&sub).unwrap();

        // >7d old; the 30-day subtitle TTL must protect it from the 7-day sweep.
        let sub_file = write_old_file(&sub, "abc.srt");
        // >7d old outside subtitles/; must still be swept.
        let root_file = write_old_file(&dir, "old.json");

        clean_dir_old_files(&dir, 7 * 24 * 60 * 60, &["subtitles"]);

        assert!(
            sub_file.exists(),
            "subtitles/ files must survive the 7-day cleaner"
        );
        assert!(!root_file.exists(), "old non-subtitle file must be swept");
        let _ = std::fs::remove_dir_all(dir);
    }
}
