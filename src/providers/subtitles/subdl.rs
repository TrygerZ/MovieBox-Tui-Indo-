//! SubDL (subdl.com) subtitle provider.
//!
//! Implements the v2 search API (`https://api.subdl.com/api/v2`, auth via
//! `Authorization: Bearer <key>`) and the anonymous download host
//! (`https://dl.subdl.com`, no key needed, separate 300/day/IP quota).
//! See `RESEARCH-SUBDL-API.md` for the verified API specification.
//!
//! Search results are cached through the shared subtitle cache
//! ([`super::cache`]) with a `subdl:` key prefix so they never collide with
//! the OpenSubtitles entries.

use super::SubtitleContext;
use super::cache;
use serde::Deserialize;
use std::io::Read;

/// v2 API base URL.
pub const BASE_URL: &str = "https://api.subdl.com/api/v2";

/// Anonymous download host. No API key is required here and the separate
/// quota (300 downloads/day/IP) never consumes the small per-account
/// download quota (50/day free).
pub const DOWNLOAD_BASE_URL: &str = "https://dl.subdl.com";

/// Browser-like User-Agent so SubDL/Cloudflare doesn't reject the plain
/// reqwest client (mirrors the OpenSubtitles client).
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0 Safari/537.36 MovieBox-Tui/1.0";

/// Subtitle extensions accepted from a SubDL archive. Other files (readme,
/// nfo, samples) are skipped when picking the first usable subtitle.
const ACCEPTED_SUBTITLE_EXTS: [&str; 5] = ["srt", "vtt", "ass", "ssa", "sub"];

/// Maximum number of candidates kept per search (mirrors OpenSubtitles).
const MAX_CANDIDATES: usize = 5;

#[derive(Debug, Clone, Default)]
pub struct SubdlConfig {
    pub api_key: Option<String>,
    pub languages: Vec<String>,
    pub enabled: bool,
    pub base_url: Option<String>,
}

