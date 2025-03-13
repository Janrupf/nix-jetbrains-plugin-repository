use std::num::NonZeroUsize;
use std::path::PathBuf;
use clap::Parser;

#[derive(Debug, Clone, Parser)]
pub struct IndexerArgs {
    #[arg(short, long, default_value = "indexer.db", env = "JP_REPO_INDEXER_DB")]
    pub database: PathBuf,

    #[arg(long, default_value = "32")]
    pub max_parallel_small_requests: NonZeroUsize,

    #[arg(long, default_value = "4")]
    pub max_parallel_large_requests: NonZeroUsize,
}