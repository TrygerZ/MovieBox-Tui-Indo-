use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Channel {
    pub id: String,
    pub name: String,
    pub logo: String,
    pub group: String,
    pub stream_url: String,
}

pub struct M3UParser {
    cache_dir: PathBuf,
}

impl Default for M3UParser {
    fn default() -> Self {
        Self::new()
    }
}

impl M3UParser {
    pub fn new() -> Self {
        let mut cache_dir = dirs::cache_dir().unwrap_or_else(|| PathBuf::from("."));
        cache_dir.push("moviebox-tui");
        cache_dir.push("tv_playlists");
        std::fs::create_dir_all(&cache_dir).ok();
        Self { cache_dir }
    }

    pub async fn fetch_playlist(
        &self,
        url: &str,
        filename: &str,
    ) -> Result<Vec<Channel>, Box<dyn std::error::Error>> {
        let file_path = self.cache_dir.join(filename);
        let mut needs_download = true;

        if file_path.exists() {
            if let Ok(metadata) = fs::metadata(&file_path) {
                if let Ok(modified) = metadata.modified() {
                    if let Ok(duration) = SystemTime::now().duration_since(modified) {
                        if duration.as_secs() < 24 * 3600 {
                            needs_download = false;
                        }
                    }
                }
            }
        }

        let content = if needs_download {
            // Timeouts so a dead playlist URL can't block the fetch task forever;
            // mirrors timeouts used by the other providers (subdl, moviebox).
            let client = reqwest::Client::builder()
                .timeout(Duration::from_secs(15))
                .connect_timeout(Duration::from_secs(5))
                .build()?;
            let res = client.get(url).send().await?.text().await?;
            fs::write(&file_path, &res).ok();
            res
        } else {
            fs::read_to_string(&file_path)?
        };

        Ok(self.parse_m3u(&content))
    }

    fn parse_m3u(&self, content: &str) -> Vec<Channel> {
        // Strip UTF-8 BOM so "\u{feff}#EXTM3U" is recognized as the header
        // instead of being parsed as a bogus stream-URL channel.
        let content = content.trim_start_matches('\u{feff}');
        let mut channels = Vec::new();
        let mut current_channel = Channel {
            id: String::new(),
            name: String::new(),
            logo: String::new(),
            group: String::new(),
            stream_url: String::new(),
        };

        let extract_attr = |line: &str, attr: &str| -> String {
            if let Some(idx) = line.find(attr) {
                let start = idx + attr.len();
                if let Some(end) = line[start..].find('"') {
                    return line[start..start + end].to_string();
                }
            }
            String::new()
        };

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if line.starts_with("#EXTINF:") {
                current_channel.id = extract_attr(line, "tvg-id=\"");
                current_channel.logo = extract_attr(line, "tvg-logo=\"");
                current_channel.group = extract_attr(line, "group-title=\"");

                if let Some(idx) = line.rfind(',') {
                    current_channel.name = line[idx + 1..].trim().to_string();
                }
            } else if !line.starts_with('#') {
                current_channel.stream_url = line.to_string();
                if current_channel.id.is_empty() {
                    current_channel.id = current_channel.name.clone();
                }
                channels.push(current_channel.clone());

                current_channel = Channel {
                    id: String::new(),
                    name: String::new(),
                    logo: String::new(),
                    group: String::new(),
                    stream_url: String::new(),
                };
            }
        }

        channels
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf8_bom_is_stripped_before_parsing() {
        let parser = M3UParser::new();
        let content = concat!(
            "\u{feff}#EXTM3U\n",
            "#EXTINF:-1 tvg-id=\"cnn\" tvg-logo=\"http://logo/x.png\" group-title=\"News\",CNN International\n",
            "http://stream.example/cnn.m3u8\n",
        );
        let channels = parser.parse_m3u(content);
        assert_eq!(channels.len(), 1, "BOM must not produce a bogus channel");
        assert_eq!(channels[0].name, "CNN International");
        assert_eq!(channels[0].id, "cnn");
        assert_eq!(channels[0].stream_url, "http://stream.example/cnn.m3u8");
    }
}