impl SubdlConfig {
    pub fn from_env() -> Self {
        let enabled = std::env::var("MOVIEBOX_SUBDL_ENABLED")
            .map(|v| !v.eq_ignore_ascii_case("false") && v != "0")
            .unwrap_or(true);
        let languages = std::env::var("MOVIEBOX_SUBDL_LANGUAGES")
            .map(|v| {
                v.split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect::<Vec<_>>()
            })
            .ok()
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| vec!["id".to_string(), "en".to_string()]);
        Self {
            api_key: std::env::var("MOVIEBOX_SUBDL_API_KEY").ok(),
            languages,
            enabled,
            base_url: std::env::var("MOVIEBOX_SUBDL_BASE_URL").ok(),
        }
    }

    /// Enabled only when the flag is on AND an API key is configured. Search
    /// requires a (free) key; anonymous downloads work without one.
    pub fn enabled(&self) -> bool {
        self.enabled && self.api_key.as_deref().is_some_and(|s| !s.is_empty())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SubdlError {
    #[error("missing SubDL credential: {0}")]
    MissingCredentials(&'static str),
    #[error("SubDL API error: HTTP {0}: {1}")]
    Http(u16, String),
    #[error("subtitle not found")]
    NotFound,
    #[error("subtitle archive contains no subtitle file")]
    NoSubtitleInZip,
    #[error("ZIP error: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("reqwest error: {0}")]
    Reqwest(#[from] reqwest::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Clone)]
pub struct SubdlClient {
    http: reqwest::Client,
    config: SubdlConfig,
}

impl SubdlClient {
    pub fn new(config: SubdlConfig) -> Self {
        let http = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(std::time::Duration::from_secs(12))
            .connect_timeout(std::time::Duration::from_secs(5))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Self { http, config }
    }

    pub fn from_env() -> Self {
        Self::new(SubdlConfig::from_env())
    }

    pub fn enabled(&self) -> bool {
        self.config.enabled()
    }

    fn base_url(&self) -> &str {
        self.config.base_url.as_deref().unwrap_or(BASE_URL)
    }

    /// Read the response body as a short single-line snippet for error
    /// messages, capped at 300 chars so a huge Cloudflare page can't bloat
    /// the UI.
    async fn body_snippet(res: reqwest::Response) -> String {
        let body = res.text().await.unwrap_or_default();
        let snippet: String = body.split_whitespace().collect::<Vec<_>>().join(" ");
        let snippet: String = snippet.chars().take(300).collect();
        if snippet.is_empty() {
            "(no response body)".to_string()
        } else {
            snippet
        }
    }

    /// v2 subtitle search (`GET /api/v2/subtitles/search`). The response is
    /// cached per (imdb_id, title, year, season, episode, languages) so the
    /// small free quota (2.000 search/day) is preserved across runs.
    pub async fn search(&self, ctx: &SubtitleContext) -> Result<SubdlSearchOutcome, SubdlError> {
        let query_key = format!(
            "subdl:{}",
            cache::search_query_key(ctx, &self.config.languages.join(","))
        );
        if let Some(cached_val) = cache::get_search_cache(&query_key) {
            if let Ok(search_res) = serde_json::from_value::<SubdlSearchResponse>(cached_val) {
                let candidates = self.parse_and_score_search(search_res, ctx);
                return Ok(SubdlSearchOutcome {
                    candidates,
                    from_cache: true,
                });
            }
        }

        let api_key = self
            .config
            .api_key
            .as_deref()
            .ok_or(SubdlError::MissingCredentials("api_key"))?;

        // Build the query string inside a block scope: the
        // `url::form_urlencoded::Serializer` is not `Send` (it holds a
        // non-`Sync` encoding override), so it must be consumed before the
        // first await below or the returned future can't be `Send` and the
        // caller can't use it inside `tokio::spawn`.
        let query_string = {
            let mut params = url::form_urlencoded::Serializer::new(String::new());
            params.append_pair("languages", &self.config.languages.join(","));
            if let Some(imdb_id) = &ctx.imdb_id {
                params.append_pair("imdb_id", imdb_id);
            } else {
                params.append_pair("film_name", &ctx.title);
                if let Some(year) = &ctx.year {
                    params.append_pair("year", year);
                }
            }
            if ctx.is_episode {
                params.append_pair("type", "tv");
                if let Some(season) = ctx.season {
                    params.append_pair("season", &season.to_string());
                }
                if let Some(episode) = ctx.episode {
                    params.append_pair("episode", &episode.to_string());
                }
            } else {
                params.append_pair("type", "movie");
            }
            params.append_pair("subs_per_page", "30");
            params.finish()
        };

        let url = format!("{}/subtitles/search?{}", self.base_url(), query_string);
        let res = self
            .http
            .get(&url)
            .header("Authorization", format!("Bearer {api_key}"))
            .send()
            .await?;
        if !res.status().is_success() {
            let status = res.status().as_u16();
            let message = Self::body_snippet(res).await;
            return Err(SubdlError::Http(status, message));
        }

        let raw_val: serde_json::Value = res.json().await?;
        cache::set_search_cache(&query_key, &raw_val);

        let search_res: SubdlSearchResponse = serde_json::from_value(raw_val)?;
        let candidates = self.parse_and_score_search(search_res, ctx);

        Ok(SubdlSearchOutcome {
            candidates,
            from_cache: false,
        })
    }

    fn parse_and_score_search(
        &self,
        search_res: SubdlSearchResponse,
        ctx: &SubtitleContext,
    ) -> Vec<SubdlCandidate> {
        let title_year = search_res.results.first().and_then(|r| r.year);
        let mut list = Vec::new();
        for item in search_res.subtitles {
            let Some(subtitle_id) = item.subtitle_id() else {
                continue;
            };
            let language = item.language.clone().unwrap_or_else(|| "id".to_string());
            let score = score_subdl_candidate(&item, ctx, title_year);
            list.push(SubdlCandidate {
                subtitle_id,
                language,
                score,
                release_name: item.release_name.clone(),
                rating: item.rating,
                download_count: item.download_count,
            });
        }
        list.sort_by_key(|c| std::cmp::Reverse(c.score));
        list.truncate(MAX_CANDIDATES);
        list
    }

    /// Download a subtitle archive from the anonymous host
    /// (`dl.subdl.com/subtitle/{n_id}.zip`) and return the first usable
    /// subtitle file as `(extension, bytes)`. No API key is sent, so the
    /// per-account download quota is never consumed.
    pub async fn download_bytes(&self, subtitle_id: &str) -> Result<(String, Vec<u8>), SubdlError> {
        let url = subdl_download_url(subtitle_id);
        let res = self.http.get(&url).send().await?;
        if res.status().as_u16() == 404 {
            return Err(SubdlError::NotFound);
        }
        let res = res.error_for_status()?;
        let bytes = res.bytes().await?.to_vec();
        extract_first_subtitle_from_zip(&bytes)
    }
}

/// Search response. Follows the v1 search shape documented in
/// `RESEARCH-SUBDL-API.md` §4.1 (`results` for the matched title, `subtitles`
/// for the actual subtitle entries); both lists are optional so a response
/// without a match deserializes cleanly.
#[derive(Debug, Clone, Deserialize)]
pub struct SubdlSearchResponse {
    #[serde(default)]
    pub status: Option<bool>,
    #[serde(default)]
    pub results: Vec<SubdlTitleResult>,
    #[serde(default)]
    pub subtitles: Vec<SubdlSubtitleItem>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SubdlTitleResult {
    #[serde(default)]
    pub imdb_id: Option<String>,
    #[serde(default)]
    pub tmdb_id: Option<i64>,
    #[serde(rename = "type", default)]
    pub item_type: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(rename = "sd_id", default)]
    pub sd_id: Option<i64>,
    #[serde(default)]
    pub year: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SubdlSubtitleItem {
    #[serde(rename = "release_name", default)]
    pub release_name: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    /// Relative download path, e.g. `/subtitle/3197651-3213944.zip`.
    #[serde(default)]
    pub url: Option<String>,
    /// 2-letter language code; the API returns it UPPERCASE (`ID`).
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub season: Option<u32>,
    #[serde(default)]
    pub episode: Option<u32>,
    #[serde(rename = "full_season", default)]
    pub full_season: Option<bool>,
    #[serde(default)]
    pub hi: Option<bool>,
    #[serde(default)]
    pub fps: Option<String>,
    #[serde(rename = "download_count", default)]
    pub download_count: Option<u32>,
    #[serde(default)]
    pub rating: Option<f32>,
    #[serde(default)]
    pub id: Option<String>,
    #[serde(rename = "n_id", default)]
    pub n_id: Option<String>,
}

impl SubdlSubtitleItem {
    /// The download identifier (`n_id`) for this subtitle. Prefers an explicit
    /// id field, then falls back to parsing it from the `url` download path.
    fn subtitle_id(&self) -> Option<String> {
        if let Some(id) = self.n_id.as_deref().or(self.id.as_deref()) {
            if !id.trim().is_empty() {
                return Some(id.trim().to_string());
            }
        }
        self.url.as_deref().and_then(subtitle_id_from_url)
    }
}

/// A single scored subtitle candidate, ready for the subtitle list UI.
#[derive(Debug, Clone, PartialEq)]
pub struct SubdlCandidate {
    pub subtitle_id: String,
    pub language: String,
    pub score: i32,
    pub release_name: Option<String>,
    pub rating: Option<f32>,
    pub download_count: Option<u32>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct SubdlSearchOutcome {
    pub candidates: Vec<SubdlCandidate>,
    pub from_cache: bool,
}

/// Score a SubDL subtitle entry. Indonesian subtitles get a large language
/// bonus; year/title matches in the release name, download count and rating
/// add smaller bonuses. Mirrors `score_candidate` from `mod.rs`.
pub fn score_subdl_candidate(
    item: &SubdlSubtitleItem,
    ctx: &SubtitleContext,
    title_year: Option<i64>,
) -> i32 {
    let mut score = 0;
    let lang = item.language.as_deref().unwrap_or("");
    if lang.eq_ignore_ascii_case("id") || lang.eq_ignore_ascii_case("indonesian") {
        score += 50;
    }
    let year_hit = ctx
        .year
        .as_deref()
        .map(str::to_string)
        .or_else(|| title_year.map(|y| y.to_string()));
    if let Some(yr) = year_hit {
        if item
            .release_name
            .as_deref()
            .is_some_and(|r| r.contains(&yr))
        {
            score += 15;
        }
    }
    if item.release_name.as_deref().is_some_and(|r| {
        ctx.title.split_whitespace().take(3).any(|tok| {
            let t = tok
                .trim_matches(|c: char| !c.is_alphanumeric())
                .to_lowercase();
            t.len() >= 4 && r.to_lowercase().contains(&t)
        })
    }) {
        score += 20;
    }
    if let Some(dc) = item.download_count {
        score += (dc.min(100_000) as f32 / 10_000.0) as i32;
    }
    if let Some(r) = item.rating {
        // Up to +20 for a perfect 10.0 rating.
        score += (r.clamp(0.0, 10.0) * 2.0) as i32;
    }
    score
}

/// Human-readable label with the `[SubDL]` marker (mirrors `build_label`).
pub fn build_subdl_label(c: &SubdlCandidate) -> String {
    let lang =
        if c.language.eq_ignore_ascii_case("id") || c.language.eq_ignore_ascii_case("indonesian") {
            "Indonesian".to_string()
        } else {
            c.language.clone()
        };
    let mut label = format!("{lang} [SubDL]");
    if let Some(rn) = &c.release_name {
        let short: String = rn.chars().take(40).collect();
        label.push_str(&format!(" · {short}"));
    }
    if let Some(dc) = c.download_count {
        label.push_str(&format!(" · {dc} dl"));
    }
    if let Some(r) = c.rating {
        label.push_str(&format!(" · {r:.1}★"));
    }
    label
}

/// Marker used to identify this candidate in the subtitle list UI (mirrors
/// the `os:{file_id}:{lang}` marker of OpenSubtitles).
pub fn subdl_marker(c: &SubdlCandidate) -> String {
    format!("subdl:{}:{}", c.subtitle_id, c.language)
}

/// Build the anonymous download URL for a subtitle id (`n_id`).
fn subdl_download_url(subtitle_id: &str) -> String {
    format!("{DOWNLOAD_BASE_URL}/subtitle/{subtitle_id}.zip")
}

/// Extract the `n_id` from a download path like `/subtitle/3197651-3213944.zip`
/// or `https://dl.subdl.com/subtitle/3197651-3213944.zip`. Also accepts a
/// bare id (`3197651-3213944`).
fn subtitle_id_from_url(url: &str) -> Option<String> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return None;
    }
    if !trimmed.contains(['/', '.']) {
        return Some(trimmed.to_string());
    }
    let last = trimmed.rsplit('/').next()?;
    let id = last.split('.').next().unwrap_or(last);
    if id.is_empty() {
        None
    } else {
        Some(id.to_string())
    }
}

/// Extract the first usable subtitle file (`.srt`, `.vtt`, `.ass`, `.ssa` or
/// `.sub`) from a SubDL zip archive, returning `(extension, bytes)`. Folders
/// are skipped and only the basename of each entry is used, so a malicious
/// archive cannot escape the (in-memory) extraction via `../` or absolute
/// paths (zip-slip safe).
fn extract_first_subtitle_from_zip(bytes: &[u8]) -> Result<(String, Vec<u8>), SubdlError> {
    let reader = std::io::Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(reader)?;
    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        if file.is_dir() {
            continue;
        }
        let Some(basename) = safe_basename(file.name()) else {
            continue;
        };
        let Some(ext) = subtitle_ext_of(&basename) else {
            continue;
        };
        let mut buf = Vec::new();
        file.read_to_end(&mut buf)?;
        return Ok((ext.to_string(), buf));
    }
    Err(SubdlError::NoSubtitleInZip)
}

/// Reduce an entry name to its basename, rejecting path traversal (`..`) and
/// absolute paths. Returns `None` when the entry is unsafe or has no name.
fn safe_basename(name: &str) -> Option<String> {
    let normalized = name.replace('\\', "/");
    if normalized.starts_with('/') || normalized.contains("..") {
        return None;
    }
    let base = normalized.rsplit('/').next().unwrap_or(normalized.as_str());
    if base.is_empty() {
        None
    } else {
        Some(base.to_string())
    }
}

/// Recognized subtitle extension of a file name, or `None`.
fn subtitle_ext_of(name: &str) -> Option<&'static str> {
    let ext = name.rsplit('.').next()?.to_ascii_lowercase();
    ACCEPTED_SUBTITLE_EXTS
        .iter()
        .copied()
        .find(|a| *a == ext.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::models::ProviderKind;
    use std::io::Write;

    fn sample_context() -> SubtitleContext {
        SubtitleContext {
            provider: ProviderKind::MovieBox,
            subject_id: "1".into(),
            resource_id: "1".into(),
            title: "Oppenheimer".into(),
            year: Some("2023".into()),
            is_episode: false,
            season: None,
            episode: None,
            imdb_id: Some("tt15398776".into()),
        }
    }

    #[test]
    fn test_parse_search_response_fixture() {
        // Fixture taken from RESEARCH-SUBDL-API.md §4.1 (v1 search shape,
        // tolerant fields for v2).
        let json_data = serde_json::json!({
            "status": true,
            "results": [
                {
                    "imdb_id": "tt1375666",
                    "tmdb_id": 27205,
                    "type": "movie",
                    "name": "Inception",
                    "sd_id": 123456,
                    "first_air_date": null,
                    "year": 2010
                }
            ],
            "subtitles": [
                {
                    "release_name": "Season Pack",
                    "name": "Season.Pack.zip",
                    "url": "/subtitle/3197651-3213944.zip",
                    "season": 1,
                    "episode": 1,
                    "framerate": 2,
                    "fps": "23.976",
                    "episode_from": 1,
                    "episode_end": 10,
                    "full_season": true,
                    "unpack_files": []
                },
                {
                    "release_name": "Oppenheimer.2023.1080p.WEB-DL",
                    "name": "Oppenheimer.id.srt",
                    "url": "/subtitle/3197652-3213945.zip",
                    "season": 0,
                    "episode": 0,
                    "framerate": 1,
                    "fps": "24",
                    "language": "ID",
                    "download_count": 123,
                    "rating": 8.5
                }
            ]
        });
        let parsed: SubdlSearchResponse = serde_json::from_value(json_data).unwrap();
        assert_eq!(parsed.results.len(), 1);
        assert_eq!(parsed.results[0].imdb_id.as_deref(), Some("tt1375666"));
        assert_eq!(parsed.results[0].year, Some(2010));
        assert_eq!(parsed.subtitles.len(), 2);

        // subtitle_id is parsed from the relative download URL.
        let first = parsed.subtitles[0].subtitle_id().unwrap();
        assert_eq!(first, "3197651-3213944");

        // Language code, download count and rating are parsed.
        let second = &parsed.subtitles[1];
        assert_eq!(second.language.as_deref(), Some("ID"));
        assert_eq!(second.download_count, Some(123));
        assert_eq!(second.rating, Some(8.5));
    }

    #[test]
    fn test_subtitle_id_from_url_variants() {
        assert_eq!(
            subtitle_id_from_url("/subtitle/3197651-3213944.zip"),
            Some("3197651-3213944".to_string())
        );
        assert_eq!(
            subtitle_id_from_url("https://dl.subdl.com/subtitle/3197651-3213944.zip"),
            Some("3197651-3213944".to_string())
        );
        assert_eq!(
            subtitle_id_from_url("3197651-3213944"),
            Some("3197651-3213944".to_string())
        );
        assert_eq!(subtitle_id_from_url(""), None);
        assert_eq!(subtitle_id_from_url("   "), None);
    }

    #[test]
    fn test_subdl_download_url() {
        assert_eq!(
            subdl_download_url("3197651-3213944"),
            "https://dl.subdl.com/subtitle/3197651-3213944.zip"
        );
    }

    #[test]
    fn test_build_subdl_label_and_marker() {
        let c = SubdlCandidate {
            subtitle_id: "3197651-3213944".into(),
            language: "ID".into(),
            score: 60,
            release_name: Some("Oppenheimer.2023.1080p.WEB-DL".into()),
            rating: Some(8.5),
            download_count: Some(123),
        };
        let label = build_subdl_label(&c);
        assert!(label.starts_with("Indonesian [SubDL]"), "label: {label}");
        assert!(label.contains("[SubDL]"), "label: {label}");
        assert!(label.contains("123 dl"), "label: {label}");
        assert!(label.contains("8.5"), "label: {label}");
        // Release name is truncated to 40 chars.
        let short = label.split(" · ").nth(1).unwrap();
        assert!(short.chars().count() <= 40, "label: {label}");

        let marker = subdl_marker(&c);
        assert_eq!(marker, "subdl:3197651-3213944:ID");
        assert!(marker.starts_with("subdl:"));
    }

    #[test]
    fn test_score_subdl_candidate_prioritizes_id() {
        let id_item: SubdlSubtitleItem = serde_json::from_value(serde_json::json!({
            "language": "ID",
            "url": "/subtitle/1.zip"
        }))
        .unwrap();
        let en_item: SubdlSubtitleItem = serde_json::from_value(serde_json::json!({
            "language": "EN",
            "url": "/subtitle/2.zip"
        }))
        .unwrap();
        let ctx = sample_context();
        let id_score = score_subdl_candidate(&id_item, &ctx, None);
        let en_score = score_subdl_candidate(&en_item, &ctx, None);
        assert!(id_score > en_score, "expected {id_score} > {en_score}");
    }

    #[test]
    fn test_score_subdl_candidate_year_and_title_bonus() {
        let with_bonus: SubdlSubtitleItem = serde_json::from_value(serde_json::json!({
            "language": "EN",
            "release_name": "Oppenheimer.2023.1080p.BluRay.x264",
            "url": "/subtitle/1.zip"
        }))
        .unwrap();
        let without_bonus: SubdlSubtitleItem = serde_json::from_value(serde_json::json!({
            "language": "EN",
            "release_name": "Some Other Movie",
            "url": "/subtitle/2.zip"
        }))
        .unwrap();
        let ctx = sample_context();
        let a = score_subdl_candidate(&with_bonus, &ctx, None);
        let b = score_subdl_candidate(&without_bonus, &ctx, None);
        assert!(a > b, "expected year+title bonus, got {a} <= {b}");
    }

    /// Test-only helper: set/clear an env var, run `f`, then restore the
    /// previous value. Keeps the mutation window minimal for test isolation.
    fn with_env(key: &str, val: Option<&str>, f: impl FnOnce()) {
        let prev = std::env::var(key).ok();
        // SAFETY: test-only; the variable is restored right after the closure.
        unsafe {
            match val {
                Some(v) => std::env::set_var(key, v),
                None => std::env::remove_var(key),
            }
        }
        f();
        // SAFETY: restoring the previous value keeps other tests isolated.
        unsafe {
            match prev {
                Some(p) => std::env::set_var(key, p),
                None => std::env::remove_var(key),
            }
        }
    }

    /// Shared lock so env-var-dependent tests never run concurrently (cargo
    /// runs tests in parallel by default).
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn test_config_from_env_enabled() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());

        // Key present -> enabled with default languages.
        with_env("MOVIEBOX_SUBDL_API_KEY", Some("key"), || {
            with_env("MOVIEBOX_SUBDL_ENABLED", None, || {
                with_env("MOVIEBOX_SUBDL_LANGUAGES", None, || {
                    let cfg = SubdlConfig::from_env();
                    assert!(cfg.enabled());
                    assert_eq!(cfg.languages, vec!["id".to_string(), "en".to_string()]);
                });
            });
        });

        // No key -> disabled even when the flag defaults to true.
        with_env("MOVIEBOX_SUBDL_API_KEY", None, || {
            with_env("MOVIEBOX_SUBDL_ENABLED", None, || {
                let cfg = SubdlConfig::from_env();
                assert!(!cfg.enabled());
            });
        });

        // Explicitly disabled wins even with a key present.
        with_env("MOVIEBOX_SUBDL_API_KEY", Some("key"), || {
            with_env("MOVIEBOX_SUBDL_ENABLED", Some("false"), || {
                let cfg = SubdlConfig::from_env();
                assert!(!cfg.enabled());
            });
        });

        // Custom languages + base url are read.
        with_env("MOVIEBOX_SUBDL_API_KEY", Some("key"), || {
            with_env("MOVIEBOX_SUBDL_LANGUAGES", Some("fr,de"), || {
                with_env(
                    "MOVIEBOX_SUBDL_BASE_URL",
                    Some("https://mirror.example"),
                    || {
                        let cfg = SubdlConfig::from_env();
                        assert!(cfg.enabled());
                        assert_eq!(cfg.languages, vec!["fr".to_string(), "de".to_string()]);
                        assert_eq!(cfg.base_url.as_deref(), Some("https://mirror.example"));
                    },
                );
            });
        });
    }

    /// Build an in-memory zip archive from `(name, content)` entries.
    fn build_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut buf = Vec::new();
        {
            let mut writer = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
            let options = zip::write::SimpleFileOptions::default();
            for (name, data) in entries {
                writer.start_file(*name, options).unwrap();
                writer.write_all(data).unwrap();
            }
            writer.finish().unwrap();
        }
        buf
    }

    #[test]
    fn test_extract_srt_from_zip() {
        let zip_bytes = build_zip(&[(
            "Oppenheimer.2023.WEB-DL[@AirenTeam].srt",
            b"1\n00:00:01,000 --> 00:00:02,000\nTest\n",
        )]);
        let (ext, bytes) = extract_first_subtitle_from_zip(&zip_bytes).unwrap();
        assert_eq!(ext, "srt");
        assert!(bytes.starts_with(b"1\n00:00:01,000"));
    }

    #[test]
    fn test_extract_skips_folder_and_other_files() {
        // A readme + a nested .srt: the extractor must skip non-subtitle
        // files and folders, then pick the .srt.
        let zip_bytes = build_zip(&[
            ("readme.txt", b"hello"),
            (
                "folder/movie.id.srt",
                b"1\n00:00:01,000 --> 00:00:02,000\nTest\n",
            ),
        ]);
        let (ext, bytes) = extract_first_subtitle_from_zip(&zip_bytes).unwrap();
        assert_eq!(ext, "srt");
        assert!(bytes.starts_with(b"1\n00:00:01,000"));
    }

    #[test]
    fn test_extract_prefers_vtt_extension() {
        let zip_bytes = build_zip(&[(
            "movie.id.vtt",
            b"WEBVTT\n\n00:00:01.000 --> 00:00:02.000\nTest\n",
        )]);
        let (ext, _) = extract_first_subtitle_from_zip(&zip_bytes).unwrap();
        assert_eq!(ext, "vtt");
    }

    #[test]
    fn test_extract_empty_or_without_subtitle_errors() {
        // Empty archive.
        assert!(matches!(
            extract_first_subtitle_from_zip(&[]),
            Err(SubdlError::Zip(_))
        ));
        // Archive with only non-subtitle files.
        let zip_bytes = build_zip(&[("notes.txt", b"no subtitle here")]);
        assert!(matches!(
            extract_first_subtitle_from_zip(&zip_bytes),
            Err(SubdlError::NoSubtitleInZip)
        ));
    }

    #[test]
    fn test_safe_basename_rejects_traversal() {
        assert_eq!(safe_basename("../evil.srt"), None);
        assert_eq!(safe_basename("../../etc/passwd"), None);
        assert_eq!(safe_basename("/absolute/path.srt"), None);
        assert_eq!(safe_basename("a/b/../c.srt"), None);
        assert_eq!(safe_basename(""), None);
        assert_eq!(safe_basename("folder/sub.srt"), Some("sub.srt".to_string()));
        assert_eq!(safe_basename("plain.srt"), Some("plain.srt".to_string()));
        // Windows-style separators are normalized.
        assert_eq!(
            safe_basename("folder\\sub.srt"),
            Some("sub.srt".to_string())
        );
    }

    #[test]
    fn test_zip_slip_traversal_rejected() {
        let mut buf = Vec::new();
        let wrote = {
            let mut writer = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
            let options = zip::write::SimpleFileOptions::default();
            match writer.start_file("../evil.srt", options) {
                Ok(()) => {
                    let _ = writer.write_all(b"evil");
                    let _ = writer.finish();
                    true
                }
                Err(_) => false,
            }
        };
        if wrote {
            // If the writer accepted the traversal name, our extractor must
            // refuse it (never yields the entry content).
            assert!(matches!(
                extract_first_subtitle_from_zip(&buf),
                Err(SubdlError::NoSubtitleInZip)
            ));
        }
        // The guard itself is exercised deterministically above.
        assert_eq!(safe_basename("../evil.srt"), None);
    }
}
