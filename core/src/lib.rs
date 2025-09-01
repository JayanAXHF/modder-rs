#![allow(dead_code)]
pub mod cli;
pub mod curseforge_wrapper;
pub mod gh_releases;
pub mod metadata;
pub mod modrinth_wrapper;
use clap::ValueEnum;
use cli::Source;
use color_eyre::Result;
use color_eyre::eyre::{ContextCompat, bail};
use curseforge_wrapper::{CurseForgeAPI, CurseForgeMod};
use gh_releases::{Error, GHReleasesAPI};
use hmac_sha512::Hash;
use itertools::Itertools;
use metadata::Metadata;
use modrinth_wrapper::modrinth;
use serde::Deserialize;
use std::collections::HashSet;
use std::ffi::OsStr;
use std::fmt;
use std::hash::RandomState;
use std::sync::{Arc, LazyLock};
use std::{env, path::PathBuf};
use std::{fmt::Display, fs, io::Read};
use strum::{Display, EnumIter, IntoEnumIterator};
use tokio::task::JoinHandle;
use tracing::{self, error, info};

pub static MOD_LOADERS: LazyLock<Vec<ModLoader>> =
    LazyLock::new(|| ModLoader::iter().collect_vec());
#[derive(Debug, Deserialize, Clone)]
pub enum Mods {
    AntiXray,
    AppleSkin,
    CarpetExtra,
    EasyAuth,
    EssentialCommands,
    FabricApi,
    FabricCarpet,
    Geyser,
    Lithium,
    Origins,
    SkinRestorer,
    Status,
    WorldEdit,
}

impl Display for Mods {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text = match self {
            Mods::FabricApi => "fabric-api".to_string(),
            Mods::AntiXray => "anti-xray".to_string(),
            Mods::AppleSkin => "appleskin".to_string(),
            Mods::EasyAuth => "easy-auth".to_string(),
            Mods::EssentialCommands => "essential-commands".to_string(),
            Mods::Lithium => "lithium".to_string(),
            Mods::Origins => "origins".to_string(),
            Mods::SkinRestorer => "skin-restorer".to_string(),
            Mods::Status => "status".to_string(),
            Mods::CarpetExtra => "carpet-extra".to_string(),
            Mods::FabricCarpet => "fabric-carpet".to_string(),
            Mods::Geyser => "geyser".to_string(),
            Mods::WorldEdit => "worldedit".to_string(),
        };

        write!(f, "{}", text)
    }
}

pub fn calc_sha512(filename: &str) -> String {
    let mut file = fs::File::open(filename).unwrap();
    let mut text = Vec::new();
    file.read_to_end(&mut text).unwrap();
    let hash = Hash::hash(text);
    hex::encode(hash)
}

pub async fn update_dir(
    github: &mut GHReleasesAPI,
    curseforge: CurseForgeAPI,
    dir: &str,
    new_version: &str,
    del_prev: bool,
    prefix: &str,
    source: Option<Source>,
    no_other_sources: bool,
    loader: Option<ModLoader>,
) -> Result<()> {
    let mut handles = Vec::new();
    let github = Arc::new(github.clone());
    let curseforge = Arc::new(curseforge.clone());
    let source = source.clone().unwrap_or(Source::Modrinth);
    for entry in fs::read_dir(dir).unwrap() {
        let new_version = new_version.to_string();
        let loader = loader.clone();
        let prefix = prefix.to_string();
        let source = source.clone();
        let github = github.clone();
        let curseforge = curseforge.clone();
        let handle: JoinHandle<Result<()>> = tokio::spawn(async move {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_file() && path.extension().unwrap_or(OsStr::new("")) == "jar" {
                info!("Updating {:?}", path);
                let success: Result<()> = match source {
                    Source::Modrinth => modrinth::update_from_file(
                        path.to_str().unwrap(),
                        &new_version,
                        &prefix,
                        loader.clone(),
                    )
                    .await
                    .map_err(|err| err.into()),
                    Source::Github => {
                        update_file_github(
                            (*github).clone(),
                            path.to_str().unwrap(),
                            &new_version,
                            del_prev,
                            &prefix,
                        )
                        .await
                    }
                    Source::CurseForge => {
                        update_file_curseforge(
                            (*curseforge).clone(),
                            path.to_str().unwrap(),
                            &new_version,
                            &prefix,
                        )
                        .await
                    }
                };
                if success.is_err() && no_other_sources {
                    let mut set = HashSet::<Source, RandomState>::from_iter(Source::iter());
                    set.remove(&source);
                    for source in set {
                        let loader = loader.clone();
                        info!(
                            "Trying to update {} with {}",
                            path.to_str().unwrap(),
                            source
                        );
                        let success: Result<()> = match source {
                            Source::Modrinth => modrinth::update_from_file(
                                path.to_str().unwrap(),
                                &new_version,
                                &prefix,
                                loader,
                            )
                            .await
                            .map_err(|err| err.into()),
                            Source::Github => {
                                update_file_github(
                                    (*github).clone(),
                                    path.to_str().unwrap(),
                                    &new_version,
                                    del_prev,
                                    &prefix,
                                )
                                .await
                            }
                            Source::CurseForge => {
                                update_file_curseforge(
                                    (*curseforge).clone(),
                                    path.to_str().unwrap(),
                                    &new_version,
                                    &prefix,
                                )
                                .await
                            }
                        };
                        match success {
                            Ok(_) => {
                                info!(
                                    "Successfully updated {} with {}",
                                    path.to_str().unwrap(),
                                    source
                                );
                                if del_prev && path.ends_with(entry.file_name()) {
                                    fs::remove_file(path).unwrap();
                                }

                                break;
                            }
                            Err(err) => {
                                error!(
                                    "Failed to update {} with {}: {err}",
                                    path.to_str().unwrap(),
                                    source
                                );
                                continue;
                            }
                        }
                    }
                }
            }

            Ok(())
        });
        handles.push(handle);
    }
    for handle in handles {
        handle.await??;
    }
    Ok(())
}

