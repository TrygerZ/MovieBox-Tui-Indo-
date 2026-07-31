//! Embedded-subtitle detection for 4KHDHub streams.
//!
//! MKV/MP4 streams often carry subtitle tracks baked into the container. mpv
//! already auto-loads those tracks, so this module only *informs* the user
//! that embedded subtitles may exist (press `j` in mpv to cycle tracks).
//! Playback behavior is never changed.
//!
//! Probing the actual tracks with `ffprobe` is optional and gated behind the
//! `MOVIEBOX_PROBE_EMBEDDED_SUBTITLES` env var (default `false`). When
//! disabled, only a cheap, extension-based hint is produced.

use std::time::Duration;

use serde::Deserialize;

/// Containers known to support embedded subtitle tracks.
const EMBEDDED_SUB_CONTAINERS: &[&str] = &[
    "mkv", "mk3d", "mka", "webm", "mp4", "m4v", "mov", "ts", "m2ts", "mts", "avi", "flv",
];

/// Env var that enables the optional ffprobe probe.
const PROBE_ENV: &str = "MOVIEBOX_PROBE_EMBEDDED_SUBTITLES";

/// Whether the URL's container can carry embedded subtitles.
///
/// The path is extracted from the URL (query `?` and fragment `#` are
/// dropped), and the extension is matched case-insensitively. HLS playlists
/// (`.m3u8`) return `false` because their subtitle tracks are separate.
pub fn container_supports_embedded_subs(url: &str) -> bool {
    let path = url.split(['?', '#']).next().unwrap_or(url);
    let filename = path.rsplit('/').next().unwrap_or(path);
    let Some((_, ext)) = filename.rsplit_once('.') else {
        return false;
    };
    let ext = ext.to_ascii_lowercase();
    if ext.is_empty() || ext == "m3u8" {
        return false;
    }
    EMBEDDED_SUB_CONTAINERS.contains(&ext.as_str())
}

/// Whether the optional ffprobe probe is enabled via env.
pub fn probe_enabled() -> bool {
    std::env::var(PROBE_ENV)
        .map(|value| value.eq_ignore_ascii_case("true") || value == "1")
        .unwrap_or(false)
}

/// A single subtitle track discovered inside the container.
#[derive(Debug, Clone, PartialEq)]
pub struct ProbeTrack {
    pub index: i64,
    pub language: Option<String>,
    pub codec: Option<String>,
}

/// Result of probing a stream for embedded subtitle tracks.
#[derive(Debug, Clone, PartialEq)]
pub struct ProbeReport {
    pub tracks: Vec<ProbeTrack>,
}

/// Probe the stream for embedded subtitle tracks with `ffprobe`.
///
/// Only runs when `probe_enabled()` is true and `ffprobe` is on the PATH.
/// Runs inside `spawn_blocking` with an 8s timeout so a hung ffprobe never
/// blocks the TUI. Any failure (missing binary, non-zero exit, bad JSON,
/// timeout) resolves to `None`.
pub async fn probe_embedded_subtitle_tracks(
    url: &str,
    headers: &[(String, String)],
) -> Option<ProbeReport> {
    if !probe_enabled() {
        return None;
    }
    let url = url.to_string();
    let headers = headers.to_vec();
    let handle = tokio::task::spawn_blocking(move || run_ffprobe(&url, &headers));
    match tokio::time::timeout(Duration::from_secs(8), handle).await {
        Ok(Ok(report)) => report,
        _ => None,
    }
}

/// Build a human-readable embedded-subtitle notice.
///
/// - With a probe report containing tracks: reports the count and the unique
///   languages found (e.g. `id, en`), empty when languages are unknown.
/// - Without a report but with a likely container: generic extension-based
///   hint.
/// - Otherwise: an empty string (no notification).
pub fn format_embedded_notice(likely: bool, report: Option<&ProbeReport>) -> String {
    if let Some(report) = report
        && !report.tracks.is_empty()
    {
        let mut languages: Vec<&str> = Vec::new();
        for track in &report.tracks {
            if let Some(lang) = track.language.as_deref()
                && !lang.is_empty()
                && !languages.contains(&lang)
            {
                languages.push(lang);
            }
        }
        let example = if languages.is_empty() {
            String::new()
        } else {
            format!(" (e.g. {})", languages.join(", "))
        };
        return format!(
            "Found {} embedded subtitle track(s){example}. If the player doesn't show one, press 'j' (mpv) to cycle tracks.",
            report.tracks.len()
        );
    }
    if likely {
        return "This stream is an MKV/MP4 and may contain embedded subtitles. If the player doesn't show one, press 'j' (mpv) to cycle tracks."
            .to_string();
    }
    String::new()
}

