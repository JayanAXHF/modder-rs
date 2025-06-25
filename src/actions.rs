use crate::modrinth_wrapper::modrinth::Mod;
use cli::Source;
use gh_releases::GHReleasesAPI;
use itertools::Itertools;
use metadata::Metadata;
use modder::get_minecraft_dir;
use modrinth_wrapper::modrinth::{self, VersionData};
use modrinth_wrapper::modrinth::{GetProject, Modrinth};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use tabwriter::TabWriter;

use crate::*;

pub async fn run(mut cli: Cli) {
    let dependencies = Arc::new(Mutex::new(Vec::new()));
    let default_minecraft_dir: std::path::PathBuf = get_minecraft_dir();
    if let Commands::InPlace { version, limit } = &cli.command {
        let options = vec![
            Commands::QuickAdd {
                version: version.clone(),
                limit: *limit,
            },
            Commands::Update {
                dir: default_minecraft_dir.clone(),
                version: version.clone(),
                delete_previous: false,
            },
            Commands::Add {
                mod_: String::new(),
                version: version.clone(),
                source: None,
            },
            Commands::Toggle {
                version: version.clone(),
                dir: default_minecraft_dir,
            },
        ];
        let options = options.into_iter().collect::<Vec<Commands>>();
        let prompt = inquire::Select::new("Select Option", options);
        let option = prompt.prompt().unwrap();
        cli.command = option;
    }
    match cli.command {
        Commands::QuickAdd { version, limit } => {
            let version = if let Some(version) = version {
                version
            } else {
                inquire::Text::new("Version").prompt().unwrap()
            };
            let mods: Vec<modrinth::Project> = Modrinth::get_top_mods(limit).await;
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
                    let version_data = Modrinth::get_version(&mod_.slug, &version).await;
                    if let Some(version_data) = version_data {
                        info!("Downloading {}", mod_.title);
                        modrinth::download_file(&version_data.clone().files.unwrap()[0], "./")
                            .await;
                        Modrinth::download_dependencies(&mod_, &version, dependencies, "./").await;
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
            version,
            delete_previous,
        } => {
            let version = if let Some(version) = version {
                version
            } else {
                inquire::Text::new("Version").prompt().unwrap()
            };
            let update_dir = dir.into_os_string().into_string().unwrap();
            modder::update_dir(&update_dir, &version, delete_previous, &update_dir).await;
        }
        Commands::Add {
            mod_,
            version,
            source,
        } => {
            let version = if let Some(version) = version {
                version
            } else {
                inquire::Text::new("Version").prompt().unwrap()
            };
            let source = match source {
                Some(source) => source,
                None => {
                    if mod_.contains('/') {
                        Source::Github
                    } else {
                        Source::Modrinth
                    }
                }
            };
            if source == Source::Github {
                let mod_ = mod_.split('/').collect_vec();
                let gh = GHReleasesAPI::new();
                let releases = gh.get_releases(mod_[0], mod_[1]).await.unwrap();
                //  TODO: Add support for other loaders
                let release =
                    gh_releases::get_mod_from_release(&releases, "fabric", &version).await;
                if let Ok(release) = release {
                    let url = release.get_download_url().unwrap();
                    let file_name = url.path_segments().unwrap().last().unwrap();
                    let path = format!("./{}", file_name);
                    info!("Downloading {}", file_name);
                    release
                        .download(path.clone().into(), mod_.join("/"))
                        .await
                        .unwrap();
                } else {
                    error!(err=?release.err().unwrap().to_string(), "Error finding or downloading mod");
                }
                return;
            }
            let res = Modrinth::search_mods(&mod_, 100, 0).await;
            let hits = res.hits;
            if hits.is_empty() {
                error!("Could not find mod {}", mod_);
                process::exit(1);
            }
            if hits.len() == 1 {
                let mod_ = hits[0].clone();
                let version_data = Modrinth::get_version(&mod_.slug, &version).await;
                if let Some(version_data) = version_data {
                    info!("Downloading {}", mod_.title);
                    modrinth::download_file(&version_data.clone().files.unwrap()[0], "./").await;
                    Modrinth::download_dependencies(&mod_.into(), &version, dependencies, "./")
                        .await;
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
                    let version_data = Modrinth::get_version(&hit.slug, &version).await;
                    if let Some(version_data) = version_data {
                        info!("Downloading {}", hit.title);
                        modrinth::download_file(&version_data.clone().files.unwrap()[0], "./")
                            .await;
                        Modrinth::download_dependencies(&hit.into(), &version, dependencies, "./")
                            .await;
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
        Commands::Toggle { version: _, dir } => {
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

            let prompt =
                inquire::MultiSelect::new("Select Mods", filenames).with_default(&defaults);
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
            version: _,
            limit: _,
        } => {
            unreachable!()
        }
        Commands::List { dir, verbose } => {
            let files = fs::read_dir(dir).unwrap();

            let mut output = String::new();
            let mut handles = Vec::new();
            for f in files {
                let handle = tokio::spawn(async move {
                    if f.is_err() {
                        return None;
                    }
                    let f = f.unwrap();
                    let path = f.path();
                    let extension = path
                        .extension()
                        .unwrap_or_default()
                        .to_str()
                        .unwrap_or_default();

                    if extension != "jar" && extension != "disabled" {
                        return None;
                    }

                    let path_str = path.to_str().unwrap_or_default().to_string();
                    let hash = calc_sha512(&path_str);
                    let version_data = VersionData::from_hash(hash).await.unwrap();
                    let project = GetProject::from_id(&version_data.project_id).await?;
                    let out = if verbose {
                        version_data.format_verbose(&project.get_title(), &project.get_categories())
                    } else {
                        version_data.format(&project.get_title())
                    };
                    Some(out)
                });
                handles.push(handle);
            }
            for handle in handles {
                let out = handle.await.unwrap();
                output.push_str(&out.unwrap_or_default());
            }

            let mut tw = TabWriter::new(vec![]);
            tw.write_all(output.as_bytes()).unwrap();
            tw.flush().unwrap();
            let written = String::from_utf8(tw.into_inner().unwrap()).unwrap();
            println!("{}", written);
        }
    }
}
