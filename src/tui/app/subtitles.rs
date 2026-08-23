use crate::providers::models::ProviderKind;
use crate::providers::subtitles::{OsCandidate, SubtitleContext};
use crate::tui::action::Action;
use crate::tui::overlay::NotificationKind;

use super::App;

/// Parse an OpenSubtitles cache marker (`os:{file_id}:{lang}`) into its parts.
/// Returns `None` when the marker is not a valid OS marker.
pub(crate) fn parse_os_marker(marker: &str) -> Option<(u32, String)> {
    let rest = marker.strip_prefix("os:")?;
    let mut parts = rest.splitn(2, ':');
    let file_id = parts.next()?.parse::<u32>().ok()?;
    let lang = parts.next().unwrap_or("id").to_string();
    Some((file_id, lang))
}

/// Parse a SubDL cache marker (`subdl:{n_id}:{lang}`) into its parts.
/// Returns `None` when the marker is not a valid SubDL marker; the language
/// part defaults to `"id"` when missing.
pub(crate) fn parse_subdl_marker(marker: &str) -> Option<(String, String)> {
    let rest = marker.strip_prefix("subdl:")?;
    let mut parts = rest.splitn(2, ':');
    let subdl_id = parts.next()?.to_string();
    if subdl_id.is_empty() {
        return None;
    }
    let lang = parts.next().unwrap_or("id").to_string();
    Some((subdl_id, lang))
}

/// Pick the best OpenSubtitles candidate: the first Indonesian one, or the
/// top-ranked candidate when no Indonesian subtitle exists.
pub(crate) fn pick_best_os_candidate(candidates: &[OsCandidate]) -> Option<&OsCandidate> {
    candidates
        .iter()
        .find(|c| {
            c.language.eq_ignore_ascii_case("id") || c.language.eq_ignore_ascii_case("indonesian")
        })
        .or_else(|| candidates.first())
}

/// Find the caption URL whose popup label (`lanName`) equals the stored
/// season-subtitle preference. `ext_captions` is the raw `get-ext-captions`
/// response object (`{ "extCaptions": [...] }`). Returns `None` when nothing
/// matches so the episode downloads without a subtitle (existing default).
pub(crate) fn match_season_subtitle_preference(
    ext_captions: &serde_json::Value,
    preference: &str,
) -> Option<String> {
    let captions = ext_captions.get("extCaptions").and_then(|c| c.as_array())?;
    for cap in captions {
        let name = cap.get("lanName").and_then(|n| n.as_str()).unwrap_or("");
        let url = cap.get("url").and_then(|u| u.as_str()).unwrap_or("");
        if !url.is_empty() && name == preference {
            return Some(url.to_string());
        }
    }
    None
}

/// Heuristic guard against caching non-subtitle content (e.g. an HTML error
/// page or a JSON error body from a proxy/CDN) as a 30-day cache entry.
/// Real SRT/VTT/ASS files start with a line number, `WEBVTT`, or
/// `[Script Info]`; anything else that is non-empty and not HTML/JSON is
/// accepted as a subtitle.
pub(crate) fn looks_like_subtitle(bytes: &[u8]) -> bool {
    if bytes.is_empty() {
        return false;
    }
    // Only the top of the file matters; subtitle markers live there.
    let head = &bytes[..bytes.len().min(2048)];
    let lower = head.to_ascii_lowercase();
    // Reject HTML error/challenge pages.
    for marker in ["<!doctype", "<html", "<head", "<body"] {
        if lower.windows(marker.len()).any(|w| w == marker.as_bytes()) {
            return false;
        }
    }
    // Reject JSON error bodies (e.g. `{"error": "..."}`).
    if let Some(start) = head.iter().position(|b| !b.is_ascii_whitespace()) {
        if head[start] == b'{' {
            return false;
        }
    }
    // Reject ZIP/GZIP bodies: OpenSubtitles `/download` can return zipped
    // subtitles. Caching those under a `.srt` name would feed garbage to the
    // player, so let the fallback chain try the next provider instead.
    if head.starts_with(b"PK\x03\x04") || head.starts_with(&[0x1F, 0x8B]) {
        return false;
    }
    true
}

/// Cap on a remote subtitle body downloaded to a temp file for VLC/IINA
/// (~50 MB), mirroring `fetch_capped_image`: chunked, so a misbehaving CDN
/// cannot stream the whole body into RAM before rejection.
const MAX_SUBTITLE_BYTES: usize = 50 * 1024 * 1024;

