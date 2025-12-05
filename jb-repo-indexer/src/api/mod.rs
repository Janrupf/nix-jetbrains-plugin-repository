mod models;
pub use models::*;

use crate::args::IndexerArgs;
use crate::error::IndexerError;
use base64::Engine as _;
use base64::prelude::BASE64_STANDARD;
use reqwest::redirect::Policy;
use reqwest::{Client, StatusCode, Url};
use sha2::Digest as _;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

#[derive(Debug, Clone)]
pub struct JetbrainsRepoApi {
    client: Client,
    small_request_semaphore: Arc<Semaphore>,
    large_request_semaphore: Arc<Semaphore>,
    base: Url,
}

impl JetbrainsRepoApi {
    /// Prepare the API client.
    pub fn new(args: &IndexerArgs) -> Result<Self, IndexerError> {
        let client = Client::builder()
            .user_agent(concat!(
                env!("CARGO_PKG_NAME"),
                "/",
                env!("CARGO_PKG_VERSION")
            ))
            .redirect(Policy::limited(10))
            .hickory_dns(true)
            .build()?;

        let small_request_semaphore =
            Arc::new(Semaphore::new(args.max_parallel_small_requests.get()));

        let large_request_semaphore =
            Arc::new(Semaphore::new(args.max_parallel_large_requests.get()));

        let base = Url::parse("https://plugins.jetbrains.com/").unwrap();

        Ok(Self {
            client,
            small_request_semaphore,
            large_request_semaphore,
            base,
        })
    }

    #[tracing::instrument(skip(self))]
    pub async fn fetch_all_xml_ids(&self) -> Result<HashSet<String>, IndexerError> {
        let permit = self.acquire_small_permit().await;

        let response = self
            .client
            .get(self.path(["files", "pluginsXMLIds.json"]))
            .send()
            .await?
            .error_for_status()?;

        let data = response.bytes().await?;
        drop(permit);

        serde_json::from_slice(&data).map_err(IndexerError::from)
    }

    #[tracing::instrument(skip(self))]
    pub async fn fetch_plugin_details(
        &self,
        xml_id: &str,
    ) -> Result<RepoPluginDetails, IndexerError> {
        let permit = self.acquire_small_permit().await;

        let response = self
            .client
            .get(self.path(["api", "plugins", "intellij", xml_id]))
            .send()
            .await?
            .error_for_status()?;

        let data = response.bytes().await?;
        drop(permit);

        serde_json::from_slice(&data).map_err(IndexerError::from)
    }

    #[tracing::instrument(skip(self))]
    pub async fn fetch_plugin_versions(
        &self,
        plugin_id: u64,
    ) -> Result<Vec<RepoUpdateVersion>, IndexerError> {
        let permit = self.acquire_small_permit().await;

        let plugin_id_str = plugin_id.to_string();

        let response = self
            .client
            .get(self.path(["api", "plugins", &plugin_id_str, "updateVersions"]))
            .send()
            .await?
            .error_for_status()?;

        let data = response.bytes().await?;
        drop(permit);

        serde_json::from_slice(&data).map_err(IndexerError::from)
    }

    #[tracing::instrument(skip(self))]
    pub async fn fetch_update_metadata(
        &self,
        plugin_id: u64,
        update_id: u64,
    ) -> Result<RepoUpdateMetadata, IndexerError> {
        let permit = self.acquire_small_permit().await;

        let plugin_id_str = plugin_id.to_string();
        let update_id_str = update_id.to_string();

        let response = self
            .client
            .get(self.path(["files", &plugin_id_str, &update_id_str, "meta.json"]))
            .send()
            .await?
            .error_for_status()?;

        let data = response.bytes().await?;
        drop(permit);

        serde_json::from_slice(&data).map_err(IndexerError::from)
    }

