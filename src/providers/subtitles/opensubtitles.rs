use super::cache;
use super::{SubtitleContext, score_candidate};
use crate::providers::subtitles::{OsCandidate, OsSearchOutcome};
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::Mutex;

pub const BASE_URL: &str = "https://api.opensubtitles.com/api/v1";

/// Browser-like User-Agent so OpenSubtitles/Cloudflare doesn't reject the
/// plain reqwest client with HTTP 403.
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0 Safari/537.36 MovieBox-Tui/1.0";

/// Maximum bytes buffered for a subtitle file (50 MB). Endpoints answering
/// larger bodies are rejected instead of exhausting memory.
const MAX_SUBTITLE_BYTES: u64 = 50 * 1024 * 1024;

fn urlencoding_simple(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.' || b == b'~' {
            out.push(b as char);
        } else if b == b' ' {
            out.push('+');
        } else {
            out.push_str(&format!("%{:02X}", b));
        }
    }
    out
}

#[derive(Debug, Clone, Default)]
pub struct OpenSubtitlesConfig {
    pub api_key: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub languages: Vec<String>,
    pub enabled: bool,
    pub base_url: Option<String>,
}

impl OpenSubtitlesConfig {
    pub fn from_env() -> Self {
        let enabled = std::env::var("MOVIEBOX_OPENSUBTITLES_ENABLED")
            .map(|v| !v.eq_ignore_ascii_case("false") && v != "0")
            .unwrap_or(true);
        let languages = std::env::var("MOVIEBOX_OPENSUBTITLES_LANGUAGES")
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
            api_key: std::env::var("MOVIEBOX_OPENSUBTITLES_API_KEY").ok(),
            username: std::env::var("MOVIEBOX_OPENSUBTITLES_USERNAME").ok(),
            password: std::env::var("MOVIEBOX_OPENSUBTITLES_PASSWORD").ok(),
            languages,
            enabled,
            base_url: std::env::var("MOVIEBOX_OPENSUBTITLES_BASE_URL").ok(),
        }
    }

    pub fn enabled(&self) -> bool {
        self.enabled
            && self.api_key.as_deref().is_some_and(|s| !s.is_empty())
            && self.username.as_deref().is_some_and(|s| !s.is_empty())
            && self.password.as_deref().is_some_and(|s| !s.is_empty())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum OpenSubtitlesError {
    #[error("missing OpenSubtitles credential: {0}")]
    MissingCredentials(&'static str),
    #[error("login failed: HTTP {0}: {1}")]
    LoginHttp(u16, String),
    #[error("login response has no token")]
    MissingToken,
    #[error("OpenSubtitles API error: HTTP {0}: {1}")]
    Http(u16, String),
    #[error("rate limited (retry after ~{0}s)")]
    RateLimited(u64),
    #[error("download quota exhausted: {0}")]
    Quota(String),
    #[error("subtitle file too large (over {0} MB)")]
    BodyTooLarge(u64),
    #[error("subtitle not found")]
    NotFound,
    #[error("reqwest error: {0}")]
    Reqwest(#[from] reqwest::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Clone)]
pub struct OpenSubtitlesClient {
    http: reqwest::Client,
    config: OpenSubtitlesConfig,
    token: Arc<Mutex<Option<String>>>,
    last_login_at: Arc<Mutex<Option<u64>>>,
}

/// Map a known OpenSubtitles gateway (Kong/Cloudflare) error body to a
/// friendly, actionable message. The HTTP status code stays in the error
/// variant (`LoginHttp`/`Http`), so only the body text is replaced here.
/// Falls back to the original snippet when nothing matches.
fn friendly_http_error(_status: u16, body: &str) -> String {
    let lower = body.to_ascii_lowercase();
    if lower.contains("kong-user-agent-block") {
        "User-Agent diblokir gateway OpenSubtitles (perlu User-Agent browser/valid)".to_string()
    } else if lower.contains("you cannot consume this service") {
        "API key tidak valid atau tidak terdaftar di OpenSubtitles".to_string()
    } else if lower.contains("missing username and password") {
        "Field username/password tidak terkirim dengan benar".to_string()
    } else if lower.contains("<html") || lower.contains("<!doctype") {
        "Server mengembalikan halaman error (kemungkinan diblokir WAF/Cloudflare)".to_string()
    } else {
        body.to_string()
    }
}

impl OpenSubtitlesClient {
    pub fn new(config: OpenSubtitlesConfig) -> Self {
        let http = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(std::time::Duration::from_secs(12))
            .connect_timeout(std::time::Duration::from_secs(5))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Self {
            http,
            config,
            token: Arc::new(Mutex::new(None)),
            last_login_at: Arc::new(Mutex::new(None)),
        }
    }

    pub fn from_env() -> Self {
        Self::new(OpenSubtitlesConfig::from_env())
    }

    pub fn enabled(&self) -> bool {
        self.config.enabled()
    }

    fn base_url(&self) -> &str {
        self.config.base_url.as_deref().unwrap_or(BASE_URL)
    }

    /// Read the response body as a short single-line snippet for error
    /// messages, capped at 300 chars so a huge Cloudflare page can't bloat
    /// the UI. Falls back to a placeholder when the body is unreadable.
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

    /// Read a response body into memory, rejecting it when `Content-Length`
    /// exceeds `cap` or the stream grows beyond it (guards against a
    /// misbehaving download host serving multi-GB responses).
    async fn read_body_limited(
        mut res: reqwest::Response,
        cap: u64,
    ) -> Result<Vec<u8>, OpenSubtitlesError> {
        if let Some(len) = res.content_length() {
            if len > cap {
                return Err(OpenSubtitlesError::BodyTooLarge(cap / (1024 * 1024)));
            }
        }
        let mut buf = Vec::with_capacity(res.content_length().unwrap_or(0).min(cap) as usize);
        while let Some(chunk) = res.chunk().await? {
            if buf.len() as u64 + chunk.len() as u64 > cap {
                return Err(OpenSubtitlesError::BodyTooLarge(cap / (1024 * 1024)));
            }
            buf.extend_from_slice(&chunk);
        }
        Ok(buf)
    }

    pub async fn ensure_token(&self) -> Result<String, OpenSubtitlesError> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        {
            let token_guard = self.token.lock().await;
            let login_at_guard = self.last_login_at.lock().await;
            if let (Some(t), Some(last)) = (token_guard.as_ref(), *login_at_guard) {
                if now.saturating_sub(last) < 20 * 3600 {
                    return Ok(t.clone());
                }
            }
        }

        let api_key = self
            .config
            .api_key
            .as_deref()
            .ok_or(OpenSubtitlesError::MissingCredentials("api_key"))?;
        let username = self
            .config
            .username
            .as_deref()
            .ok_or(OpenSubtitlesError::MissingCredentials("username"))?;
        let password = self
            .config
            .password
            .as_deref()
            .ok_or(OpenSubtitlesError::MissingCredentials("password"))?;

        let url = format!("{}/login", self.base_url());
        let req = self
            .http
            .post(&url)
            .header("Api-Key", api_key)
            .json(&serde_json::json!({
                "username": username,
                "password": password
            }));

        // Login shares the same 429 policy as every other API call: honour
        // `Retry-After` (capped), retry once, then surface `RateLimited` if
        // the gateway is still throttling us.
        let res = self.send_with_retry_429(req).await?;
        if res.status().as_u16() == 429 {
            let retry_after = res
                .headers()
                .get("Retry-After")
                .and_then(|h| h.to_str().ok())
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(3);
            return Err(OpenSubtitlesError::RateLimited(retry_after));
        }

        if !res.status().is_success() {
            let status = res.status().as_u16();
            let body = Self::body_snippet(res).await;
            let message = friendly_http_error(status, &body);
            return Err(OpenSubtitlesError::LoginHttp(status, message));
        }

        let login_res: LoginResponse = res.json().await?;
        let token = login_res.user.token;
        if token.is_empty() {
            return Err(OpenSubtitlesError::MissingToken);
        }

        *self.token.lock().await = Some(token.clone());
        *self.last_login_at.lock().await = Some(now);

        Ok(token)
    }

    pub async fn search(
        &self,
        ctx: &SubtitleContext,
    ) -> Result<OsSearchOutcome, OpenSubtitlesError> {
        let query_key = cache::search_query_key(ctx, &self.config.languages.join(","));
        if let Some(cached_val) = cache::get_search_cache(&query_key) {
            if let Ok(search_res) = serde_json::from_value::<SearchResponse>(cached_val) {
                let candidates = self.parse_and_score_search(search_res, ctx);
                return Ok(OsSearchOutcome {
                    candidates,
                    from_cache: true,
                });
            }
        }

        let api_key = self
            .config
            .api_key
            .as_deref()
            .ok_or(OpenSubtitlesError::MissingCredentials("api_key"))?;

        let mut query_params: Vec<(&str, String)> = Vec::new();

        let langs = self.config.languages.join(",");
        query_params.push(("languages", langs));

        if let Some(imdb_id) = &ctx.imdb_id {
            query_params.push(("imdb_id", imdb_id.clone()));
            if ctx.is_episode {
                query_params.push(("type", "episode".to_string()));
                if let Some(s) = ctx.season {
                    query_params.push(("season_number", s.to_string()));
                }
                if let Some(e) = ctx.episode {
                    query_params.push(("episode_number", e.to_string()));
                }
            } else {
                query_params.push(("type", "movie".to_string()));
            }
        } else {
            query_params.push(("query", ctx.title.clone()));
            if let Some(yr) = &ctx.year {
                query_params.push(("year", yr.clone()));
            }
            if ctx.is_episode {
                query_params.push(("type", "episode".to_string()));
                if let Some(s) = ctx.season {
                    query_params.push(("season_number", s.to_string()));
                }
                if let Some(e) = ctx.episode {
                    query_params.push(("episode_number", e.to_string()));
                }
            } else {
                query_params.push(("type", "movie".to_string()));
            }
        }

        let query_string = query_params
            .iter()
            .map(|(k, v)| format!("{}={}", k, urlencoding_simple(v)))
            .collect::<Vec<_>>()
            .join("&");

        let url = format!("{}/subtitles?{}", self.base_url(), query_string);
        let req = self.http.get(&url).header("Api-Key", api_key);

        let res = self.send_with_retry_429(req).await?;
        if res.status().as_u16() == 429 {
            let retry_after = res
                .headers()
                .get("Retry-After")
                .and_then(|h| h.to_str().ok())
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(3);
            return Err(OpenSubtitlesError::RateLimited(retry_after));
        }
        if !res.status().is_success() {
            let status = res.status().as_u16();
            let body = Self::body_snippet(res).await;
            let message = friendly_http_error(status, &body);
            return Err(OpenSubtitlesError::Http(status, message));
        }

        let raw_val: serde_json::Value = res.json().await?;
        cache::set_search_cache(&query_key, &raw_val);

        let search_res: SearchResponse = serde_json::from_value(raw_val)?;
        let candidates = self.parse_and_score_search(search_res, ctx);

        Ok(OsSearchOutcome {
            candidates,
            from_cache: false,
        })
    }

    fn parse_and_score_search(
        &self,
        search_res: SearchResponse,
        ctx: &SubtitleContext,
    ) -> Vec<OsCandidate> {
        let mut list = Vec::new();
        for item in search_res.data {
            let score = score_candidate(&item, ctx);
            for file in item.attributes.files {
                let lang = item
                    .attributes
                    .language
                    .clone()
                    .unwrap_or_else(|| "und".into());
                let machine_translated = item.attributes.ai_translated.unwrap_or(false)
                    || item.attributes.machine_translated.unwrap_or(false);
                list.push(OsCandidate {
                    label: String::new(),
                    file_id: file.file_id,
                    language: lang,
                    score,
                    release_name: item.attributes.release_name.clone(),
                    download_count: item.attributes.download_count,
                    machine_translated,
                });
            }
        }
        list.sort_by_key(|b| std::cmp::Reverse(b.score));
        list.truncate(5);
        list
    }

    /// Send a request, retrying once when the API responds 429 (rate limited).
    /// The retry honours `Retry-After`, capped at 3 seconds.
    async fn send_with_retry_429(
        &self,
        req: reqwest::RequestBuilder,
    ) -> Result<reqwest::Response, OpenSubtitlesError> {
        let mut attempt = 0;
        loop {
            let builder = req
                .try_clone()
                .ok_or_else(|| OpenSubtitlesError::Http(0, String::new()))?;
            let res = builder.send().await?;
            if res.status().as_u16() == 429 && attempt == 0 {
                let retry_after = res
                    .headers()
                    .get("Retry-After")
                    .and_then(|h| h.to_str().ok())
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or(3);
                tokio::time::sleep(std::time::Duration::from_secs(retry_after.min(3))).await;
                attempt += 1;
                continue;
            }
            return Ok(res);
        }
    }

    pub async fn download_link(
        &self,
        file_id: u32,
    ) -> Result<DownloadResponse, OpenSubtitlesError> {
        let token = self.ensure_token().await?;
        let api_key = self
            .config
            .api_key
            .as_deref()
            .ok_or(OpenSubtitlesError::MissingCredentials("api_key"))?;

        let url = format!("{}/download", self.base_url());
        let req = self
            .http
            .post(&url)
            .header("Api-Key", api_key)
            .header("Authorization", format!("Bearer {token}"))
            .json(&serde_json::json!({ "file_id": file_id }));
        let res = self.send_with_retry_429(req).await?;

        if res.status().as_u16() == 429 {
            let retry_after = res
                .headers()
                .get("Retry-After")
                .and_then(|h| h.to_str().ok())
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(3);
            return Err(OpenSubtitlesError::RateLimited(retry_after));
        }

        if !res.status().is_success() {
            let status = res.status().as_u16();
            let body = Self::body_snippet(res).await;
            let message = friendly_http_error(status, &body);
            return Err(OpenSubtitlesError::Http(status, message));
        }

        let dl_res: DownloadResponse = res.json().await?;
        if let (Some(reqs), Some(rem)) = (dl_res.requests, dl_res.remaining) {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            cache::set_quota_cache(&cache::QuotaInfo {
                requests: reqs,
                remaining: rem,
                updated_at: now,
            });
        }

        Ok(dl_res)
    }

    pub async fn fetch_bytes(&self, link: &str) -> Result<Vec<u8>, OpenSubtitlesError> {
        // The download link requires the same `Api-Key` header as every other
        // endpoint; without it OpenSubtitles answers HTTP 403.
        let api_key = self
            .config
            .api_key
            .as_deref()
            .ok_or(OpenSubtitlesError::MissingCredentials("api_key"))?;
        let req = self.http.get(link).header("Api-Key", api_key);
        // The download host throttles like the API does; retry once honouring
        // `Retry-After` (same policy as `send_with_retry_429`) so a single 429
        // does not waste the already-decremented download quota.
        let res = self.send_with_retry_429(req).await?;
        if res.status().as_u16() == 429 {
            let retry_after = res
                .headers()
                .get("Retry-After")
                .and_then(|h| h.to_str().ok())
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(3);
            return Err(OpenSubtitlesError::RateLimited(retry_after));
        }
        let res = res.error_for_status()?;
        Self::read_body_limited(res, MAX_SUBTITLE_BYTES).await
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct LoginResponse {
    pub user: LoginUser,
    #[serde(default)]
    pub status: Option<u16>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LoginUser {
    pub token: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SearchResponse {
    #[serde(default)]
    pub total_count: Option<u32>,
    #[serde(default)]
    pub data: Vec<SubtitleItem>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SubtitleItem {
    pub id: String,
    #[serde(rename = "type", default)]
    pub item_type: Option<String>,
    pub attributes: SubtitleAttributes,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SubtitleAttributes {
    #[serde(default)]
    pub language: Option<String>,
    #[serde(rename = "release_name", default)]
    pub release_name: Option<String>,
    #[serde(default)]
    pub files: Vec<SubtitleFile>,
    #[serde(rename = "download_count", default)]
    pub download_count: Option<u32>,
    #[serde(rename = "ai_translated", default)]
    pub ai_translated: Option<bool>,
    #[serde(rename = "machine_translated", default)]
    pub machine_translated: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SubtitleFile {
    #[serde(rename = "file_id")]
    pub file_id: u32,
    #[serde(rename = "file_name", default)]
    pub file_name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DownloadResponse {
    pub link: String,
    #[serde(rename = "file_name", default)]
    pub file_name: Option<String>,
    #[serde(default)]
    pub requests: Option<u32>,
    #[serde(default)]
    pub remaining: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_login_response() {
        let json_data = serde_json::json!({
            "user": {
                "token": "secret_jwt_token"
            },
            "status": 200
        });
        let parsed: LoginResponse = serde_json::from_value(json_data).unwrap();
        assert_eq!(parsed.user.token, "secret_jwt_token");
    }

    #[test]
    fn test_parse_search_response() {
        let json_data = serde_json::json!({
            "total_count": 1,
            "data": [{
                "id": "101",
                "attributes": {
                    "language": "id",
                    "release_name": "Avengers",
                    "files": [{ "file_id": 555, "file_name": "sub.srt" }]
                }
            }]
        });
        let parsed: SearchResponse = serde_json::from_value(json_data).unwrap();
        assert_eq!(parsed.data.len(), 1);
        assert_eq!(parsed.data[0].attributes.files[0].file_id, 555);
    }

    #[test]
    fn test_parse_download_response() {
        let json_data = serde_json::json!({
            "link": "https://dl.opensubtitles.org/en/file/abc.srt",
            "file_name": "movie.id.srt",
            "requests": 5,
            "remaining": 15
        });
        let parsed: DownloadResponse = serde_json::from_value(json_data).unwrap();
        assert_eq!(parsed.link, "https://dl.opensubtitles.org/en/file/abc.srt");
        assert_eq!(parsed.file_name.as_deref(), Some("movie.id.srt"));
        assert_eq!(parsed.requests, Some(5));
        assert_eq!(parsed.remaining, Some(15));
    }

    #[test]
    fn test_parse_search_empty() {
        let json_data = serde_json::json!({ "data": [] });
        let parsed: SearchResponse = serde_json::from_value(json_data).unwrap();
        assert!(parsed.data.is_empty());
        assert_eq!(parsed.total_count, None);
    }

    #[test]
    fn test_deserialize_missing_optional_fields() {
        let json_data = serde_json::json!({
            "id": "42",
            "attributes": {
                "language": "id",
                "files": [{ "file_id": 7 }]
            }
        });
        let parsed: SubtitleItem = serde_json::from_value(json_data).unwrap();
        assert_eq!(parsed.attributes.ai_translated, None);
        assert_eq!(parsed.attributes.release_name, None);
        assert_eq!(parsed.attributes.download_count, None);
        assert_eq!(parsed.attributes.machine_translated, None);
        assert_eq!(parsed.attributes.files[0].file_name, None);
    }

    #[test]
    fn test_parse_language_as_name() {
        let json_data = serde_json::json!({
            "id": "9",
            "attributes": {
                "language": "Indonesian",
                "files": [{ "file_id": 9 }]
            }
        });
        let item: SubtitleItem = serde_json::from_value(json_data).unwrap();
        let ctx = SubtitleContext::default();
        let score = score_candidate(&item, &ctx);
        assert!(score >= 50, "expected language bonus >= 50, got {score}");
    }

    #[test]
    fn test_missing_language_defaults_to_und_not_indonesian() {
        // Regression: an API entry without a `language` must NOT be tagged
        // Indonesian (which would win the +50 bonus and be auto-picked as the
        // Indonesian fallback).
        let search_res: SearchResponse = serde_json::from_value(serde_json::json!({
            "total_count": 1,
            "data": [{
                "id": "1",
                "attributes": {
                    "release_name": "Movie",
                    "files": [{ "file_id": 1 }],
                    "download_count": 0
                }
            }]
        }))
        .unwrap();
        let client = OpenSubtitlesClient::new(OpenSubtitlesConfig::default());
        let cands = client.parse_and_score_search(search_res, &SubtitleContext::default());
        assert_eq!(cands.len(), 1);
        assert_eq!(cands[0].language, "und");
        assert!(
            cands[0].score < 50,
            "missing language must not get the +50 Indonesian bonus, got {}",
            cands[0].score
        );
        let label = crate::providers::subtitles::build_label(&cands[0]);
        assert!(!label.starts_with("Indonesian"), "label: {label}");
        // The explicit `id` path still wins the bonus.
        let id_item: SubtitleItem = serde_json::from_value(serde_json::json!({
            "id": "2",
            "attributes": { "language": "id", "files": [{ "file_id": 2 }], "download_count": 0 }
        }))
        .unwrap();
        assert!(score_candidate(&id_item, &SubtitleContext::default()) >= 50);
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

        // All credentials present -> enabled with default languages.
        with_env("MOVIEBOX_OPENSUBTITLES_API_KEY", Some("key"), || {
            with_env("MOVIEBOX_OPENSUBTITLES_USERNAME", Some("user"), || {
                with_env("MOVIEBOX_OPENSUBTITLES_PASSWORD", Some("pass"), || {
                    with_env("MOVIEBOX_OPENSUBTITLES_ENABLED", None, || {
                        let cfg = OpenSubtitlesConfig::from_env();
                        assert!(cfg.enabled());
                        assert_eq!(cfg.languages, vec!["id".to_string(), "en".to_string()]);
                    });
                });
            });
        });

        // One credential missing -> disabled.
        with_env("MOVIEBOX_OPENSUBTITLES_API_KEY", Some("key"), || {
            with_env("MOVIEBOX_OPENSUBTITLES_USERNAME", Some("user"), || {
                with_env("MOVIEBOX_OPENSUBTITLES_PASSWORD", None, || {
                    let cfg = OpenSubtitlesConfig::from_env();
                    assert!(!cfg.enabled());
                });
            });
        });

        // Explicitly disabled wins even with all credentials present.
        with_env("MOVIEBOX_OPENSUBTITLES_API_KEY", Some("key"), || {
            with_env("MOVIEBOX_OPENSUBTITLES_USERNAME", Some("user"), || {
                with_env("MOVIEBOX_OPENSUBTITLES_PASSWORD", Some("pass"), || {
                    with_env("MOVIEBOX_OPENSUBTITLES_ENABLED", Some("false"), || {
                        let cfg = OpenSubtitlesConfig::from_env();
                        assert!(!cfg.enabled());
                    });
                });
            });
        });
    }

    #[test]
    fn test_config_default_languages() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        with_env("MOVIEBOX_OPENSUBTITLES_LANGUAGES", None, || {
            let cfg = OpenSubtitlesConfig::from_env();
            assert_eq!(cfg.languages, vec!["id".to_string(), "en".to_string()]);
        });
        with_env("MOVIEBOX_OPENSUBTITLES_LANGUAGES", Some("fr,de"), || {
            let cfg = OpenSubtitlesConfig::from_env();
            assert_eq!(cfg.languages, vec!["fr".to_string(), "de".to_string()]);
        });
    }

    #[test]
    fn test_urlencoding_simple() {
        assert_eq!(urlencoding_simple("hello world"), "hello+world");
        assert_eq!(urlencoding_simple("a&b"), "a%26b");
        assert_eq!(
            urlencoding_simple("café com açúcar"),
            "caf%C3%A9+com+a%C3%A7%C3%BAcar"
        );
        assert_eq!(urlencoding_simple("abc123-_.~"), "abc123-_.~");
    }

    #[tokio::test]
    async fn test_login_flow_http() {
        use std::io::{Read, Write};

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();

        // Minimal local mock server answering POST /login. Non-blocking accept
        // with a deadline so a failing client can never hang the test process.
        let server = std::thread::spawn(move || -> std::io::Result<String> {
            listener.set_nonblocking(true)?;
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
            let (mut stream, _) = loop {
                match listener.accept() {
                    Ok(conn) => break conn,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        if std::time::Instant::now() > deadline {
                            return Err(std::io::Error::new(
                                std::io::ErrorKind::TimedOut,
                                "mock server: no connection",
                            ));
                        }
                        std::thread::sleep(std::time::Duration::from_millis(20));
                    }
                    Err(e) => return Err(e),
                }
            };
            stream.set_read_timeout(Some(std::time::Duration::from_secs(10)))?;
            let mut buf = [0u8; 8192];
            let n = stream.read(&mut buf)?;
            let request = String::from_utf8_lossy(&buf[..n]).to_string();
            let body = r#"{"user":{"token":"tok_123"}}"#;
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            stream.write_all(response.as_bytes())?;
            stream.flush()?;
            Ok(request)
        });

        let config = OpenSubtitlesConfig {
            api_key: Some("test_api_key".into()),
            username: Some("test_user".into()),
            password: Some("test_pass".into()),
            languages: vec!["id".into()],
            enabled: true,
            base_url: Some(format!("http://{}", addr)),
        };
        let client = OpenSubtitlesClient::new(config);

        let token = client.ensure_token().await.unwrap();
        assert_eq!(token, "tok_123");

        let request = server.join().unwrap().unwrap();
        assert!(
            request.starts_with("POST /login HTTP/1.1"),
            "got: {request}"
        );
        assert!(
            request
                .to_ascii_lowercase()
                .contains("api-key: test_api_key"),
            "missing Api-Key header in: {request}"
        );
        assert!(
            request.contains("test_user"),
            "missing username in: {request}"
        );
    }

    #[test]
    fn test_friendly_http_error_kong_user_agent_block() {
        // Arrange
        let body =
            r#"{"message":"Not allowed - blocked by Kong","kong-user-agent-block":"not_allowed"}"#;

        // Act
        let msg = friendly_http_error(403, body);

        // Assert
        assert_eq!(
            msg,
            "User-Agent diblokir gateway OpenSubtitles (perlu User-Agent browser/valid)"
        );
    }

    #[test]
    fn test_friendly_http_error_invalid_api_key() {
        // Arrange
        let body = r#"{"message":"You cannot consume this service. Your API key is invalid or deactivated."}"#;

        // Act
        let msg = friendly_http_error(401, body);

        // Assert
        assert_eq!(
            msg,
            "API key tidak valid atau tidak terdaftar di OpenSubtitles"
        );
    }

    #[test]
    fn test_friendly_http_error_missing_credentials() {
        // Arrange
        let body = r#"{"message":"Missing username and password in the request"}"#;

        // Act
        let msg = friendly_http_error(401, body);

        // Assert
        assert_eq!(msg, "Field username/password tidak terkirim dengan benar");
    }

    #[test]
    fn test_friendly_http_error_html_waf_page() {
        // Arrange
        let body = "<!DOCTYPE html><html><head><title>Attention Required! | Cloudflare</title></head><body>Request blocked</body></html>";

        // Act
        let msg = friendly_http_error(403, body);

        // Assert
        assert_eq!(
            msg,
            "Server mengembalikan halaman error (kemungkinan diblokir WAF/Cloudflare)"
        );
    }

    #[test]
    fn test_friendly_http_error_falls_back_to_snippet() {
        // Arrange
        let body = "some unknown error text";

        // Act
        let msg = friendly_http_error(500, body);

        // Assert
        assert_eq!(msg, body);
    }

    #[test]
    fn test_friendly_http_error_matching_is_case_insensitive() {
        // Arrange
        let body = r#"{"message":"Kong-User-Agent-Block"}"#;

        // Act
        let msg = friendly_http_error(403, body);

        // Assert
        assert_eq!(
            msg,
            "User-Agent diblokir gateway OpenSubtitles (perlu User-Agent browser/valid)"
        );
    }

    /// Spawn a minimal mock HTTP server that accepts `num_connections`
    /// connections and answers request `i` with `respond(i)`. Every response
    /// must include `Connection: close` so reqwest opens a fresh connection
    /// per request (required to observe retries). Returns the server handle
    /// and the bound address.
    fn spawn_mock_server(
        num_connections: usize,
        respond: impl Fn(usize) -> String + Send + 'static,
    ) -> (
        std::thread::JoinHandle<std::io::Result<Vec<String>>>,
        std::net::SocketAddr,
    ) {
        use std::io::{Read, Write};

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = std::thread::spawn(move || -> std::io::Result<Vec<String>> {
            listener.set_nonblocking(true)?;
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
            let mut requests = Vec::with_capacity(num_connections);
            let mut idx = 0usize;
            while idx < num_connections {
                let (mut stream, _) = loop {
                    match listener.accept() {
                        Ok(conn) => break conn,
                        // Tolerate transient accept errors (e.g. Windows
                        // WSAECONNABORTED/10053 when the client aborts a
                        // previous connection right before reconnecting) —
                        // retry until the deadline instead of failing the
                        // whole mock server.
                        Err(_) => {
                            if std::time::Instant::now() > deadline {
                                return Err(std::io::Error::new(
                                    std::io::ErrorKind::TimedOut,
                                    "mock server: no connection",
                                ));
                            }
                            std::thread::sleep(std::time::Duration::from_millis(20));
                        }
                    }
                };
                // Read the whole request (head + declared body) before
                // responding so the socket holds no unread data when
                // dropped: on Windows, dropping a socket with unread data
                // sends an RST that races the client's retry connection and
                // flakes the tests with `IncompleteMessage`/10053.
                stream.set_read_timeout(Some(std::time::Duration::from_secs(10)))?;
                let mut req = Vec::new();
                let mut chunk = [0u8; 8192];
                let mut need = None; // head end + Content-Length body bytes
                let n0 = stream.read(&mut chunk);
                if let Err(e) = n0 {
                    return Err(e);
                }
                let mut n = n0.unwrap();
                loop {
                    if n == 0 {
                        break;
                    }
                    req.extend_from_slice(&chunk[..n]);
                    if need.is_none()
                        && let Some(pos) =
                            req.windows(4).position(|w| w == b"\r\n\r\n")
                    {
                        let content_length = String::from_utf8_lossy(&req[..pos])
                            .lines()
                            .find_map(|line| {
                                let (k, v) = line.split_once(':')?;
                                k.eq_ignore_ascii_case("content-length").then(|| {
                                    v.trim().parse::<usize>().ok()
                                })?
                            })
                            .unwrap_or(0);
                        need = Some(pos + 4 + content_length);
                    }
                    if let Some(need) = need
                        && req.len() >= need
                    {
                        break;
                    }
                    let r = stream.read(&mut chunk);
                    match r {
                        Ok(0) => break,
                        Ok(k) => n = k,
                        Err(e) => return Err(e),
                    }
                }
                requests.push(String::from_utf8_lossy(&req).to_string());
                let response = respond(idx);
                stream.write_all(response.as_bytes())?;
                stream.flush()?;
                // Hold the socket open briefly after writing the response:
                // closing it immediately races the client's response read on
                // Windows and the connection can be aborted before the
                // headers arrive, surfacing as `IncompleteMessage` flakes.
                std::thread::sleep(std::time::Duration::from_millis(100));
                idx += 1;
            }
            Ok(requests)
        });
        (handle, addr)
    }

    #[tokio::test]
    async fn test_login_flow_retries_on_429() {
        // Arrange: first attempt is throttled, the retry succeeds.
        let (server, addr) = spawn_mock_server(2, |idx| {
            if idx == 0 {
                "HTTP/1.1 429 Too Many Requests\r\nRetry-After: 0\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_string()
            } else {
                let body = r#"{"user":{"token":"tok_retry"}}"#;
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                )
            }
        });
        let config = OpenSubtitlesConfig {
            api_key: Some("test_api_key".into()),
            username: Some("test_user".into()),
            password: Some("test_pass".into()),
            languages: vec!["id".into()],
            enabled: true,
            base_url: Some(format!("http://{}", addr)),
        };
        let client = OpenSubtitlesClient::new(config);

        // Act
        let token = client.ensure_token().await.unwrap();

        // Assert
        assert_eq!(token, "tok_retry");
        let requests = server.join().unwrap().unwrap();
        assert_eq!(
            requests.len(),
            2,
            "expected exactly one retry after the 429"
        );
        assert!(
            requests[0].starts_with("POST /login HTTP/1.1"),
            "got: {}",
            requests[0]
        );
        assert!(
            requests[1].starts_with("POST /login HTTP/1.1"),
            "got: {}",
            requests[1]
        );
    }

    #[tokio::test]
    async fn test_login_flow_returns_rate_limited_after_retry() {
        // Arrange: the gateway keeps answering 429 even after the retry.
        let (server, addr) = spawn_mock_server(2, |_| {
            "HTTP/1.1 429 Too Many Requests\r\nRetry-After: 0\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_string()
        });
        let config = OpenSubtitlesConfig {
            api_key: Some("test_api_key".into()),
            username: Some("test_user".into()),
            password: Some("test_pass".into()),
            languages: vec!["id".into()],
            enabled: true,
            base_url: Some(format!("http://{}", addr)),
        };
        let client = OpenSubtitlesClient::new(config);

        // Act
        let err = client.ensure_token().await.unwrap_err();

        // Assert
        match err {
            OpenSubtitlesError::RateLimited(retry_after) => assert_eq!(retry_after, 0),
            other => panic!("expected RateLimited, got {other:?}"),
        }
        let requests = server.join().unwrap().unwrap();
        assert_eq!(requests.len(), 2, "expected one retry then RateLimited");
    }

    #[tokio::test]
    async fn test_fetch_bytes_rejects_oversized_response() {
        // Regression: a response declaring a >50 MB body must be rejected
        // before any buffering, not exhaust memory.
        let (server, addr) = spawn_mock_server(1, |_| {
            "HTTP/1.1 200 OK\r\nContent-Length: 62914560\r\nConnection: close\r\n\r\n".to_string()
        });
        let config = OpenSubtitlesConfig {
            api_key: Some("test_api_key".into()),
            username: Some("test_user".into()),
            password: Some("test_pass".into()),
            languages: vec!["id".into()],
            enabled: true,
            base_url: Some(format!("http://{}", addr)),
        };
        let client = OpenSubtitlesClient::new(config);

        let err = client
            .fetch_bytes(&format!("http://{addr}/file.srt"))
            .await
            .unwrap_err();
        assert!(
            matches!(err, OpenSubtitlesError::BodyTooLarge(_)),
            "expected BodyTooLarge, got {err:?}"
        );
        let _ = server.join();
    }

    #[tokio::test]
    async fn test_fetch_bytes_retries_on_429() {
        // Regression: a single 429 from the download host must be retried once
        // (honouring Retry-After), not fail the resolve outright.
        let (server, addr) = spawn_mock_server(2, |idx| {
            if idx == 0 {
                "HTTP/1.1 429 Too Many Requests\r\nRetry-After: 0\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_string()
            } else {
                let body = b"1\n00:00:01,000 --> 00:00:02,000\nTest\n";
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    String::from_utf8_lossy(body)
                )
            }
        });
        let config = OpenSubtitlesConfig {
            api_key: Some("test_api_key".into()),
            username: Some("test_user".into()),
            password: Some("test_pass".into()),
            languages: vec!["id".into()],
            enabled: true,
            base_url: Some(format!("http://{}", addr)),
        };
        let client = OpenSubtitlesClient::new(config);

        let bytes = client
            .fetch_bytes(&format!("http://{addr}/file.srt"))
            .await
            .unwrap();
        assert!(bytes.starts_with(b"1\n00:00:01,000"), "got: {bytes:?}");
        let requests = server.join().unwrap().unwrap();
        assert_eq!(
            requests.len(),
            2,
            "expected exactly one retry after the 429"
        );
    }
}