/// Fetch a remote URL with a `MAX_SUBTITLE_BYTES` cap. Returns `None` when
/// the request fails or the body exceeds the cap.
pub(crate) async fn fetch_capped_bytes(url: &str) -> Option<Vec<u8>> {
    let mut resp = reqwest::get(url).await.ok()?;
    if let Some(len) = resp.content_length()
        && len > MAX_SUBTITLE_BYTES as u64
    {
        return None;
    }
    let mut buf = Vec::new();
    while let Some(chunk) = resp.chunk().await.ok().flatten() {
        if buf.len() as u64 + chunk.len() as u64 > MAX_SUBTITLE_BYTES as u64 {
            return None;
        }
        buf.extend_from_slice(&chunk);
    }
    Some(buf)
}

/// Resolve an OpenSubtitles file to a local cache path (checking the cache
/// first, otherwise downloading). Returns `Err(reason)` when the subtitle
/// could not be resolved so callers can surface the reason to the user;
/// playback still degrades gracefully when the caller ignores the error.
pub(crate) async fn resolve_os_subtitle_to_cache(
    provider: ProviderKind,
    subject_id: &str,
    season: usize,
    episode: usize,
    file_id: u32,
    lang: &str,
) -> Result<String, String> {
    let os = crate::providers::subtitles::opensubtitles::OpenSubtitlesClient::from_env();
    let target_path = crate::providers::subtitles::cache::subtitle_path(
        provider, subject_id, season, episode, file_id, lang, "srt",
    );
    if let Some(cached) = crate::providers::subtitles::cache::get_cached_subtitle_path(&target_path)
    {
        return Ok(cached.to_string_lossy().to_string());
    }
    let dl = os
        .download_link(file_id)
        .await
        .map_err(|e| format!("OpenSubtitles download failed: {e}"))?;
    let ext = crate::providers::subtitles::cache::subtitle_extension(dl.file_name.as_deref());
    let target_path = crate::providers::subtitles::cache::subtitle_path(
        provider, subject_id, season, episode, file_id, lang, ext,
    );
    let bytes = os
        .fetch_bytes(&dl.link)
        .await
        .map_err(|e| format!("Subtitle download failed: {e}"))?;
    if !looks_like_subtitle(&bytes) {
        return Err(
            "Subtitle download failed: response is not subtitle content (HTML/JSON error page)"
                .to_string(),
        );
    }
    if let Err(e) = crate::providers::subtitles::cache::set_subtitle_cache(&target_path, &bytes) {
        return Err(format!("Failed to write subtitle cache: {e}"));
    }
    Ok(target_path.to_string_lossy().to_string())
}

/// Resolve a SubDL subtitle (`n_id`) to a local cache path (checking the
/// cache first, otherwise downloading the anonymous `dl.subdl.com` archive).
/// Returns `Err(reason)` when the subtitle could not be resolved so callers
/// can surface the reason to the user; playback still degrades gracefully
/// when the caller ignores the error.
pub(crate) async fn resolve_subdl_subtitle_to_cache(
    provider: ProviderKind,
    subject_id: &str,
    season: usize,
    episode: usize,
    subdl_id: &str,
    lang: &str,
) -> Result<String, String> {
    let subdl = crate::providers::subtitles::subdl::SubdlClient::from_env();
    let target_path = crate::providers::subtitles::cache::subdl_subtitle_path(
        provider, subject_id, season, episode, subdl_id, lang, "srt",
    );
    if let Some(cached) = crate::providers::subtitles::cache::get_cached_subtitle_path(&target_path)
    {
        return Ok(cached.to_string_lossy().to_string());
    }
    let (ext, bytes) = subdl
        .download_bytes(subdl_id)
        .await
        .map_err(|e| format!("SubDL download failed: {e}"))?;
    if !looks_like_subtitle(&bytes) {
        return Err(
            "SubDL download failed: response is not subtitle content (HTML/JSON error page)"
                .to_string(),
        );
    }
    let target_path = crate::providers::subtitles::cache::subdl_subtitle_path(
        provider, subject_id, season, episode, subdl_id, lang, &ext,
    );
    if let Err(e) = crate::providers::subtitles::cache::set_subtitle_cache(&target_path, &bytes) {
        return Err(format!("Failed to write subtitle cache: {e}"));
    }
    Ok(target_path.to_string_lossy().to_string())
}