    #[tracing::instrument(skip(self))]
    pub async fn resolve_update_download_info(
        &self,
        update_id: u64,
    ) -> Result<RepoDownloadInfo, IndexerError> {
        let permit = self.acquire_small_permit().await;

        let response = self
            .client
            .head(self.path(["plugin", "download"]))
            .query(&[("updateId", update_id)])
            .send()
            .await?
            .error_for_status()?;

        drop(permit);

        let url = response.url().clone();

        let etag = response.headers().get("etag").and_then(|v| {
            let v = v.to_str().ok()?.trim();

            v.strip_prefix('"')?
                .strip_suffix('"')
                .map(ToOwned::to_owned)
        });

        let file_name = response.headers().get("content-disposition").and_then(|v| {
            let v = v.to_str().ok()?.trim();

            v.strip_prefix("attachment; filename=\"")?
                .strip_suffix('"')
                .map(ToOwned::to_owned)
        });

        Ok(RepoDownloadInfo {
            url,
            etag,
            file_name,
        })
    }

    /// Compute the hash of a download URL.
    ///
    /// If `force_download` is false, tries to use JetBrains' pre-computed .hash.json files first.
    /// If `force_download` is true, always downloads the actual file and computes the hash.
    #[tracing::instrument(skip_all, fields(url = url.as_str(), force_download))]
    pub async fn hash_download_url(
        &self,
        url: &Url,
        force_download: bool,
    ) -> Result<RepoDownloadHash, IndexerError> {
        #[derive(serde::Deserialize)]
        struct DownloadHashData {
            algorithm: String,
            hash: String,
        }

        // Try pre-computed hash first (unless force_download is set)
        if !force_download {
            let mut hash_url = url.clone();
            hash_url.set_path(&(url.path().to_owned() + ".hash.json"));

            let permit = self.acquire_small_permit().await;
            let response = self.client.get(hash_url).send().await?;

            if !matches!(
                response.status(),
                StatusCode::NOT_FOUND | StatusCode::BAD_REQUEST | StatusCode::FORBIDDEN
            ) {
                // Use pre-computed hash
                let data = response.bytes().await?;
                drop(permit);

                let data: DownloadHashData =
                    serde_json::from_slice(&data).map_err(IndexerError::from)?;
                let decoded = BASE64_STANDARD.decode(&data.hash)?;

                return Ok(RepoDownloadHash {
                    algorithm: data.algorithm,
                    value: decoded,
                });
            }

            drop(permit);

            tracing::warn!(
                "Falling back to manual hashing for {} because we got status {}",
                url,
                response.status().as_str()
            );
        } else {
            tracing::info!("Force downloading {} to compute hash", url);
        }

        // Download the file and hash it ourselves
        let permit = self
            .large_request_semaphore
            .clone()
            .acquire_owned()
            .await
            .unwrap();

        let mut hasher = sha2::Sha256::new();

        let mut response = self
            .client
            .get(url.clone())
            .send()
            .await?
            .error_for_status()?;
        while let Some(chunk) = response.chunk().await? {
            hasher.update(&chunk);
        }

        drop(permit);

        Ok(RepoDownloadHash {
            algorithm: "SHA-256".to_owned(),
            value: hasher.finalize().to_vec(),
        })
    }

    #[tracing::instrument(skip(self))]
    pub async fn fetch_broken_plugins(&self) -> Result<Vec<RepoBrokenPlugin>, IndexerError> {
        let permit = self.acquire_small_permit().await;

        let response = self
            .client
            .get(self.path(["files", "brokenPlugins.json"]))
            .send()
            .await?
            .error_for_status()?;

        let data = response.bytes().await?;
        drop(permit);

        serde_json::from_slice(&data).map_err(IndexerError::from)
    }

    fn path(&self, segments: impl IntoIterator<Item = impl AsRef<str>>) -> Url {
        let mut new_path = self.base.clone();
        new_path.path_segments_mut().unwrap().extend(segments);

        new_path
    }

    #[tracing::instrument(skip(self))]
    async fn acquire_small_permit(&self) -> OwnedSemaphorePermit {
        self.small_request_semaphore
            .clone()
            .acquire_owned()
            .await
            .unwrap()
    }
}
