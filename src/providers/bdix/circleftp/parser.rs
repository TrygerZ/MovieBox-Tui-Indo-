use crate::providers::models::{CatalogItem, MediaType, ProviderKind, ProviderMediaId};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CircleFtpPost {
    pub id: u64,
    pub title: Option<String>,
    pub name: Option<String>,
    pub r#type: Option<String>,
    pub year: Option<serde_json::Value>,
    pub quality: Option<String>,
    pub image: Option<String>,
    pub image_sm: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CircleFtpSearchResponse {
    pub posts: Option<Vec<CircleFtpPost>>,
}

pub fn circleftp_search_to_catalog(response: &CircleFtpSearchResponse) -> Vec<CatalogItem> {
    let mut items = Vec::new();
    if let Some(posts) = &response.posts {
        for post in posts {
            let title = post
                .title
                .as_ref()
                .or(post.name.as_ref())
                .cloned()
                .unwrap_or_else(|| "Unknown".to_string());
            let media_type = if post.r#type.as_deref() == Some("series") {
                MediaType::Series
            } else {
                MediaType::Movie
            };

            let year = post.year.as_ref().and_then(|y| {
                if y.is_number() {
                    Some(y.to_string())
                } else {
                    y.as_str().map(|s| s.to_string())
                }
            });

            let poster_url = post
                .image
                .as_ref()
                .or(post.image_sm.as_ref())
                .map(|img| format!("{}{}", super::client::UPLOADS_URL, img));

            items.push(CatalogItem {
                id: ProviderMediaId {
                    provider: ProviderKind::BdixCircleFtp,
                    value: post.id.to_string(),
                },
                title,
                media_type,
                year,
                poster_url,
                season_count: None,
            });
        }
    }
    items
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_circleftp_search_to_catalog_maps_movies_and_series() {
        let json_str = r#"{
            "posts": [
                {
                    "id": 101,
                    "title": "Dune: Part Two",
                    "type": "movie",
                    "year": 2024,
                    "image": "dune2.jpg"
                },
                {
                    "id": 202,
                    "name": "Severance",
                    "type": "series",
                    "year": "2022",
                    "imageSm": "sev_sm.jpg"
                }
            ]
        }"#;
        let response: CircleFtpSearchResponse = serde_json::from_str(json_str).unwrap();
        let catalog = circleftp_search_to_catalog(&response);
        assert_eq!(catalog.len(), 2);
        assert_eq!(catalog[0].id.value, "101");
        assert_eq!(catalog[0].title, "Dune: Part Two");
        assert_eq!(catalog[0].media_type, MediaType::Movie);
        assert_eq!(catalog[0].year.as_deref(), Some("2024"));
        assert!(
            catalog[0]
                .poster_url
                .as_deref()
                .unwrap()
                .ends_with("dune2.jpg")
        );

        assert_eq!(catalog[1].id.value, "202");
        assert_eq!(catalog[1].title, "Severance");
        assert_eq!(catalog[1].media_type, MediaType::Series);
        assert_eq!(catalog[1].year.as_deref(), Some("2022"));
        assert!(
            catalog[1]
                .poster_url
                .as_deref()
                .unwrap()
                .ends_with("sev_sm.jpg")
        );
    }
}
