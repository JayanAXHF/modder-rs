use crate::modrinth_wrapper::modrinth::Mod;
use cli::Source;
use color_eyre::eyre::bail;
use colored::Colorize;
use curseforge_wrapper::{API_KEY, CurseForgeAPI, CurseForgeMod};
use gh_releases::GHReleasesAPI;
use itertools::Itertools;
use metadata::Metadata;
use modrinth_wrapper::modrinth::{self, VersionData};
use modrinth_wrapper::modrinth::{GetProject, Modrinth};
use percent_encoding::percent_decode;
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use tabwriter::TabWriter;
use tokio::task::JoinHandle;

use crate::*;
const GRAY: (u8, u8, u8) = (128, 128, 128);

pub async fn run(cli: Cli) -> color_eyre::Result<()> {
    let dependencies = Arc::new(Mutex::new(Vec::new()));
    match cli.command {
        Commands::QuickAdd {
            version,
            limit,
            loader,
        } => {
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
                let loader = loader.clone();
                let handle = tokio::spawn(async move {
                    let version_data =
                        Modrinth::get_version(&mod_.slug, &version, loader.clone()).await;
                    if let Some(version_data) = version_data {
                        info!("Downloading {}", mod_.title);
                        modrinth::download_file(&version_data.clone().files.unwrap()[0], "./")
                            .await;
                        Modrinth::download_dependencies(
                            &mod_,
                            &version,
                            dependencies,
                            "./",
                            loader,
                        )
                        .await;
                    }
                });
                handles.push(handle);
            }
            for handle in handles {
                handle.await.unwrap();
            }
            return Ok(());
        }
        Commands::Update {
            dir,
            version,
            delete_previous,
            token,
            source,
            other_sources,
            loader,
        } => {
            let version = if let Some(version) = version {
                version
            } else {
                inquire::Text::new("Version").prompt().unwrap()
            };
            let update_dir = dir.into_os_string().into_string().unwrap();
            let mut github = GHReleasesAPI::new();
            if let Some(token) = token {
                github.token(token.clone());
            }
            let curseforge = CurseForgeAPI::new(API_KEY.to_string());

            modder::update_dir(
                &mut github,
                curseforge,
                &update_dir,
                &version,
                delete_previous,
                &update_dir,
                source,
                other_sources,
                loader,
            )
            .await?;
        }
        Commands::Add {
            mod_,
            version,
            source,
            token,
            loader,
            dir,
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
            match source {
                Source::Github => {
                    let mod_ = mod_.split('/').collect_vec();
                    let mut gh = GHReleasesAPI::new();
                    if let Some(token) = token {
                        gh.token(token);
                    }
                    let releases = gh.get_releases(mod_[0], mod_[1]).await.unwrap();
                    //  TODO: Add support for other loaders
                    let release =
                        gh_releases::get_mod_from_release(&releases, "fabric", &version).await?;
                    let url = release.get_download_url().unwrap();
                    let file_name =
                        percent_decode(url.path_segments().unwrap().last().unwrap().as_bytes())
                            .decode_utf8_lossy()
                            .to_string();
                    let path = format!("./{}", file_name);
                    info!("Downloading {}", file_name);
                    release
                        .download(path.clone().into(), mod_.join("/"))
                        .await?;
                }
                Source::Modrinth => {
                    let res = Modrinth::search_mods(&mod_, 100, 0).await;
                    let hits = res.hits;
                    if hits.is_empty() {
                        bail!("Could not find mod {}", mod_);
                    }
                    if hits.len() == 1 {
                        let mod_ = hits[0].clone();
                        let version_data =
                            Modrinth::get_version(&mod_.slug, &version, loader.clone()).await;
                        if let Some(version_data) = version_data {
                            info!("Downloading {}", mod_.title);
                            modrinth::download_file(&version_data.clone().files.unwrap()[0], "./")
                                .await;
                            Modrinth::download_dependencies(
                                &mod_.into(),
                                &version,
                                dependencies.clone(),
                                "./",
                                loader,
                            )
                            .await;
                            return Ok(());
                        } else {
                            info!(
                                "Could not find version {} for {}, trying curseforge",
                                version, mod_.title
                            );
                            bail!("Could not find version {} for {}", version, mod_.title);
                        }
                    }
                    let prompt = inquire::MultiSelect::new("Select Mods", hits);
                    let hits = prompt.prompt().unwrap();
                    let mut handles = Vec::new();
                    for hit in hits {
                        let loader = loader.clone();
                        let version = version.clone();
                        let dependencies = Arc::clone(&dependencies);
                        let handle = tokio::spawn(async move {
                            let version_data =
                                Modrinth::get_version(&hit.slug, &version, loader.clone()).await;
                            if let Some(version_data) = version_data {
                                info!("Downloading {}", hit.title);
                                modrinth::download_file(
                                    &version_data.clone().files.unwrap()[0],
                                    "./",
                                )
                                .await;
                                Modrinth::download_dependencies(
                                    &hit.into(),
                                    &version,
                                    dependencies,
                                    "./",
                                    loader,
                                )
                                .await;
                            } else {
                                bail!("Could not find version {} for {}", version, hit.title);
                            }
                            Ok(())
                        });

                        handles.push(handle);
                    }
                    for handle in handles {
                        handle.await??;
                    }
                }
                Source::CurseForge => {
                    let api = CurseForgeAPI::new(API_KEY.to_string());
                    let dependencies = Arc::new(Mutex::new(Vec::new()));
                    let mods = api.search_mods(&version, loader, &mod_, 30).await.unwrap();
                    let prompt = inquire::MultiSelect::new("Select mods", mods);
                    let selected = prompt.prompt().unwrap();
                    let mut handles = Vec::new();
                    let dir = Arc::new(dir.clone());
                    for mod_ in selected {
                        let dependencies = Arc::clone(&dependencies);
                        let version = version.clone();
                        let api = api.clone();
                        let dir = dir.clone();
                        let handle: JoinHandle<Result<()>> = tokio::spawn(async move {
                            info!("Downloading {}", mod_.name);
                            let v = mod_.get_version_and_loader(&version).unwrap();

                            api.download_mod(mod_.id, v.file_id, dir.to_path_buf())
                                .await?;
                            let deps = api.get_dependencies(mod_.id, &version).await?;
                            for dep in deps {
                                if dependencies.lock().await.contains(&dep.id) {
                                    info!("Skipping dependency {}", dep.name);
                                }
                                info!("Downloading dependency {}", dep.name);
                                let v = dep.get_version_and_loader(&version).unwrap();
                                api.download_mod(dep.id, v.file_id, dir.to_path_buf())
                                    .await?;
                            }
                            Ok(())
                        });
                        handles.push(handle);
                    }
                    for handle in handles {
                        handle.await?.unwrap();
                    }
                }
            }
        }
        Commands::Toggle { version: _, dir } => toggle(dir)?,
        Commands::List { dir, verbose } => {
            let files = fs::read_dir(dir).unwrap();

            let mut output = String::new();
            let mut handles = Vec::new();
            for f in files {
                let handle = tokio::spawn(async move {
                    let Ok(f) = f else {
                        return None;
                    };
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
                    let version_data = VersionData::from_hash(hash).await;
                    if version_data.is_err() {
                        println!("   ");
                        let metadata = Metadata::get_all_metadata(path_str.clone().into());
                        if metadata.is_err() {
                            return None;
                        }
                        let Ok(metadata) = metadata else {
                            return None;
                        };
                        let source = metadata.get("source")?;
                        if source.is_empty() {
                            return None;
                        }
                        let repo = metadata.get("repo")?;
                        let repo_name = repo.split('/').last()?;
                        let link = Link::new(
                            repo_name.to_string(),
                            format!("https://github.com/{}", repo),
                        );
                        let out = if verbose {
                            format!(
                                "{}  {}  {}",
                                "GITHUB".yellow(),
                                repo.truecolor(GRAY.0, GRAY.1, GRAY.2),
                                link.to_string().bold()
                            )
                        } else {
                            format!(
                                "{}\t{}\t{}",
                                "GITHUB".yellow(),
                                repo.truecolor(GRAY.0, GRAY.1, GRAY.2),
                                link.to_string().bold()
                            )
                        };
                        return Some(out);
                    }
                    let Ok(version_data) = version_data else {
                        return None;
                    };
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
                let out = match handle.await {
                    Ok(out) => out,
                    Err(_) => continue,
                };
                output.push_str(&out.unwrap_or_default());
            }

            let mut tw = TabWriter::new(vec![]);
            tw.write_all(output.as_bytes()).unwrap();
            tw.flush().unwrap();
            let written = String::from_utf8(tw.into_inner().unwrap()).unwrap();
            println!("{}", written);
        }
    };
    Ok(())
}

fn toggle(dir: PathBuf) -> color_eyre::Result<()> {
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

    let prompt = inquire::MultiSelect::new("Select Mods", filenames).with_default(&defaults);
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
    Ok(())
}
