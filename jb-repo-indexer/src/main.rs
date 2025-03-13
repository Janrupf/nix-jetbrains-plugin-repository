mod error;
mod db;
mod args;
mod api;
mod meta;
mod statistics;

use clap::Parser as _;
use tracing_subscriber::layer::SubscriberExt as _;
use tracing_subscriber::util::SubscriberInitExt as _;
use crate::args::IndexerArgs;
use crate::error::IndexerError;
use crate::meta::MetadataProcessor;

fn main() {
    let indicatif_layer = tracing_indicatif::IndicatifLayer::new();

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::from_env(
            "JP_REPO_INDEXER_LOG",
        ))
        .with(tracing_subscriber::fmt::layer().with_writer(indicatif_layer.get_stdout_writer()))
        .with(indicatif_layer)
        .init();

    let args = args::IndexerArgs::parse();

    let result = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build() {
        Ok(v) => v.block_on(async_main(args)),
        Err(err) => {
            tracing::error!("Failed to create tokio runtime: {:?}", err);
            std::process::exit(1);
        }
    };

    if let Err(err) = result {
        tracing::error!("Error: {:?}", err);
        std::process::exit(1);
    }
}

async fn async_main(args: IndexerArgs) -> Result<(), IndexerError> {
    tracing::trace!("args = {:#?}", args);

    let processor = MetadataProcessor::new(&args).await?;

    tracing::info!("Starting to sync plugin metadata...");
    let statistics = processor.sync_plugin_metadata().await?;

    tracing::info!("Done.");

    if !statistics.problems.is_empty() {
        tracing::warn!("Problems encountered:");
        for problem in &statistics.problems {
            tracing::warn!("- {}: {}", problem.task_name, problem.error);
        }
    }

    if !statistics.failures.is_empty() {
        tracing::error!("Failed tasks:");
        for failure in &statistics.failures {
            tracing::error!("- {}: {}", failure.task_name, failure.error);
        }
    }

    tracing::info!("Encountered problems: {}", statistics.problems.len());
    tracing::info!("Failed tasks: {}", statistics.failures.len());
    tracing::info!("Succeeded tasks: {}", statistics.successful_tasks);

    Ok(())
}
