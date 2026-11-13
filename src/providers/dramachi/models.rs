use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct DramachiSearchResponse {
    #[serde(default)]
    pub data: Option<Vec<DramachiSearchItem>>,
    #[serde(default)]
    pub last_page: Option<usize>,
    #[serde(default)]
    pub next_page: Option<usize>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DramachiSearchItem {
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub thumb: Option<String>,
    #[serde(default)]
    pub year: Option<String>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub content: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct DramachiTitleDetailsResponse {
    #[serde(default)]
    pub album: Option<Vec<DramachiAlbumItem>>,
    #[serde(default)]
    pub cast: Option<Vec<DramachiCastMember>>,
    #[serde(default)]
    pub seasons: Option<HashMap<String, DramachiSeasonGroup>>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct DramachiAlbumItem {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub year: Option<String>,
    #[serde(default)]
    pub storyline: Option<String>,
    #[serde(default)]
    pub director: Option<String>,
    #[serde(default)]
    pub genres: Option<String>,
    #[serde(default)]
    pub thumb: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct DramachiCastMember {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub role: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct DramachiSeasonGroup {
    #[serde(default)]
    pub season_name: String,
    #[serde(default)]
    pub versions: Option<Vec<DramachiSeasonVersion>>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct DramachiSeasonVersion {
    #[serde(default)]
    pub version_name: String,
    #[serde(default)]
    pub rip: String,
    #[serde(default)]
    pub server: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct DramachiEpisodeListResponse {
    #[serde(default)]
    pub episode_list: Option<Vec<DramachiEpisodeItem>>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct DramachiEpisodeItem {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub rip: Option<String>,
    #[serde(default)]
    pub f_title: String,
    #[serde(default)]
    pub fid: String,
    #[serde(default)]
    pub disk: String,
    #[serde(default)]
    pub quality: Option<String>,
    #[serde(default)]
    pub size: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct DramachiFileInfoResponse {
    #[serde(default, rename = "fileInfo")]
    pub file_info: Option<Vec<DramachiFileInfo>>,
    #[serde(default, rename = "hostInfo")]
    pub host_info: Option<DramachiHostInfo>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct DramachiFileInfo {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub thumbnail: Option<String>,
    #[serde(default)]
    pub rip: Option<String>,
    #[serde(default)]
    pub ext: Option<String>,
    #[serde(default)]
    pub f_title: Option<String>,
    #[serde(default)]
    pub fid: Option<String>,
    #[serde(default)]
    pub size: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub filename: Option<String>,
    #[serde(default)]
    pub disk: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct DramachiHostInfo {
    #[serde(default)]
    pub host: String,
    #[serde(default, rename = "isDL")]
    pub is_dl: Option<bool>,
}
