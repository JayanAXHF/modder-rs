mod actions;
mod cli;
use clap::Parser;
use cli::*;
use futures::lock::Mutex;
use modder::*;
use serde_json::Result;
use std::{collections::HashSet, process, sync::Arc};
use tracing::{error, info};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();
    actions::run(cli).await;
}