/// Search for the best subtitle and resolve it to a local cache path.
/// SubDL is tried first (free, never consumes the OpenSubtitles quota); when
/// it is disabled, fails or finds nothing, OpenSubtitles is used as the
/// fallback. Returns `None` when no candidate is found or any step fails.
pub(crate) async fn resolve_best_os_subtitle(ctx: &SubtitleContext) -> Option<String> {
    // Step 1: SubDL (gratis, tidak konsumsi kuota OS).
    let subdl = crate::providers::subtitles::subdl::SubdlClient::from_env();
    if subdl.enabled() {
        if let Ok(outcome) = subdl.search(ctx).await {
            // Candidates are already sorted by score (Indonesian first).
            if let Some(best) = outcome.candidates.first() {
                if let Ok(path) = resolve_subdl_subtitle_to_cache(
                    ctx.provider,
                    &ctx.subject_id,
                    ctx.season.unwrap_or(0),
                    ctx.episode.unwrap_or(0),
                    &best.subtitle_id,
                    &best.language,
                )
                .await
                {
                    return Some(path);
                }
            }
        }
    }
    // Step 2: OpenSubtitles (existing behaviour).
    let os = crate::providers::subtitles::opensubtitles::OpenSubtitlesClient::from_env();
    let outcome = os.search(ctx).await.ok()?;
    let best = pick_best_os_candidate(&outcome.candidates)?;
    resolve_os_subtitle_to_cache(
        ctx.provider,
        &ctx.subject_id,
        ctx.season.unwrap_or(0),
        ctx.episode.unwrap_or(0),
        best.file_id,
        &best.language,
    )
    .await
    .ok()
}

impl App {
    pub(crate) fn build_subtitle_context(&self) -> Option<SubtitleContext> {
        let details = self.state.selected_details.as_ref()?;
        let title = details
            .get("title")
            .and_then(|t| t.as_str())
            .map(crate::providers::moviebox::clean_moviebox_title)
            .unwrap_or_default();
        if title.is_empty() {
            return None;
        }
        let subject_id = self.state.active_subject_id.clone().unwrap_or_default();
        // Prefer the currently selected resource, falling back to the details payload.
        let resource_id = self
            .state
            .selected_resources
            .as_ref()
            .and_then(|res| res.get("list"))
            .and_then(|l| l.as_array())
            .and_then(|list| {
                let idx = self.state.resource_list_state.selected().unwrap_or(0);
                list.get(idx)
            })
            .and_then(|file| file.get("resourceId"))
            .and_then(|r| r.as_str())
            .map(str::to_string)
            .or_else(|| {
                details
                    .get("resourceId")
                    .or_else(|| details.get("resource_id"))
                    .and_then(|r| r.as_str())
                    .map(str::to_string)
            })
            .unwrap_or_default();
        let is_episode = details
            .get("subjectType")
            .or_else(|| details.get("stype"))
            .and_then(|v| v.as_i64())
            .is_some_and(|t| t == 2);
        let year = details
            .get("releaseDate")
            .and_then(|y| y.as_str())
            .or_else(|| details.get("year").and_then(|y| y.as_str()))
            .and_then(|s| {
                let digits: String = s.chars().filter(|c| c.is_ascii_digit()).take(4).collect();
                if digits.len() == 4 {
                    Some(digits)
                } else {
                    None
                }
            })
            .or_else(|| {
                details
                    .get("year")
                    .and_then(|y| y.as_u64())
                    .map(|y| y.to_string())
            });
        let imdb_id = crate::providers::subtitles::extract_imdb_id(details);
        let (season, episode) = if is_episode {
            (
                Some(self.state.selected_season),
                Some(self.state.selected_episode),
            )
        } else {
            (None, None)
        };
        Some(SubtitleContext {
            provider: self.state.active_provider,
            subject_id,
            resource_id,
            title,
            year,
            is_episode,
            season,
            episode,
            imdb_id,
        })
    }

