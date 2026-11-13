pub mod client;
pub mod models;

#[cfg(test)]
mod tests;

pub use client::{DramachiClient, DramachiError};

use super::models::{CatalogItem, MediaDetails, ProviderError, ProviderKind, Release};
use super::{Provider, ProviderCapabilities, ReleaseProvider};

impl Provider for DramachiClient {
    fn id(&self) -> ProviderKind {
        ProviderKind::Dramachi
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            supports_search: true,
            supports_pagination: true,
            supports_series: true,
            supports_subtitles: false,
            supports_homepage: false,
        }
    }

    async fn search(&self, query: &str, page: usize) -> Result<Vec<CatalogItem>, ProviderError> {
        self.search(query, page).await.map_err(ProviderError::from)
    }

    async fn details(&self, id: &str) -> Result<MediaDetails, ProviderError> {
        self.details(id).await.map_err(ProviderError::from)
    }
}

impl ReleaseProvider for DramachiClient {
    async fn episode_streams(
        &self,
        id: &str,
        season: usize,
        episode: usize,
    ) -> Result<Vec<Release>, ProviderError> {
        self.episode_streams(id, season, episode)
            .await
            .map_err(ProviderError::from)
    }
}
