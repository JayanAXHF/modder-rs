use clap::{Parser, Subcommand};
use futures::lock::Mutex;
use modder::*;
use serde_json::Result;
use std::{collections::HashSet, path::PathBuf, process, sync::Arc};
use tracing::{error, info};

#[derive(Debug, Parser)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    #[command(arg_required_else_help = true)]
    Add {
        #[arg(required = true)]
        mod_: String,
        #[arg(short, long)]
        mod_version: Option<String>,
    },
    #[command(arg_required_else_help = true)]
    Update {
        #[arg(required = true)]
        dir: PathBuf,
        #[arg(short, long)]
        mod_version: Option<String>,
        #[arg(short, long)]
        delete_previous: bool,
    },
    QuickAdd {
        #[arg(short, long)]
        mod_version: Option<String>,
        #[arg(short, long, default_value_t = 100)]
        limit: u16,
    },
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    let dependencies = Arc::new(Mutex::new(Vec::new()));

    let cli = Cli::parse();
    match cli.command {
        Commands::QuickAdd { mod_version, limit } => {
            let version = if let Some(version) = mod_version {
                version
            } else {
                inquire::Text::new("Version").prompt().unwrap()
            };
            let mods = get_top_mods(limit).await;
            let mods = mods
                .into_iter()
                .map(|mod_| mod_.into())
                .collect::<Vec<Mod>>();
            let extras = vec![
                Mod {
                    slug: "anti-xray".into(),
                    title: "Anti Xray".into(),
                },
                Mod {
                    slug: "appleskin".into(),
                    title: "Apple Skin".into(),
                },
                Mod {
                    slug: "carpet-extra".into(),
                    title: "Carpet Extra".into(),
                },
                Mod {
                    slug: "easyauth".into(),
                    title: "Easy Auth".into(),
                },
                Mod {
                    slug: "essential-commands".into(),
                    title: "Essential Commands".into(),
                },
                Mod {
                    slug: "fabric-carpet".into(),
                    title: "Fabric Carpet".into(),
                },
                Mod {
                    slug: "geyser".into(),
                    title: "Geyser".into(),
                },
                Mod {
                    slug: "origins".into(),
                    title: "Origins".into(),
                },
                Mod {
                    slug: "skinrestorer".into(),
                    title: "Skin Restorer".into(),
                },
                Mod {
                    slug: "status".into(),
                    title: "Status".into(),
                },
            ];
            let mods = mods
                .into_iter()
                .chain(extras.into_iter())
                .collect::<HashSet<Mod>>();
            let mods = mods.into_iter().collect::<Vec<Mod>>();
            let prompt = inquire::MultiSelect::new("Select Mods", mods);
            let mods = prompt.prompt().unwrap();
            let mut handles = Vec::new();
            for mod_ in mods {
                let version = version.clone();
                let dependencies = Arc::clone(&dependencies);
                let handle = tokio::spawn(async move {
                    let version_data = get_version(&mod_.slug, &version).await;
                    if let Some(version_data) = version_data {
                        info!("Downloading {}", mod_.title);
                        download_file(&version_data.clone().files.unwrap()[0], "./").await;
                        download_dependencies(&mod_, &version, dependencies, "./").await;
                    }
                });
                handles.push(handle);
            }
            for handle in handles {
                handle.await.unwrap();
            }
        }
        Commands::Update {
            dir,
            mod_version,
            delete_previous,
        } => {
            let version = if let Some(version) = mod_version {
                version
            } else {
                inquire::Text::new("Version").prompt().unwrap()
            };
            let update_dir = dir.into_os_string().into_string().unwrap();
            modder::update_dir(&update_dir, &version, delete_previous, &update_dir).await;
        }
        Commands::Add { mod_, mod_version } => {
            let version = if let Some(version) = mod_version {
                version
            } else {
                inquire::Text::new("Version").prompt().unwrap()
            };
            let client = reqwest::Client::new();
            let res = client
                .get(format!("https://api.modrinth.com/v2/search?query={}", mod_))
                .send()
                .await
                .unwrap();
            let res = res.text().await.unwrap();

            let res: Result<ProjectSearch> = serde_json::from_str(&res);
            let res = res.unwrap();
            let hits = res.hits;
            if hits.is_empty() {
                error!("Could not find mod {}", mod_);
                process::exit(1);
            }
            if hits.len() == 1 {
                let mod_ = hits[0].clone();
                let version_data = get_version(&mod_.slug, &version).await;
                if let Some(version_data) = version_data {
                    info!("Downloading {}", mod_.title);
                    download_file(&version_data.clone().files.unwrap()[0], "./").await;
                    download_dependencies(&mod_.into(), &version, dependencies, "./").await;
                } else {
                    error!("Could not find version {} for {}", version, mod_.title);
                    process::exit(1);
                }
                return;
            }
            let prompt = inquire::MultiSelect::new("Select Mods", hits);
            let hits = prompt.prompt().unwrap();
            let mut handles = Vec::new();
            for hit in hits {
                let version = version.clone();
                let dependencies = Arc::clone(&dependencies);
                let handle = tokio::spawn(async move {
                    let version_data = get_version(&hit.slug, &version).await;
                    if let Some(version_data) = version_data {
                        info!("Downloading {}", hit.title);
                        download_file(&version_data.clone().files.unwrap()[0], "./").await;
                        download_dependencies(&hit.into(), &version, dependencies, "./").await;
                    } else {
                        error!("Could not find version {} for {}", version, hit.title);
                        process::exit(1);
                    }
                });
                handles.push(handle);
            }
            for handle in handles {
                handle.await.unwrap();
            }
        }
    }
}