    pub(crate) async fn handle_subtitles(&mut self, action: Action) -> Option<()> {
        match action {
            Action::OpenSubtitlesReady {
                context_id,
                candidates,
                is_download,
            } => {
                self.state.subtitle_searching = false;
                // P5: strict equality guard. Discard results that no longer
                // match the content the user is currently viewing.
                let current_ctx = format!(
                    "{}:{}:{}",
                    self.state.active_subject_id.clone().unwrap_or_default(),
                    self.state
                        .selected_resources
                        .as_ref()
                        .and_then(|res| res.get("list"))
                        .and_then(|l| l.as_array())
                        .and_then(|list| {
                            let idx = self.state.resource_list_state.selected().unwrap_or(0);
                            list.get(idx)
                        })
                        .and_then(|file| file.get("resourceId"))
                        .and_then(|r| r.as_str())
                        .unwrap_or(""),
                    self.state.selected_episode
                );
                if current_ctx != context_id {
                    self.state.os_waiting = false;
                    return None;
                }

                if self.state.os_waiting {
                    // The play/download decision was deferred until the search
                    // finished. Resume it now.
                    self.state.os_waiting = false;
                    if candidates.is_empty() {
                        if is_download {
                            self.action_sender.send(Action::DownloadStream(None)).ok();
                        } else if let Some(link) = self.state.pending_play_link.take() {
                            let open_with = self.state.pending_open_with;
                            if open_with {
                                self.action_sender
                                    .send(Action::ShowPlayerPicker(link, None))
                                    .ok();
                            } else {
                                self.action_sender.send(Action::LaunchMpv(link, None)).ok();
                            }
                        }
                    } else {
                        // P4: reset the list to ["None"] + OS candidates, then
                        // show the subtitle popup.
                        self.state.os_subtitles = candidates.clone();
                        let mut list = vec![("None".to_string(), "".to_string())];
                        for (label, marker) in candidates {
                            if !list.iter().any(|(l, _)| l == &label) {
                                list.push((label, marker));
                            }
                        }
                        self.state.subtitle_list = list;
                        self.state.subtitle_list_state.select(Some(0));
                        self.state.show_help = false;
                        self.state.player_picker_popup = false;
                        if is_download {
                            self.state.subtitle_popup = false;
                            self.state.is_download_subtitle_popup = true;
                            self.state.pending_play_link = None;
                        } else {
                            self.state.is_download_subtitle_popup = false;
                            self.state.subtitle_popup = true;
                        }
                    }
                } else {
                    // A MovieBox subtitle popup is already open; append the OS
                    // candidates to it. If the popup was dismissed in the
                    // meantime (Escape), do NOT reopen it — that was the
                    // "late popup" race.
                    let popup_open = if is_download {
                        self.state.is_download_subtitle_popup
                    } else {
                        self.state.subtitle_popup
                    };
                    if !popup_open {
                        return None;
                    }
                    self.state.os_subtitles = candidates.clone();
                    for (label, marker) in candidates {
                        if !self.state.subtitle_list.iter().any(|(l, _)| l == &label) {
                            self.state.subtitle_list.push((label, marker));
                        }
                    }
                }
            }
            Action::OpenSubtitlesFailed {
                context_id,
                is_download,
                error,
            } => {
                self.state.subtitle_searching = false;
                let current_ctx = format!(
                    "{}:{}:{}",
                    self.state.active_subject_id.clone().unwrap_or_default(),
                    self.state
                        .selected_resources
                        .as_ref()
                        .and_then(|res| res.get("list"))
                        .and_then(|l| l.as_array())
                        .and_then(|list| {
                            let idx = self.state.resource_list_state.selected().unwrap_or(0);
                            list.get(idx)
                        })
                        .and_then(|file| file.get("resourceId"))
                        .and_then(|r| r.as_str())
                        .unwrap_or(""),
                    self.state.selected_episode
                );
                if current_ctx != context_id {
                    // Stale failure from a previous search — ignore it.
                    return None;
                }
                self.state.subtitle_search_error = Some(error.clone());
                if self.state.os_waiting {
                    // Resume the deferred play/download without subtitles.
                    self.state.os_waiting = false;
                    if is_download {
                        self.action_sender.send(Action::DownloadStream(None)).ok();
                    } else if let Some(link) = self.state.pending_play_link.take() {
                        let open_with = self.state.pending_open_with;
                        if open_with {
                            self.action_sender
                                .send(Action::ShowPlayerPicker(link, None))
                                .ok();
                        } else {
                            self.action_sender.send(Action::LaunchMpv(link, None)).ok();
                        }
                    }
                }
                self.state.notify(
                    NotificationKind::Warning,
                    "Subtitles unavailable",
                    format!("OpenSubtitles: {error}"),
                );
            }
            Action::SubdlReady {
                context_id,
                candidates,
                is_download,
            } => {
                self.state.subtitle_searching = false;
                // P5: strict equality guard. Discard results that no longer
                // match the content the user is currently viewing.
                let current_ctx = format!(
                    "{}:{}:{}",
                    self.state.active_subject_id.clone().unwrap_or_default(),
                    self.state
                        .selected_resources
                        .as_ref()
                        .and_then(|res| res.get("list"))
                        .and_then(|l| l.as_array())
                        .and_then(|list| {
                            let idx = self.state.resource_list_state.selected().unwrap_or(0);
                            list.get(idx)
                        })
                        .and_then(|file| file.get("resourceId"))
                        .and_then(|r| r.as_str())
                        .unwrap_or(""),
                    self.state.selected_episode
                );
                if current_ctx != context_id {
                    self.state.os_waiting = false;
                    return None;
                }

                if self.state.os_waiting {
                    // The play/download decision was deferred until the search
                    // finished. Resume it now.
                    self.state.os_waiting = false;
                    if candidates.is_empty() {
                        if is_download {
                            self.action_sender.send(Action::DownloadStream(None)).ok();
                        } else if let Some(link) = self.state.pending_play_link.take() {
                            let open_with = self.state.pending_open_with;
                            if open_with {
                                self.action_sender
                                    .send(Action::ShowPlayerPicker(link, None))
                                    .ok();
                            } else {
                                self.action_sender.send(Action::LaunchMpv(link, None)).ok();
                            }
                        }
                    } else {
                        // P4: reset the list to ["None"] + SubDL candidates,
                        // then show the subtitle popup.
                        self.state.os_subtitles = candidates.clone();
                        let mut list = vec![("None".to_string(), "".to_string())];
                        for (label, marker) in candidates {
                            if !list.iter().any(|(l, _)| l == &label) {
                                list.push((label, marker));
                            }
                        }
                        self.state.subtitle_list = list;
                        self.state.subtitle_list_state.select(Some(0));
                        self.state.show_help = false;
                        self.state.player_picker_popup = false;
                        if is_download {
                            self.state.subtitle_popup = false;
                            self.state.is_download_subtitle_popup = true;
                            self.state.pending_play_link = None;
                        } else {
                            self.state.is_download_subtitle_popup = false;
                            self.state.subtitle_popup = true;
                        }
                    }
                } else {
                    // A MovieBox subtitle popup is already open; append the
                    // SubDL candidates to it. If the popup was dismissed in
                    // the meantime (Escape), do NOT reopen it — that was the
                    // "late popup" race.
                    let popup_open = if is_download {
                        self.state.is_download_subtitle_popup
                    } else {
                        self.state.subtitle_popup
                    };
                    if !popup_open {
                        return None;
                    }
                    self.state.os_subtitles = candidates.clone();
                    for (label, marker) in candidates {
                        if !self.state.subtitle_list.iter().any(|(l, _)| l == &label) {
                            self.state.subtitle_list.push((label, marker));
                        }
                    }
                }
            }
            Action::SubdlFailed {
                context_id,
                is_download,
                error,
            } => {
                self.state.subtitle_searching = false;
                let current_ctx = format!(
                    "{}:{}:{}",
                    self.state.active_subject_id.clone().unwrap_or_default(),
                    self.state
                        .selected_resources
                        .as_ref()
                        .and_then(|res| res.get("list"))
                        .and_then(|l| l.as_array())
                        .and_then(|list| {
                            let idx = self.state.resource_list_state.selected().unwrap_or(0);
                            list.get(idx)
                        })
                        .and_then(|file| file.get("resourceId"))
                        .and_then(|r| r.as_str())
                        .unwrap_or(""),
                    self.state.selected_episode
                );
                if current_ctx != context_id {
                    // Stale failure from a previous search — ignore it.
                    return None;
                }
                self.state.subtitle_search_error = Some(error.clone());
                if self.state.os_waiting {
                    // Resume the deferred play/download without subtitles.
                    self.state.os_waiting = false;
                    if is_download {
                        self.action_sender.send(Action::DownloadStream(None)).ok();
                    } else if let Some(link) = self.state.pending_play_link.take() {
                        let open_with = self.state.pending_open_with;
                        if open_with {
                            self.action_sender
                                .send(Action::ShowPlayerPicker(link, None))
                                .ok();
                        } else {
                            self.action_sender.send(Action::LaunchMpv(link, None)).ok();
                        }
                    }
                }
                self.state.notify(
                    NotificationKind::Warning,
                    "Subtitles unavailable",
                    format!("SubDL: {error}"),
                );
            }
            _ => return None,
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_os_marker() {
        assert_eq!(
            parse_os_marker("os:5091684:id"),
            Some((5091684, "id".to_string()))
        );
        assert_eq!(parse_os_marker("os:123"), Some((123, "id".to_string())));
        assert_eq!(parse_os_marker("os:abc"), None);
        assert_eq!(parse_os_marker("http://example.com/sub.srt"), None);
        assert_eq!(parse_os_marker(""), None);
    }

    #[test]
    fn test_parse_subdl_marker() {
        assert_eq!(
            parse_subdl_marker("subdl:3197651-3213944:id"),
            Some(("3197651-3213944".to_string(), "id".to_string()))
        );
        assert_eq!(
            parse_subdl_marker("subdl:3197651-3213944"),
            Some(("3197651-3213944".to_string(), "id".to_string()))
        );
        assert_eq!(
            parse_subdl_marker("subdl:3197651-3213944:en"),
            Some(("3197651-3213944".to_string(), "en".to_string()))
        );
        assert_eq!(parse_subdl_marker("subdl:"), None);
        assert_eq!(parse_subdl_marker("os:123:id"), None);
        assert_eq!(parse_subdl_marker("http://example.com/sub.srt"), None);
        assert_eq!(parse_subdl_marker(""), None);
    }

    #[test]
    fn test_pick_best_os_candidate_prefers_id() {
        let en = OsCandidate {
            label: String::new(),
            file_id: 1,
            language: "en".into(),
            score: 100,
            release_name: None,
            download_count: None,
            machine_translated: false,
        };
        let id = OsCandidate {
            label: String::new(),
            file_id: 2,
            language: "id".into(),
            score: 60,
            release_name: None,
            download_count: None,
            machine_translated: false,
        };
        let mixed = vec![en.clone(), id.clone()];
        assert_eq!(pick_best_os_candidate(&mixed).map(|c| c.file_id), Some(2));
        let no_id = vec![en.clone()];
        assert_eq!(pick_best_os_candidate(&no_id).map(|c| c.file_id), Some(1));
        assert_eq!(pick_best_os_candidate(&[]), None);
    }

    #[test]
    fn test_looks_like_subtitle() {
        assert!(looks_like_subtitle(
            b"1\n00:00:01,000 --> 00:00:04,000\nHello\n"
        ));
        assert!(looks_like_subtitle(
            b"WEBVTT\n\n00:00:01.000 --> 00:00:04.000\nHello\n"
        ));
        assert!(looks_like_subtitle(b"[Script Info]\nTitle: Test\n"));
        assert!(looks_like_subtitle(b"plain text subtitle-ish content"));
        assert!(!looks_like_subtitle(b""));
        assert!(!looks_like_subtitle(
            b"<!DOCTYPE html><html><body>Error</body></html>"
        ));
        assert!(!looks_like_subtitle(
            b"<html>\n<head><title>403</title></head></html>"
        ));
        assert!(!looks_like_subtitle(b"{\"error\":\"rate limit exceeded\"}"));
        assert!(!looks_like_subtitle(b"  {\"error\":\"forbidden\"}"));
        assert!(!looks_like_subtitle(b"PK\x03\x04\x14\x00\x00\x00\x08\x00"));
        assert!(!looks_like_subtitle(b"PK\x03\x04rest of zip archive..."));
        assert!(!looks_like_subtitle(b"\x1f\x8b\x08\x00\x00\x00\x00\x00"));
        assert!(looks_like_subtitle(
            b"1\n00:00:01,000 --> 00:00:04,000\nPK zip-like text\n"
        ));
    }

    #[test]
    fn test_match_season_subtitle_preference() {
        let ext = serde_json::json!({
            "extCaptions": [
                { "lanName": "English", "url": "https://cdn/x/en.vtt" },
                { "lanName": "Bahasa Indonesia", "url": "https://cdn/x/id.vtt" },
                { "lanName": "Chinese", "url": "" }
            ]
        });
        assert_eq!(
            match_season_subtitle_preference(&ext, "Bahasa Indonesia"),
            Some("https://cdn/x/id.vtt".to_string())
        );
        assert_eq!(match_season_subtitle_preference(&ext, "id"), None);
        assert_eq!(match_season_subtitle_preference(&ext, "Chinese"), None);
        assert_eq!(match_season_subtitle_preference(&ext, "Korean"), None);
        assert_eq!(
            match_season_subtitle_preference(&serde_json::json!({}), "English"),
            None
        );
        assert_eq!(
            match_season_subtitle_preference(&serde_json::json!({ "extCaptions": [] }), "English"),
            None
        );
    }
}