pub fn get_minecraft_dir() -> PathBuf {
    let home_dir = env::var("HOME").ok().map(PathBuf::from);
    #[cfg(target_os = "windows")]
    {
        let appdata = env::var("APPDATA").expect("%APPDATA% not set");
        PathBuf::from(appdata).join(".minecraft")
    }

    #[cfg(target_os = "macos")]
    {
        home_dir
            .expect("HOME not set")
            .join("Library/Application Support/minecraft")
    }

    #[cfg(target_os = "linux")]
    {
        home_dir.expect("HOME not set").join(".minecraft")
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq, Default)]
pub struct Link {
    pub text: String,
    pub url: String,
}

impl Link {
    pub fn new(text: String, url: String) -> Self {
        Self { text, url }
    }
}

impl fmt::Display for Link {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "\u{1b}]8;;{}\u{1b}\\{}\u{1b}]8;;\u{1b}\\",
            self.url, self.text
        )
    }
}

pub struct UrlBuilder {
    pub base: String,
    pub path: String,
    pub params: Vec<(String, String)>,
}
impl UrlBuilder {
    pub fn new(base: &str, path: &str) -> Self {
        Self {
            base: base.to_string(),
            path: path.to_string(),
            params: Vec::new(),
        }
    }
    fn add_param(&mut self, key: &str, value: &str) {
        self.params.push((key.to_string(), value.to_string()));
    }
}

impl fmt::Display for UrlBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut url = self.base.clone();
        url.push_str(&self.path);
        if self.params.is_empty() {
            return write!(f, "{}", url);
        }
        let mut iter = self.params.iter();
        let first = iter.next().unwrap();
        url.push('?');
        url.push_str(&first.0);
        url.push('=');
        url.push_str(&first.1);
        for param in iter {
            url.push('&');
            url.push_str(&param.0);
            url.push('=');
            url.push_str(&param.1);
        }
        write!(f, "{}", url)
    }
}

#[derive(
    Debug, clap::ValueEnum, PartialEq, Default, Eq, Clone, Display, Hash, EnumIter, strum::AsRefStr,
)]
pub enum ModLoader {
    Forge,
    #[default]
    Fabric,
    Quilt,
    NeoForge,
    Cauldron,
    LiteLoader,
    Any,
}

pub async fn update_file_github(
    github: GHReleasesAPI,
    filename: &str,
    new_version: &str,
    del_prev: bool,
    prefix: &str,
) -> Result<()> {
    let metadata = Metadata::get_all_metadata(PathBuf::from(filename));
    let Ok(metadata) = metadata else {
        bail!("Could not find metadata for {}", filename);
    };
    let source: Result<Source> = match metadata.get("source") {
        Some(source) => Ok(Source::from_str(source, true).unwrap()),
        None => bail!("No key found"),
    };

    if let Ok(Source::Github) = source {
        info!("Checking Github for mod");
        let repo = metadata.get("repo");
        if repo.is_none() {
            bail!("Could not find repo for {}", filename);
        }
        let repo = repo.unwrap();
        let split = repo.split("/").collect_vec();
        let update = github.get_releases(split[0].trim(), split[1]).await;

        if update.is_err() {
            bail!(
                "Could not find update for {}: {:?}",
                filename,
                update.err().unwrap()
            );
        }
        let update = update.unwrap();
        let mod_ = gh_releases::get_mod_from_release(&update, "fabric", new_version).await?;
        mod_.download(format!("{}/{}", prefix, mod_.name).into(), split.join("/"))
            .await
            .unwrap();
        Ok(())
    } else {
        Err(Error::NoReleases)?
    }
}

pub async fn update_file_curseforge(
    curseforge: CurseForgeAPI,
    filename: &str,
    new_version: &str,
    prefix: &str,
) -> Result<()> {
    let mod_ = curseforge
        .get_mod_from_file(PathBuf::from(filename))
        .await?;
    let new_mod = curseforge.get_mods(mod_.id).await?;
    let Some(new_mod) = new_mod.first() else {
        bail!("Version not found for {filename}");
    };
    let Some(new_version) = new_mod.get_version_and_loader(new_version) else {
        bail!("Version {new_version} not found for {filename}");
    };
    curseforge
        .download_mod(new_mod.id, new_version.file_id, prefix.into())
        .await?;
    Ok(())
}