/// Run ffprobe synchronously (blocking) and return the parsed report.
fn run_ffprobe(url: &str, headers: &[(String, String)]) -> Option<ProbeReport> {
    if !command_exists("ffprobe") {
        return None;
    }
    let mut command = std::process::Command::new("ffprobe");
    command.args([
        "-v",
        "error",
        "-print_format",
        "json",
        "-show_entries",
        "stream=index:stream_tags=language:codec_name",
        "-select_streams",
        "s",
    ]);
    if !headers.is_empty() {
        let joined = headers
            .iter()
            .map(|(name, value)| format!("{name}: {value}\r\n"))
            .collect::<String>();
        command.arg("-headers").arg(joined);
    }
    command.arg(url);
    let output = command.output().ok()?;
    if !output.status.success() {
        return None;
    }
    parse_probe_output(&output.stdout)
}

/// Parse ffprobe's JSON output into a [`ProbeReport`].
///
/// Non-subtitle streams (when present, e.g. `codec_type != "subtitle"`) are
/// ignored defensively; missing fields fall back to defaults.
fn parse_probe_output(bytes: &[u8]) -> Option<ProbeReport> {
    let parsed: FfprobeOutput = serde_json::from_slice(bytes).ok()?;
    let tracks = parsed
        .streams
        .into_iter()
        .filter(|stream| {
            stream
                .codec_type
                .as_deref()
                .is_none_or(|codec_type| codec_type == "subtitle")
        })
        .map(|stream| ProbeTrack {
            index: stream.index,
            language: stream.tags.and_then(|tags| tags.language),
            codec: stream.codec_name,
        })
        .collect();
    Some(ProbeReport { tracks })
}

/// Cross-platform "is this binary on the PATH?" check (mirrors player.rs).
fn command_exists(command: &str) -> bool {
    let finder = if cfg!(target_os = "windows") {
        "where"
    } else {
        "which"
    };
    std::process::Command::new(finder)
        .arg(command)
        .output()
        .is_ok_and(|output| output.status.success())
}

#[derive(Debug, Deserialize)]
struct FfprobeOutput {
    #[serde(default)]
    streams: Vec<FfprobeStream>,
}

#[derive(Debug, Deserialize)]
struct FfprobeStream {
    #[serde(default)]
    index: i64,
    #[serde(default)]
    codec_name: Option<String>,
    #[serde(default)]
    codec_type: Option<String>,
    #[serde(default)]
    tags: Option<StreamTags>,
}

#[derive(Debug, Deserialize)]
struct StreamTags {
    #[serde(default)]
    language: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn container_supports_common_containers() {
        assert!(container_supports_embedded_subs(
            "https://cdn.example.com/movie.mkv"
        ));
        assert!(container_supports_embedded_subs(
            "https://cdn.example.com/movie.mp4"
        ));
        assert!(container_supports_embedded_subs(
            "https://cdn.example.com/live.ts"
        ));
        assert!(container_supports_embedded_subs(
            "https://cdn.example.com/movie.webm"
        ));
        assert!(container_supports_embedded_subs("movie.mkv"));
    }

    #[test]
    fn container_rejects_non_subtitle_files() {
        assert!(!container_supports_embedded_subs(
            "https://cdn.example.com/poster.jpg"
        ));
        assert!(!container_supports_embedded_subs(
            "https://cdn.example.com/stream.m3u8"
        ));
        assert!(!container_supports_embedded_subs(
            "https://cdn.example.com/noextension"
        ));
        assert!(!container_supports_embedded_subs(""));
        assert!(!container_supports_embedded_subs(
            "https://cdn.example.com/trailing/"
        ));
    }

    #[test]
    fn container_ignores_query_and_fragment() {
        assert!(container_supports_embedded_subs(
            "https://cdn.example.com/movie.mkv?token=abc123"
        ));
        assert!(container_supports_embedded_subs(
            "https://cdn.example.com/movie.mkv#fragment"
        ));
        assert!(!container_supports_embedded_subs(
            "https://cdn.example.com/poster.jpg?token=abc123"
        ));
        assert!(!container_supports_embedded_subs(
            "https://cdn.example.com/stream.m3u8?token=abc123"
        ));
    }

