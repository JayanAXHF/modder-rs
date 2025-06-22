mod actions;
mod cli;
pub mod modrinth_wrapper;
use clap::Parser;
use cli::*;
use futures::lock::Mutex;
use modder::*;
use std::{collections::HashSet, process, sync::Arc};
use tracing::{error, info, level_filters::LevelFilter};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    let filter = if cli.silent {
        LevelFilter::ERROR
    } else {
        LevelFilter::INFO
    };
    let env_filter = EnvFilter::builder()
        .with_default_directive(filter.into())
        .from_env_lossy();
    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .compact()
        .init();
    actions::run(cli).await;
    Ok(())
}
