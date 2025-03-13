use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct CachedPlugin {
    pub xml_id: String,
    pub numeric_id: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CachedPluginVersion {
    pub version: String,
    pub update_id: u64,
    pub channel: String,
    pub plugin_xml_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CachedUpdateDependency {
    pub update_id: u64,
    pub dependency_xml_id: String,
    pub optional: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CachedUpdate {
    pub id: u64,
    pub stale: bool,
    pub etag: Option<String>,
    pub file_name: Option<String>,
    pub download_url: Option<String>,
    pub hash_algorithm: Option<String>,
    pub hash: Option<Vec<u8>>,
}