    #[test]
    fn container_is_case_insensitive() {
        assert!(container_supports_embedded_subs(
            "https://cdn.example.com/movie.MKV"
        ));
        assert!(container_supports_embedded_subs(
            "https://cdn.example.com/movie.Mp4"
        ));
        assert!(!container_supports_embedded_subs(
            "https://cdn.example.com/poster.JPG"
        ));
        assert!(!container_supports_embedded_subs(
            "https://cdn.example.com/stream.M3U8"
        ));
    }

    #[test]
    fn parse_probe_output_keeps_only_subtitle_streams() {
        let output = serde_json::json!({
            "streams": [
                {
                    "index": 0,
                    "codec_name": "subrip",
                    "codec_type": "subtitle",
                    "tags": { "language": "id" }
                },
                {
                    "index": 1,
                    "codec_name": "mov_text",
                    "tags": { "language": "en" }
                },
                {
                    "index": 2,
                    "codec_name": "h264",
                    "codec_type": "video"
                }
            ]
        });
        let report = parse_probe_output(&serde_json::to_vec(&output).unwrap()).unwrap();
        assert_eq!(report.tracks.len(), 2);
        assert_eq!(report.tracks[0].index, 0);
        assert_eq!(report.tracks[0].language.as_deref(), Some("id"));
        assert_eq!(report.tracks[0].codec.as_deref(), Some("subrip"));
        assert_eq!(report.tracks[1].index, 1);
        assert_eq!(report.tracks[1].language.as_deref(), Some("en"));
        assert_eq!(report.tracks[1].codec.as_deref(), Some("mov_text"));
    }

    #[test]
    fn parse_probe_output_handles_missing_fields() {
        // No codec_type (real -select_streams s output), no tags, no codec.
        let output = serde_json::json!({
            "streams": [
                { "index": 0 },
                { "index": 3, "tags": {} }
            ]
        });
        let report = parse_probe_output(&serde_json::to_vec(&output).unwrap()).unwrap();
        assert_eq!(report.tracks.len(), 2);
        assert_eq!(report.tracks[0].language, None);
        assert_eq!(report.tracks[0].codec, None);
        assert_eq!(report.tracks[1].language, None);
        assert_eq!(report.tracks[1].index, 3);
    }

    #[test]
    fn parse_probe_output_rejects_garbage() {
        assert!(parse_probe_output(b"not json").is_none());
        assert!(parse_probe_output(b"{}").is_some_and(|r| r.tracks.is_empty()));
    }

    #[test]
    fn notice_reports_probe_tracks() {
        let report = ProbeReport {
            tracks: vec![
                ProbeTrack {
                    index: 0,
                    language: Some("id".to_string()),
                    codec: Some("subrip".to_string()),
                },
                ProbeTrack {
                    index: 1,
                    language: Some("en".to_string()),
                    codec: Some("subrip".to_string()),
                },
                ProbeTrack {
                    index: 2,
                    language: Some("id".to_string()),
                    codec: Some("subrip".to_string()),
                },
            ],
        };
        let message = format_embedded_notice(true, Some(&report));
        assert_eq!(
            message,
            "Found 3 embedded subtitle track(s) (e.g. id, en). If the player doesn't show one, press 'j' (mpv) to cycle tracks."
        );
    }

    #[test]
    fn notice_reports_probe_tracks_without_languages() {
        let report = ProbeReport {
            tracks: vec![ProbeTrack {
                index: 0,
                language: None,
                codec: Some("subrip".to_string()),
            }],
        };
        let message = format_embedded_notice(true, Some(&report));
        assert_eq!(
            message,
            "Found 1 embedded subtitle track(s). If the player doesn't show one, press 'j' (mpv) to cycle tracks."
        );
    }

    #[test]
    fn notice_falls_back_to_likely_hint() {
        assert_eq!(
            format_embedded_notice(true, None),
            "This stream is an MKV/MP4 and may contain embedded subtitles. If the player doesn't show one, press 'j' (mpv) to cycle tracks."
        );
        // Empty report (no tracks) also falls back to the likely hint.
        let empty = ProbeReport { tracks: Vec::new() };
        assert_eq!(
            format_embedded_notice(true, Some(&empty)),
            "This stream is an MKV/MP4 and may contain embedded subtitles. If the player doesn't show one, press 'j' (mpv) to cycle tracks."
        );
    }

    #[test]
    fn notice_is_empty_when_neither() {
        assert_eq!(format_embedded_notice(false, None), "");
        let empty = ProbeReport { tracks: Vec::new() };
        assert_eq!(format_embedded_notice(false, Some(&empty)), "");
    }
}
