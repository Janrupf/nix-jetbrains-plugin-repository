use crate::db::{CachedPlugin, CachedUpdateDependency, Database};
use crate::error::IndexerError;
use crate::meta::TaskAttachment;
use base64::Engine;
use base64::prelude::BASE64_STANDARD;
use futures::stream::FuturesUnordered;
use futures::{FutureExt, StreamExt as _};
use semver::Version;
use serde::Serialize;
use sha2::Digest as _;
use std::collections::BTreeMap;
use std::collections::btree_map::Entry;
use std::future;
use std::path::{Path, PathBuf};

pub async fn generate_into(
    directory: impl Into<PathBuf>,
    database: Database,
) -> Result<(), IndexerError> {
    let directory = directory.into();
    tokio::fs::create_dir_all(&directory).await?;

    let plugin_index = database
        .get_all_plugins()
        .await?
        .into_iter()
        .map(|plugin| {
            let database = database.clone();
            let directory = directory.clone();

            tokio::spawn(async move {
                let mut sha_hasher = sha2::Sha256::new();
                sha_hasher.update(plugin.xml_id.as_bytes());
                let hash_bytes = sha_hasher.finalize_reset();

                let hex_digest =
                    hash_bytes
                        .into_iter()
                        .fold(String::with_capacity(64), |mut acc, byte| {
                            let (high, low) = byte_to_hex(byte);

                            acc.push(high);
                            acc.push(low);
                            acc
                        });

                let plugin_dir = directory
                    .join(&hex_digest[0..2])
                    .join(&hex_digest[2..4])
                    .join(&hex_digest[4..]);

                if let Err(err) = generate_plugin(plugin_dir, &plugin, &database).await {
                    tracing::error!("Failed to generate plugin '{}': {:?}", plugin.xml_id, err);
                    return None;
                }

                Some((plugin.xml_id, hex_digest))
            })
        })
        .collect::<FuturesUnordered<_>>()
        .filter_map(|v| {
            future::ready(match v {
                Ok(Some(v)) => Some(v),
                Ok(None) => None,
                Err(err) => {
                    tracing::error!("Failed to process plugin: {:?}", err);
                    None
                }
            })
        })
        .collect::<BTreeMap<_, _>>()
        .await;

    let index_path = directory.join("index.json");
    tokio::task::spawn_blocking(move || {
        let index_file = std::fs::File::create(index_path)?;
        serde_json::to_writer_pretty(index_file, &plugin_index)?;
        Ok::<_, IndexerError>(())
    })
    .await
    .unwrap()?;

    Ok(())
}

async fn generate_plugin(
    plugin_directory: impl AsRef<Path>,
    plugin: &CachedPlugin,
    database: &Database,
) -> Result<(), IndexerError> {
    let plugin_directory = plugin_directory.as_ref();
    tokio::fs::create_dir_all(plugin_directory).await?;

    let versions = database
        .get_versions_for_plugin(&plugin.xml_id)
        .await?
        .into_iter()
        .map(|version| async move {
            let (update_info, all_dependencies) = tokio::try_join!(
                database.get_update(version.update_id),
                database.get_update_dependencies(version.update_id)
            )?;

            if update_info.stale {
                tracing::warn!("Update {} is stale", version.update_id);
                return Ok(None);
            }

            let Some(download_url) = update_info.download_url else {
                tracing::warn!("No download URL for update {}", version.update_id);
                return Ok(None);
            };

            if update_info
                .hash_algorithm
                .as_deref()
                .map(|v| v != "SHA-256")
                .unwrap_or(true)
            {
                tracing::warn!(
                    "Unsupported hash algorithm for update {}",
                    version.update_id
                );
                return Ok(None);
            }

            let hash = update_info
                .hash
                .expect("Hash algorith set but no hash provided");
            let sha256 = BASE64_STANDARD.encode(&hash);

            let channel = if version.channel.is_empty() {
                "stable".to_string()
            } else {
                version.channel.to_lowercase()
            };

            let dep_id = |d: CachedUpdateDependency| d.dependency_xml_id;

            let (dependencies, optional_dependencies): (
                Vec<CachedUpdateDependency>,
                Vec<CachedUpdateDependency>,
            ) = all_dependencies.into_iter().partition(|dep| !dep.optional);

            Ok::<_, IndexerError>(Some((
                version.version,
                VersionMetadata {
                    download_url,
                    sha256,
                    channel,
                    dependencies: dependencies.into_iter().map(dep_id).collect(),
                    optional_dependencies: optional_dependencies.into_iter().map(dep_id).collect(),
                },
            )))
        })
        .collect::<FuturesUnordered<_>>()
        .filter_map(|v| {
            future::ready(match v {
                Ok(Some(v)) => Some(v),
                Ok(None) => None,
                Err(err) => {
                    tracing::error!("Failed to process version: {:?}", err);
                    None
                }
            })
        })
        .collect::<BTreeMap<String, VersionMetadata>>()
        .await;

    let mut latest = BTreeMap::<String, String>::new();

    for (version, version_metadata) in &versions {
        let mut entry = match latest.entry(version_metadata.channel.clone()) {
            Entry::Vacant(entry) => {
                entry.insert(version.clone());
                continue;
            }
            Entry::Occupied(v) => v,
        };

        let current_version = Version::parse(entry.get()).ok();
        let new_version = Version::parse(version).ok();

        match (current_version, new_version) {
            (None, None) => { /* no change */ }
            (Some(_), None) => { /* no change */ }
            (None, Some(_)) => {
                entry.insert(version.clone());
            }
            (Some(current), Some(new)) => {
                if new > current {
                    entry.insert(version.clone());
                }
            }
        }
    }

    let metadata = PluginMetadata {
        xml_id: plugin.xml_id.clone(),
        numeric_id: plugin.numeric_id,
        versions,
        latest,
    };

    let metadata_path = plugin_directory.join("metadata.json");
    tokio::task::spawn_blocking(move || {
        let metadata_file = std::fs::File::create(metadata_path)?;
        serde_json::to_writer_pretty(metadata_file, &metadata)?;
        Ok::<_, IndexerError>(())
    })
    .await
    .unwrap()?;

    Ok(())
}

fn byte_to_hex(byte: u8) -> (char, char) {
    (
        std::char::from_digit((byte >> 4) as u32, 16).unwrap(),
        std::char::from_digit((byte & 0xF) as u32, 16).unwrap(),
    )
}

#[derive(Debug, Serialize)]
struct PluginMetadata {
    pub xml_id: String,
    pub numeric_id: u64,
    pub versions: BTreeMap<String, VersionMetadata>,
    pub latest: BTreeMap<String, String>,
}

#[derive(Debug, Serialize)]
struct VersionMetadata {
    pub download_url: String,
    pub sha256: String,
    pub channel: String,
    pub dependencies: Vec<String>,
    pub optional_dependencies: Vec<String>,
}
