mod models;
pub use models::*;

use crate::args::IndexerArgs;
use crate::error::IndexerError;
use futures::{Stream, TryFutureExt, TryStreamExt, future};
use libsql::{Connection, Row};
use serde::de::DeserializeOwned;
use std::collections::HashSet;

#[derive(Clone)]
pub struct Database {
    connection: Connection,
}

fn map_row_de<T: DeserializeOwned>(r: Row) -> impl Future<Output = Result<T, IndexerError>> {
    let v = libsql::de::from_row::<T>(&r).map_err(|e| {
        tracing::error!(
            "Failed to deserialize {}: {}",
            std::any::type_name::<T>(),
            e
        );

        IndexerError::from(e)
    });

    future::ready(v)
}

impl Database {
    /// Connect to the database.
    pub async fn setup(args: &IndexerArgs) -> Result<Self, IndexerError> {
        tracing::debug!("Setting up database at {}", args.database.display());

        if let Some(parent) = args.database.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|e| {
                tracing::error!("Failed to create database directory: {}", e);
                e
            })?;
        }

        let db = libsql::Builder::new_local(&args.database).build().await?;

        // Ensure the database is created and the schema is up to date.
        let connection = db.connect()?;

        // Enable foreign key support
        connection.query("PRAGMA foreign_keys = ON", ()).await?;
        connection.query("PRAGMA journal_mode = WAL", ()).await?;
        connection.query("PRAGMA synchronous = NORMAL", ()).await?;

        tracing::debug!("Connected to database");
        Self::ensure_db_structure(&connection).await?;

        Ok(Self { connection })
    }

    async fn ensure_db_structure(connection: &Connection) -> Result<(), IndexerError> {
        tracing::trace!("Setting up database structure...");
        let tx = connection.transaction().await?;

        tx.execute(
            r#"
            CREATE TABLE IF NOT EXISTS plugins (
                xml_id TEXT PRIMARY KEY NOT NULL,
                numeric_id INTEGER NOT NULL
            )
        "#,
            (),
        )
        .await?;

        tx.execute(
            r#"
            CREATE TABLE IF NOT EXISTS versions (
                version TEXT NOT NULL,
                update_id INTEGER NOT NULL,
                channel TEXT NOT NULL,
                plugin_xml_id TEXT NOT NULL,
                PRIMARY KEY (version, plugin_xml_id),
                FOREIGN KEY (update_id) REFERENCES updates(id) ON DELETE CASCADE,
                FOREIGN KEY (plugin_xml_id) REFERENCES plugins(xml_id) ON DELETE CASCADE
            )
        "#,
            (),
        )
        .await?;

        tx.execute(
            r#"
            CREATE TABLE IF NOT EXISTS updates (
                id INTEGER PRIMARY KEY NOT NULL,
                stale BOOLEAN NOT NULL DEFAULT TRUE,
                etag TEXT DEFAULT NULL,
                file_name TEXT DEFAULT NULL,
                download_url TEXT DEFAULT NULL,
                hash_algorithm TEXT DEFAULT NULL,
                hash BLOB DEFAULT NULL
            )
        "#,
            (),
        )
        .await?;

        // Note about the following table:
        // The dependency xml id on purpose does not reference the plugins table,
        // because some dependencies might not be plugins but rather core modules
        // of IDE's.
        tx.execute(
            r#"
            CREATE TABLE IF NOT EXISTS update_dependencies (
                update_id INTEGER NOT NULL,
                dependency_xml_id TEXT NOT NULL,
                optional BOOLEAN NOT NULL,
                PRIMARY KEY (update_id, dependency_xml_id),
                FOREIGN KEY (update_id) REFERENCES updates(id) ON DELETE CASCADE
            )
        "#,
            (),
        )
        .await?;

        tx.commit().await?;

        tracing::trace!("Database structure created.");

        Ok(())
    }

    #[tracing::instrument(skip(self))]
    pub async fn known_plugin_xml_ids(&self) -> Result<HashSet<String>, IndexerError> {
        self.connection
            .query("SELECT xml_id FROM plugins", ())
            .await?
            .into_stream()
            .and_then(|r| future::ready(r.get_str(0).map(|v| v.to_string())))
            .map_err(IndexerError::from)
            .try_collect()
            .await
    }

    #[tracing::instrument(skip(self))]
    pub async fn stream_plugins(&self) -> impl Stream<Item = Result<CachedPlugin, IndexerError>> {
        self.connection
            .query("SELECT xml_id, numeric_id FROM plugins", ())
            .await
            .expect("Failed to query plugins")
            .into_stream()
            .map_err(IndexerError::from)
            .and_then(map_row_de)
    }

    #[tracing::instrument(skip_all, fields(plugin_xml_id = xml_id.as_ref()))]
    pub async fn delete_plugin_by_xml_id(
        &self,
        xml_id: impl AsRef<str>,
    ) -> Result<(), IndexerError> {
        self.connection
            .execute("DELETE FROM plugins WHERE xml_id = ?1", [xml_id.as_ref()])
            .map_err(IndexerError::from)
            .await?;

        Ok(())
    }

    #[tracing::instrument(skip(self))]
    pub async fn add_plugin(&self, plugin: &CachedPlugin) -> Result<(), IndexerError> {
        self.connection
            .execute(
                "INSERT INTO plugins (xml_id, numeric_id) VALUES (?1, ?2)",
                libsql::params![plugin.xml_id.as_str(), plugin.numeric_id],
            )
            .map_err(IndexerError::from)
            .await?;

        Ok(())
    }

    #[tracing::instrument(skip(self))]
    pub async fn add_update(&self, update_id: u64) -> Result<(), IndexerError> {
        self.connection
            .execute(
                "INSERT OR IGNORE INTO updates (id) VALUES (?1)",
                libsql::params![update_id],
            )
            .map_err(IndexerError::from)
            .await?;

        Ok(())
    }

    #[tracing::instrument(skip(self))]
    pub async fn add_plugin_version(
        &self,
        version: &CachedPluginVersion,
    ) -> Result<u64, IndexerError> {
        let count = self
            .connection
            .execute(
                r#"
                        INSERT INTO versions
                            (version, update_id, channel, plugin_xml_id)
                        VALUES (?1, ?2, ?3, ?4) ON CONFLICT DO UPDATE SET
                            update_id = ?2, channel = ?3;
                     "#,
                libsql::params![
                    version.version.as_str(),
                    version.update_id,
                    version.channel.as_str(),
                    version.plugin_xml_id.as_str()
                ],
            )
            .map_err(IndexerError::from)
            .await?;

        Ok(count)
    }

    #[tracing::instrument(
        skip_all,
        fields(plugin_xml_id = plugin_xml_id.as_ref())
    )]
    pub async fn get_versions_for_plugin(
        &self,
        plugin_xml_id: impl AsRef<str>,
    ) -> Result<Vec<CachedPluginVersion>, IndexerError> {
        self.connection
            .query("SELECT version, update_id, channel, plugin_xml_id FROM versions WHERE plugin_xml_id = ?1", libsql::params![plugin_xml_id.as_ref()])
            .await?
            .into_stream()
            .map_err(IndexerError::from)
            .and_then(map_row_de)
            .try_collect()
            .await
    }

    #[tracing::instrument(
        skip_all,
        fields(plugin_xml_id = plugin_xml_id.as_ref(), version = version.as_ref())
    )]
    pub async fn remove_plugin_version(
        &self,
        plugin_xml_id: impl AsRef<str>,
        version: impl AsRef<str>,
    ) -> Result<(), IndexerError> {
        self.connection
            .execute(
                "DELETE FROM versions WHERE plugin_xml_id = ?1 AND version = ?2",
                libsql::params![plugin_xml_id.as_ref(), version.as_ref()],
            )
            .map_err(IndexerError::from)
            .await?;

        Ok(())
    }

    #[tracing::instrument(skip(self))]
    pub async fn add_update_dependency(
        &self,
        dependency: &CachedUpdateDependency,
    ) -> Result<(), IndexerError> {
        self.connection
            .execute(
                "INSERT INTO update_dependencies (update_id, dependency_xml_id, optional) VALUES (?1, ?2, ?3) ON CONFLICT DO UPDATE SET dependency_xml_id = ?2, optional = ?3",
                libsql::params![dependency.update_id, dependency.dependency_xml_id.as_str(), dependency.optional],
            )
            .map_err(IndexerError::from)
            .await?;

        Ok(())
    }

    #[tracing::instrument(skip(self))]
    pub async fn mark_all_updates_stale(&self) -> Result<(), IndexerError> {
        self.connection
            .execute("UPDATE updates SET stale = TRUE", ())
            .await?;
        Ok(())
    }

    #[tracing::instrument(skip(self))]
    pub async fn mark_update_not_stale(&self, update_id: u64) -> Result<bool, IndexerError> {
        let affected = self
            .connection
            .execute(
                "UPDATE updates SET stale = FALSE WHERE id = ?1",
                libsql::params![update_id],
            )
            .map_err(IndexerError::from)
            .await?;

        Ok(affected > 0)
    }

    #[tracing::instrument(skip(self))]
    pub async fn get_update(&self, update_id: u64) -> Result<CachedUpdate, IndexerError> {
        self.connection
            .query(
                "SELECT id, stale, etag, file_name, download_url, hash_algorithm, hash FROM updates WHERE id = ?1",
                libsql::params![update_id],
            )
            .await?
            .next()
            .await?
            .map(map_row_de)
            .ok_or(IndexerError::NotFound)?
            .await
    }

    #[tracing::instrument(skip(self))]
    pub async fn change_update_info(&self, update: &CachedUpdate) -> Result<(), IndexerError> {
        self.connection.execute(
            "UPDATE updates SET stale = ?1, etag = ?2, file_name = ?3, download_url = ?4, hash_algorithm = ?5, hash = ?6 WHERE id = ?7",
            libsql::params![
                update.stale,
                update.etag.as_deref(),
                update.file_name.as_deref(),
                update.download_url.as_deref(),
                update.hash_algorithm.as_deref(),
                update.hash.as_deref(),
                update.id
            ],
        ).await?;

        Ok(())
    }
}
