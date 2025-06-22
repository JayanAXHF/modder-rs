use modder::get_minecraft_dir;
use std::collections::HashMap;
use std::fs;

use crate::*;

pub async fn run(mut cli: Cli) {
    let dependencies = Arc::new(Mutex::new(Vec::new()));
    let default_minecraft_dir: std::path::PathBuf = get_minecraft_dir();
    if let Commands::InPlace { mod_version, limit } = &cli.command {
        let options = vec![
            Commands::QuickAdd {
                mod_version: mod_version.clone(),
                limit: *limit,
            },
            Commands::Update {
                dir: default_minecraft_dir.clone(),
                mod_version: mod_version.clone(),
                delete_previous: false,
            },
            Commands::Add {
                mod_: String::new(),
                mod_version: mod_version.clone(),
            },
            Commands::Toggle {
                mod_version: mod_version.clone(),
                dir: default_minecraft_dir,
            },
        ];
        let options = options.into_iter().collect::<Vec<Commands>>();
        let prompt = inquire::Select::new("Select Option", options).with_vim_mode(true);
        let option = prompt.prompt().unwrap();
        cli.command = option;
    }
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
            let prompt = inquire::MultiSelect::new("Select Mods", mods).with_vim_mode(true);
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
            let prompt = inquire::MultiSelect::new("Select Mods", hits).with_vim_mode(true);
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
        Commands::Toggle {
            mod_version: _,
            dir,
        } => {
            let files = fs::read_dir(dir.clone()).unwrap();
            let toggle_map = files.map(|f| {
                let f = f.unwrap();
                let path = f.path().to_str().unwrap().to_string();
                let file_name = f.file_name().to_string_lossy().to_string();
                if file_name.ends_with(".disabled") {
                    (path, false)
                } else {
                    (path, true)
                }
            });
            let toggle_map = toggle_map.collect::<HashMap<_, _>>();
            let filenames = toggle_map
                .keys()
                .map(|f| f.split('/').last().unwrap())
                .collect::<Vec<&str>>();
            let defaults = filenames
                .iter()
                .enumerate()
                .filter_map(|(i, f)| {
                    let path = &format!("{}{}", dir.to_str().unwrap(), f);
                    if *toggle_map.get(path).unwrap() {
                        Some(i)
                    } else {
                        None
                    }
                })
                .collect::<Vec<usize>>();

            let prompt = inquire::MultiSelect::new("Select Mods", filenames)
                .with_default(&defaults)
                .with_vim_mode(true);
            let filenames = prompt.prompt().unwrap();
            for filename in toggle_map.iter() {
                let name = &filename.0.split('/').last().unwrap_or("");
                let predicate = !filenames.contains(name);
                let path = filename.0.clone();
                if predicate {
                    if !path.ends_with(".disabled") {
                        fs::rename(&path, format!("{}.disabled", path)).unwrap();
                    }
                    continue;
                }
                if path.ends_with(".disabled") {
                    fs::rename(&path, path.replace(".disabled", "")).unwrap();
                }
            }
        }
        Commands::InPlace {
            mod_version: _,
            limit: _,
        } => {
            unreachable!()
        }
    }
}
