use reqwest::Url;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoPluginDetails {
    pub xml_id: String,
    pub id: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RepoUpdateVersion {
    pub id: u64,
    pub version: String,
    pub channel: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoUpdateMetadata {
    #[serde(default)]
    pub dependencies: Vec<String>,

    #[serde(default)]
    pub optional_dependencies: Vec<String>,

    #[serde(default)]
    pub since: Option<String>,

    #[serde(default)]
    pub until: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RepoDownloadInfo {
    pub url: Url,
    pub etag: Option<String>,
    pub file_name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RepoDownloadHash {
    pub algorithm: String,
    pub value: Vec<u8>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoBrokenPlugin {
    pub id: String,
    pub version: String,
    pub since: Option<String>,
    pub until: Option<String>,
    pub original_since: Option<String>,
    pub original_until: Option<String>,
}
