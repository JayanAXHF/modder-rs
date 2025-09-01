use color_eyre::eyre::Result;
mod actions;
use clap::Parser;
use cli::*;
use futures::lock::Mutex;
use modder::*;
use std::{collections::HashSet, sync::Arc};
use tracing::{error, info, level_filters::LevelFilter};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    #[cfg(not(debug_assertions))]
    {
        color_eyre::install()?;
    }
    #[cfg(debug_assertions)]
    {
        std::panic::set_hook(Box::new(move |panic_info| {
            better_panic::Settings::auto()
                .most_recent_first(false)
                .lineno_suffix(true)
                .verbosity(better_panic::Verbosity::Full)
                .create_panic_handler()(panic_info);
        }));
    }
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
    let res = actions::run(cli).await;
    if let Err(err) = res {
        error!("{err}");
        return Err(err);
    }
    Ok(())
}
