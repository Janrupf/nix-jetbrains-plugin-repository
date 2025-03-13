use crate::db::{CachedPlugin, CachedPluginVersion, CachedUpdateDependency};
use crate::error::IndexerError;
use crate::meta::TaskAttachment;

#[tracing::instrument(skip(attachment))]
pub(super) async fn sync_new_plugin(
    attachment: TaskAttachment,
    xml_id: String,
) -> Result<(), IndexerError> {
    let details = attachment.repo.fetch_plugin_details(&xml_id).await?;

    let known = CachedPlugin {
        xml_id,
        numeric_id: details.id,
    };
    attachment.database.add_plugin(&known).await?;

    attachment.dispatch(
        format!("sync plugin {}", known.xml_id),
        sync_plugin(attachment.clone(), known),
    );

    Ok(())
}

#[tracing::instrument(
    skip(attachment, known_plugin),
    fields(plugin_id = known_plugin.xml_id.as_str())
)]
pub(super) async fn sync_plugin(
    attachment: TaskAttachment,
    known_plugin: CachedPlugin,
) -> Result<(), IndexerError> {
    let (repo_versions, cached_versions) = tokio::try_join!(
        attachment
            .repo
            .fetch_plugin_versions(known_plugin.numeric_id),
        attachment
            .database
            .get_versions_for_plugin(&known_plugin.xml_id)
    )?;

    for version in &repo_versions {
        let version = CachedPluginVersion {
            update_id: version.id,
            version: version.version.clone(),
            channel: version.channel.clone(),
            plugin_xml_id: known_plugin.xml_id.clone(),
        };

        attachment.database.add_update(version.update_id).await?;
        attachment.database.add_plugin_version(&version).await?;

        // We only do this for added versions since we don't expect a version
        // that has been released to ever change its metadata.
        attachment.dispatch(
            format!(
                "sync update metadata for {}@{}",
                known_plugin.xml_id, version.version
            ),
            sync_update_dependency_meta(attachment.clone(), known_plugin.clone(), version.clone()),
        );

        if attachment
            .database
            .mark_update_not_stale(version.update_id)
            .await?
        {
            // We were the ones marking it as not stale, so we need to sync it
            attachment.dispatch(
                format!("sync update metadata for {}", version.update_id),
                sync_update_meta(attachment.clone(), version.update_id),
            );
        }
    }

    for cached_version in &cached_versions {
        if !repo_versions
            .iter()
            .any(|v| v.id == cached_version.update_id)
        {
            tracing::trace!("Removing cached version: {:?}", cached_version);
            attachment
                .database
                .remove_plugin_version(&cached_version.plugin_xml_id, &cached_version.version)
                .await?
        }
    }

    Ok(())
}

#[tracing::instrument(
    skip(attachment, plugin, version),
    fields(plugin_id = plugin.xml_id.as_str(), version = version.version.as_str())
)]
async fn sync_update_dependency_meta(
    attachment: TaskAttachment,
    plugin: CachedPlugin,
    version: CachedPluginVersion,
) -> Result<(), IndexerError> {
    let metadata = attachment
        .repo
        .fetch_update_metadata(plugin.numeric_id, version.update_id)
        .await?;

    for dependency in metadata.dependencies {
        let dependency = CachedUpdateDependency {
            dependency_xml_id: dependency,
            update_id: version.update_id,
            optional: false,
        };

        attachment
            .database
            .add_update_dependency(&dependency)
            .await?;
    }

    for optional_dependency in metadata.optional_dependencies {
        let dependency = CachedUpdateDependency {
            dependency_xml_id: optional_dependency,
            update_id: version.update_id,
            optional: true,
        };

        attachment
            .database
            .add_update_dependency(&dependency)
            .await?;
    }

    Ok(())
}

#[tracing::instrument(skip(attachment))]
async fn sync_update_meta(attachment: TaskAttachment, update_id: u64) -> Result<(), IndexerError> {
    let (download_info, mut cached_update) = tokio::try_join!(
        attachment.repo.resolve_update_download_info(update_id),
        attachment.database.get_update(update_id)
    )?;

    if cached_update.etag.as_deref() == download_info.etag.as_deref() {
        // Up-to-date
        return Ok(());
    }

    let hash_info = attachment
        .repo
        .hash_download_url(&download_info.url)
        .await?;

    cached_update.etag = download_info.etag;
    cached_update.file_name = download_info.file_name;
    cached_update.download_url = Some(download_info.url.to_string());
    cached_update.hash_algorithm = Some(hash_info.algorithm);
    cached_update.hash = Some(hash_info.value);

    attachment
        .database
        .change_update_info(&cached_update)
        .await?;

    Ok(())
}
