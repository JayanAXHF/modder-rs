use clap::Parser;
use futures::lock::Mutex;
use modder::*;
use std::{collections::HashSet, sync::Arc};
use tracing::info;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
/// A tool to download/update mods from modrinth
struct Args {
    /// Update an entire directory of mods
    #[arg(short, long)]
    update_dir: Option<String>,
    /// The game version for which to download the mods
    #[arg(short, long)]
    game_version: Option<String>,

    /// Delete the previous version of the mod when updating
    #[arg(short, long, default_value_t = false)]
    delete_previous: bool,

    /// The number of mods to fetch from modrinth
    #[arg(short, long, default_value_t = 100)]
    limit: u16,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    let dependencies = Arc::new(Mutex::new(Vec::new()));

    let args = Args::parse();
    let version = if let Some(version) = args.game_version {
        version
    } else {
        inquire::Text::new("Version").prompt().unwrap()
    };

    if let Some(update_dir) = args.update_dir {
        modder::update_dir(&update_dir, &version, args.delete_previous).await;
        return;
    }
    let limit = args.limit;
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
            slug: "easy-auth".into(),
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
                download_file(&version_data.clone().files.unwrap()[0]).await;
                download_dependencies(&mod_, &version, dependencies).await;
            }
        });
        handles.push(handle);
    }
    for handle in handles {
        handle.await.unwrap();
    }
}
