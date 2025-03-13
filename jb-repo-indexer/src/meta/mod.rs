mod sync;

use crate::api::JetbrainsRepoApi;
use crate::args::IndexerArgs;
use crate::db::Database;
use crate::error::IndexerError;
use crate::meta::sync::{sync_new_plugin, sync_plugin};
use crate::statistics::{Statistics, StatisticsCollector, StatisticsSender};
use futures::StreamExt;
use std::collections::HashSet;
use tokio_util::task::TaskTracker;

#[derive(Clone)]
pub struct TaskAttachment {
    database: Database,
    repo: JetbrainsRepoApi,
    tracker: TaskTracker,
    statistics_sender: StatisticsSender,
}

impl TaskAttachment {
    /// Dispatch a new future and record its outcome in the statistics.
    pub fn dispatch<F, E>(&self, name: impl Into<String>, future: F)
    where
        F: Future<Output = Result<(), E>> + Send + 'static,
        E: std::error::Error + Send + 'static,
    {
        let new_fut = self.statistics_sender.guard_future(name.into(), future);
        self.tracker.spawn(new_fut);
    }

    pub fn send_problem(
        &self,
        name: impl Into<String>,
        error: impl std::error::Error + Send + 'static,
    ) {
        self.statistics_sender.send_problem(name, Box::new(error));
    }
}

pub struct MetadataProcessor {
    database: Database,
    repo: JetbrainsRepoApi,
}

impl MetadataProcessor {
    /// Prepare the metadata processor.
    pub async fn new(args: &IndexerArgs) -> Result<Self, IndexerError> {
        let database = Database::setup(args).await?;
        let repo = JetbrainsRepoApi::new(args)?;

        Ok(Self { database, repo })
    }

    pub async fn sync_plugin_metadata(&self) -> Result<Statistics, IndexerError> {
        let (local, remote, _) = futures::try_join!(
            self.database.known_plugin_xml_ids(),
            self.repo.fetch_all_xml_ids(),
            self.database.mark_all_updates_stale()
        )?;

        self.purge_unknown_plugins(&local, &remote).await?;

        let mut statistics = StatisticsCollector::new();

        let attachment = self.attachment(statistics.sender());

        // Dispatch the initial tasks for syncing all plugins
        attachment.dispatch("dispatch plugin sync", {
            let attachment = attachment.clone();

            async move {
                let plugins_stream = attachment.database.stream_plugins().await;
                tokio::pin!(plugins_stream);

                while let Some(next) = plugins_stream.next().await {
                    let plugin = match next {
                        Ok(v) => v,
                        Err(err) => {
                            attachment.send_problem("dispatch plugin sync", err);
                            continue;
                        }
                    };

                    attachment.dispatch(
                        format!("sync plugin {}", plugin.xml_id),
                        sync_plugin(attachment.clone(), plugin),
                    );
                }

                tracing::trace!("Dispatched all known plugins");

                Ok::<(), std::convert::Infallible>(())
            }
        });

        attachment.dispatch("sync all new plugins", {
            let attachment = attachment.clone();

            async move {
                Self::sync_new_plugins(&local, &remote, attachment)
            }
        });

        // Wait for everything to finish
        attachment.tracker.close();
        let tracker_wait_fut = attachment.tracker.wait();
        let statistics_wait_fut = statistics.run();

        // The statistics future never finishes either way, so we effectively wait for the tracker
        // in this select, but also poll the statistics future.
        tokio::select! {
            _ = tracker_wait_fut => {},
            _ = statistics_wait_fut => {},
        }

        Ok(statistics.reset())
    }

    async fn purge_unknown_plugins(
        &self,
        local: &HashSet<String>,
        remote: &HashSet<String>,
    ) -> Result<(), IndexerError> {
        let all_disappeared = local.difference(remote);

        for disappeared in all_disappeared {
            tracing::info!("Plugin disappeared: {}", disappeared);
            self.database.delete_plugin_by_xml_id(disappeared).await?;
        }

        Ok(())
    }

    fn sync_new_plugins(
        local: &HashSet<String>,
        remote: &HashSet<String>,
        attachment: TaskAttachment,
    ) -> Result<(), IndexerError> {
        let all_new = remote.difference(local);

        for new in all_new {
            attachment.dispatch(
                format!("sync new plugin {}", new),
                sync_new_plugin(attachment.clone(), new.clone()),
            );
        }

        Ok(())
    }

    fn attachment(&self, statistics_sender: StatisticsSender) -> TaskAttachment {
        TaskAttachment {
            database: self.database.clone(),
            repo: self.repo.clone(),
            tracker: TaskTracker::new(),
            statistics_sender,
        }
    }
}
