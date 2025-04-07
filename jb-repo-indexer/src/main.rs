mod api;
mod args;
mod db;
mod error;
mod meta;
mod statistics;

use crate::args::IndexerArgs;
use crate::error::IndexerError;
use crate::meta::MetadataProcessor;
use clap::Parser as _;
use tracing_subscriber::layer::SubscriberExt as _;
use tracing_subscriber::util::SubscriberInitExt as _;

fn main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::from_env(
            "JB_REPO_INDEXER_LOG",
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let args = args::IndexerArgs::parse();

    let result = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
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

    if !args.no_sync {
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
    }

    if !args.no_generate {
        tracing::info!("Starting to generate metadata...");
        processor.generate_metadata().await?;
        tracing::info!("Done.");
    }

    Ok(())
}
