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
            CREATE TABLE IF NOT EXISTS schema_version (
                version INTEGER NOT NULL
            )
        "#,
            (),
        )
        .await?;

        let current_schema_version = tx
            .query("SELECT version FROM schema_version", ())
            .await?
            .next()
            .await?
            .and_then(|v| v.get::<u64>(0).ok())
            .unwrap_or(0);

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

        if current_schema_version < 2 {
            // Just completely drop the versions table, it gets re-generated anyway
            tx.execute("DROP TABLE IF EXISTS versions", ()).await?;
        }

        tx.execute(
            r#"
            CREATE TABLE IF NOT EXISTS versions (
                version TEXT NOT NULL,
                update_id INTEGER NOT NULL,
                channel TEXT NOT NULL,
                plugin_xml_id TEXT NOT NULL,
                since TEXT,
                until TEXT,
                PRIMARY KEY (update_id, plugin_xml_id),
                FOREIGN KEY (update_id) REFERENCES updates(id) ON DELETE CASCADE,
                FOREIGN KEY (plugin_xml_id) REFERENCES plugins(xml_id) ON DELETE CASCADE
            )
        "#,
            (),
        )
        .await?;

        tx.execute(
            r#"
            CREATE TABLE IF NOT EXISTS broken_plugins (
                plugin_xml_id TEXT NOT NULL,
                version TEXT NOT NULL,
                original_since TEXT,
                original_until TEXT,
                since TEXT,
                until TEXT
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

        tx.execute("DELETE FROM schema_version", ()).await?;

        tx.execute(
            r#"
            INSERT INTO schema_version (version) VALUES (2)
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

    #[tracing::instrument(skip(self))]
    pub async fn get_all_plugins(&self) -> Result<Vec<CachedPlugin>, IndexerError> {
        self.connection
            .query("SELECT xml_id, numeric_id FROM plugins", ())
            .await?
            .into_stream()
            .map_err(IndexerError::from)
            .and_then(map_row_de)
            .try_collect()
            .await
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
                "INSERT INTO plugins (xml_id, numeric_id) VALUES (?1, ?2) ON CONFLICT DO UPDATE SET numeric_id = ?2",
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
                            (version, update_id, channel, plugin_xml_id, since, until)
                        VALUES (?1, ?2, ?3, ?4, ?5, ?6) ON CONFLICT DO UPDATE SET
                            version = ?1, channel = ?3, since = ?5, until = ?6;
                     "#,
                libsql::params![
                    version.version.as_str(),
                    version.update_id,
                    version.channel.as_str(),
                    version.plugin_xml_id.as_str(),
                    version.since.as_deref(),
                    version.until.as_deref(),
                ],
            )
            .map_err(IndexerError::from)
            .await?;

        Ok(count)
    }

    #[tracing::instrument(skip_all, fields(
        plugin_xml_id = plugin_xml_id.as_ref(),
        version = version.as_ref(),
        since = since.as_ref().map(|v| v.as_ref()),
        until = until.as_ref().map(|v| v.as_ref()),
    ))]
    pub async fn set_version_compat_range(
        &self,
        plugin_xml_id: impl AsRef<str>,
        version: impl AsRef<str>,
        since: Option<impl AsRef<str>>,
        until: Option<impl AsRef<str>>,
    ) -> Result<(), IndexerError> {
        self.connection.execute(
            "UPDATE versions SET since = ?3, until = ?4 WHERE plugin_xml_id = ?1 AND version = ?2",
            libsql::params![
                plugin_xml_id.as_ref(),
                version.as_ref(),
                since.as_ref().map(|v| v.as_ref()),
                until.as_ref().map(|v| v.as_ref()),
            ],
        ).await?;
        Ok(())
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
            .query("SELECT version, update_id, channel, plugin_xml_id, since, until FROM versions WHERE plugin_xml_id = ?1", libsql::params![plugin_xml_id.as_ref()])
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
    pub async fn get_update_dependencies(
        &self,
        update_id: u64,
    ) -> Result<Vec<CachedUpdateDependency>, IndexerError> {
        self.connection
            .query(
                "SELECT update_id, dependency_xml_id, optional FROM update_dependencies WHERE update_id = ?1",
                libsql::params![update_id],
            )
            .await?
            .into_stream()
            .map_err(IndexerError::from)
            .and_then(map_row_de)
            .try_collect()
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

    #[tracing::instrument(skip(self))]
    pub async fn clear_broken_plugins(&self) -> Result<(), IndexerError> {
        self.connection
            .execute("DELETE FROM broken_plugins", ())
            .await?;
        Ok(())
    }

    #[tracing::instrument(skip(self))]
    pub async fn add_broken_plugin(&self, plugin: &CachedBrokenPlugin) -> Result<(), IndexerError> {
        self.connection
            .execute(
                r#"
                INSERT INTO broken_plugins
                    (plugin_xml_id, version, original_since, original_until, since, until)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6)"#,
                libsql::params![
                    plugin.plugin_xml_id.as_str(),
                    plugin.version.as_str(),
                    plugin.original_since.as_deref(),
                    plugin.original_until.as_deref(),
                    plugin.since.as_deref(),
                    plugin.until.as_deref(),
                ],
            )
            .await?;

        Ok(())
    }

    #[tracing::instrument(skip_all, fields(plugin_xml_id = plugin_xml_id.as_ref(), version = version.as_ref()))]
    pub async fn get_broken_plugin_info(
        &self,
        plugin_xml_id: impl AsRef<str>,
        version: impl AsRef<str>,
    ) -> Result<Vec<CachedBrokenPlugin>, IndexerError> {
        self.connection
            .query(
                r#"
                SELECT plugin_xml_id, version, original_since, original_until, since, until FROM broken_plugins
                    WHERE plugin_xml_id = ?1 AND version = ?2
                "#,
                libsql::params![plugin_xml_id.as_ref(), version.as_ref()]
            ).await?
            .into_stream()
            .map_err(IndexerError::from)
            .and_then(map_row_de)
            .try_collect()
            .await
    }
}
