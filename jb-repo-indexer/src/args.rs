use std::num::NonZeroUsize;
use std::path::PathBuf;
use clap::Parser;

#[derive(Debug, Clone, Parser)]
pub struct IndexerArgs {
    #[arg(short, long, default_value = "indexer.db", env = "JB_REPO_INDEXER_DB")]
    pub database: PathBuf,

    #[arg(long, default_value = "32")]
    pub max_parallel_small_requests: NonZeroUsize,

    #[arg(long, default_value = "4")]
    pub max_parallel_large_requests: NonZeroUsize,

    #[arg(short, long, default_value = "meta", env = "JB_REPO_INDEXER_OUTPUT_DIRECTORY")]
    pub output_directory: PathBuf,

    #[arg(long, default_value_t = false)]
    pub no_sync: bool,

    #[arg(long, default_value_t = false)]
    pub no_generate: bool,
}
