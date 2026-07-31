pub mod cache;
pub mod opensubtitles;

use crate::providers::models::ProviderKind;
use opensubtitles::OpenSubtitlesError;

#[derive(Debug, Clone, Default)]
pub struct SubtitleContext {
    pub provider: ProviderKind,
    pub subject_id: String,
    pub resource_id: String,
    pub title: String,
    pub year: Option<String>,
    pub is_episode: bool,
    pub season: Option<usize>,
    pub episode: Option<usize>,
    pub imdb_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OsCandidate {
    pub label: String,
    pub file_id: u32,
    pub language: String,
    pub score: i32,
    pub release_name: Option<String>,
    pub download_count: Option<u32>,
    pub machine_translated: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OsSearchOutcome {
    pub candidates: Vec<OsCandidate>,
    pub from_cache: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum SubtitleError {
    #[error("OpenSubtitles: {0}")]
    OpenSubtitles(#[from] OpenSubtitlesError),
    #[error("Subtitle not found")]
    NotFound,
    #[error("subtitle provider disabled")]
    Disabled,
}

pub fn merge_os_candidates(
    base: Vec<(String, String)>,
    candidates: &[OsCandidate],
) -> Vec<(String, String)> {
    let mut out = base;
    let existing_labels: std::collections::HashSet<String> =
        out.iter().map(|(n, _)| n.to_lowercase()).collect();
    for c in candidates {
        let label = build_label(c);
        if existing_labels.contains(&label.to_lowercase()) {
            continue;
        }
        out.push((label, format!("os:{}:{}", c.file_id, c.language)));
    }
    out
}

pub fn build_label(c: &OsCandidate) -> String {
    let lang = if c.language.eq_ignore_ascii_case("id") || c.language.eq_ignore_ascii_case("indonesian") {
        "Indonesian".to_string()
    } else {
        c.language.clone()
    };
    let mut label = format!("{lang} [OS]");
    if let Some(rn) = &c.release_name {
        let short: String = rn.chars().take(40).collect();
        label.push_str(&format!(" · {short}"));
    }
    if let Some(dc) = c.download_count {
        label.push_str(&format!(" · {dc} dl"));
    }
    if c.machine_translated {
        label.push_str(" · MT");
    }
    label
}

pub fn is_indonesian_label(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.contains("indonesia")
        || lower.contains("indonesian")
        || lower.contains("indo")
        || lower.contains("bahasa")
        || lower.trim() == "id"
}

pub fn extract_imdb_id(details: &serde_json::Value) -> Option<String> {
    let raw = ["imdbId", "imdb_id", "imdb", "doubanId", "tmdbId"]
        .iter()
        .find_map(|k| details.get(k).and_then(|v| v.as_str()))
        .map(str::to_string)
        .filter(|s| !s.is_empty() && s != "null")?;
    if raw.starts_with("tt") && raw.len() > 2 && raw[2..].chars().all(|c| c.is_ascii_digit()) {
        Some(raw)
    } else {
        None
    }
}

pub fn score_candidate(item: &opensubtitles::SubtitleItem, ctx: &SubtitleContext) -> i32 {
    let mut score = 0;
    let attr = &item.attributes;
    let lang = attr.language.as_deref().unwrap_or("");
    if lang.eq_ignore_ascii_case("id") || lang.eq_ignore_ascii_case("indonesian") {
        score += 50;
    }
    if let Some(yr) = ctx.year.as_deref() {
        if attr.release_name.as_deref().is_some_and(|r| r.contains(yr)) {
            score += 15;
        }
    }
    if attr.release_name.as_deref().is_some_and(|r| {
        ctx.title.split_whitespace().take(3).any(|tok| {
            let t = tok.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase();
            t.len() >= 4 && r.to_lowercase().contains(&t)
        })
    }) {
        score += 20;
    }
    if !attr.ai_translated.unwrap_or(false) && !attr.machine_translated.unwrap_or(false) {
        score += 10;
    }
    if let Some(dc) = attr.download_count {
        score += (dc.min(100_000) as f32 / 10_000.0) as i32;
    }
    score
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_indonesian_label() {
        assert!(is_indonesian_label("Indonesian"));
        assert!(is_indonesian_label("Bahasa Indonesia"));
        assert!(is_indonesian_label("Indo"));
        assert!(is_indonesian_label("id"));
        assert!(!is_indonesian_label("English"));
    }

    #[test]
    fn test_extract_imdb_id() {
        let valid = serde_json::json!({"imdbId": "tt0848228"});
        assert_eq!(extract_imdb_id(&valid), Some("tt0848228".to_string()));

        let invalid = serde_json::json!({"imdbId": "12345"});
        assert_eq!(extract_imdb_id(&invalid), None);
    }

    #[test]
    fn test_merge_os_candidates_dedup() {
        let base = vec![("Indonesian [OS] · test · 10 dl".to_string(), "os:1:id".to_string())];
        let cands = vec![
            OsCandidate {
                label: "".into(),
                file_id: 1,
                language: "id".into(),
                score: 100,
                release_name: Some("test".into()),
                download_count: Some(10),
                machine_translated: false,
            },
            OsCandidate {
                label: "".into(),
                file_id: 2,
                language: "id".into(),
                score: 90,
                release_name: Some("other".into()),
                download_count: Some(5),
                machine_translated: false,
            },
        ];
        let merged = merge_os_candidates(base, &cands);
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[1].1, "os:2:id");
    }

    #[test]
    fn test_score_candidate_prioritizes_id() {
        let id_item: opensubtitles::SubtitleItem = serde_json::from_value(serde_json::json!({
            "id": "1",
            "attributes": {
                "language": "id",
                "files": [{ "file_id": 1 }],
                "download_count": 0
            }
        }))
        .unwrap();
        let en_item: opensubtitles::SubtitleItem = serde_json::from_value(serde_json::json!({
            "id": "2",
            "attributes": {
                "language": "en",
                "files": [{ "file_id": 2 }],
                "download_count": 0
            }
        }))
        .unwrap();
        let ctx = SubtitleContext::default();
        let id_score = score_candidate(&id_item, &ctx);
        let en_score = score_candidate(&en_item, &ctx);
        assert!(id_score > en_score, "expected {id_score} > {en_score}");
    }

    #[test]
    fn test_score_candidate_year_and_title_bonus() {
        let with_bonus: opensubtitles::SubtitleItem = serde_json::from_value(serde_json::json!({
            "id": "1",
            "attributes": {
                "language": "en",
                "release_name": "Dilan 1990 1080p BluRay x264",
                "files": [{ "file_id": 1 }]
            }
        }))
        .unwrap();
        let without_bonus: opensubtitles::SubtitleItem = serde_json::from_value(serde_json::json!({
            "id": "2",
            "attributes": {
                "language": "en",
                "release_name": "Some Other Movie",
                "files": [{ "file_id": 2 }]
            }
        }))
        .unwrap();

        let ctx = SubtitleContext {
            provider: ProviderKind::MovieBox,
            subject_id: "1".into(),
            resource_id: "1".into(),
            title: "Dilan 1990".into(),
            year: Some("1990".into()),
            is_episode: false,
            season: None,
            episode: None,
            imdb_id: None,
        };

        let a = score_candidate(&with_bonus, &ctx);
        let b = score_candidate(&without_bonus, &ctx);
        assert!(a > b, "expected year+title bonus, got {a} <= {b}");
    }

    #[test]
    fn test_merge_os_candidates_marker_format() {
        let cands = vec![OsCandidate {
            label: String::new(),
            file_id: 987,
            language: "id".into(),
            score: 60,
            release_name: Some("Dilan 1990".into()),
            download_count: Some(5),
            machine_translated: false,
        }];
        let merged = merge_os_candidates(vec![("None".to_string(), String::new())], &cands);
        assert_eq!(merged.len(), 2);
        let marker = &merged[1].1;
        assert_eq!(marker, "os:987:id");
        assert!(marker.starts_with("os:"));
        let (fid, lang) = marker[3..].split_once(':').unwrap();
        assert_eq!(fid, "987");
        assert_eq!(lang, "id");
    }

    #[test]
    fn test_build_label_contains_os() {
        let c = OsCandidate {
            label: String::new(),
            file_id: 1,
            language: "id".into(),
            score: 0,
            release_name: Some(
                "long_release_name_that_is_way_more_than_forty_characters_long".into(),
            ),
            download_count: Some(12),
            machine_translated: false,
        };
        let label = build_label(&c);
        assert!(label.starts_with("Indonesian [OS]"));
        assert!(label.contains("[OS]"));
        assert!(label.contains("12 dl"));

        let parts: Vec<&str> = label.split(" · ").collect();
        assert_eq!(parts.len(), 3, "unexpected label shape: {label}");
        assert!(parts[1].chars().count() <= 40, "release name not truncated: {label}");
    }
}
