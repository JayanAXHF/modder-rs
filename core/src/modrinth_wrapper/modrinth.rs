#![allow(dead_code)]
use crate::cli::Source;
use crate::gh_releases::{self, GHReleasesAPI};
use crate::metadata::{Error as MetadataError, Metadata};
use crate::{Link, ModLoader, calc_sha512};
use clap::ValueEnum;
use color_eyre::eyre::{self, ContextCompat, bail, eyre};
use colored::Colorize;
use futures::lock::Mutex;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::{fmt::Display, fs};
use tracing::{self, debug, error, info, warn};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Error sending the request. This may mean that the request was malformed: {0:?}")]
    RequestErr(#[from] reqwest::Error),
    #[error("Error deserializing the response: {0:?}")]
    SerdeErr(#[from] serde_json::Error),
    #[error("No versions found for mod {0}")]
    NoVersionsFound(String),
    // TODO: Move this to `lib.rs`
    #[error("Metadata error: {0}")]
    MetadataErr(#[from] MetadataError),
    #[error("Unknown error: {0}")]
    UnknownError(#[from] color_eyre::eyre::ErrReport),
    #[error("Error getting mod from github: {0}")]
    GithubError(#[from] gh_releases::Error),
    #[error("Error writing file: {0}")]
    IoError(#[from] std::io::Error),
}

type Result<T> = color_eyre::Result<T, Error>;

const GRAY: (u8, u8, u8) = (128, 128, 128);

#[derive(Debug, Deserialize, Clone)]
pub struct VersionData {
    name: Option<String>,
    version_number: Option<String>,
    game_versions: Option<Vec<String>>,
    changelog: Option<String>,
    pub dependencies: Option<Vec<Dependency>>,
    version_type: Option<String>,
    loaders: Option<Vec<String>>,
    featured: Option<bool>,
    status: Option<String>,
    id: String,
    pub project_id: String,
    author_id: String,
    date_published: String,
    downloads: u32,
    changelog_url: Option<String>,
    pub files: Option<Vec<File>>,
}

#[derive(Debug, Deserialize, Clone, Eq, PartialEq)]
pub struct Dependency {
    version_id: Option<String>,
    project_id: Option<String>,
    file_name: Option<String>,
    dependency_type: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct File {
    pub hashes: FileHash,
    url: String,
    pub filename: String,
    primary: bool,
    size: u32,
    file_type: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct FileHash {
    pub sha512: String,
    sha1: String,
}

#[derive(Debug, Deserialize)]
pub struct GetProject {
    id: String,
    slug: String,
    project_type: String,
    team: String,
    title: String,
    description: String,
    categories: Vec<String>,
    additional_categories: Vec<String>,
    client_side: String,
    server_side: String,
    body: String,
    status: String,
    requested_status: Option<String>,
    issues_url: Option<String>,
    source_url: Option<String>,
    wiki_url: Option<String>,
    discord_url: Option<String>,
    donation_urls: Vec<DonationLink>,
    icon_url: Option<String>,
    color: Option<u32>,
    thread_id: String,
    monetization_status: Option<String>,
    body_url: Option<String>,
    moderator_message: Option<ModeratorMessage>,
    published: String,
    updated: String,
    approved: Option<String>,
    queued: Option<String>,
    downloads: u32,
    followers: u32,
    license: License,
    versions: Vec<String>,
    game_versions: Vec<String>,
    loaders: Vec<String>,
    gallery: Vec<GalleryImage>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ModeratorMessage {
    message: String,
    body: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct License {
    id: String,
    name: String,
    url: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DonationLink {
    id: String,
    platform: String,
    url: String,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct GalleryImage {
    url: String,
    featured: bool,
    title: Option<String>,
    description: Option<String>,
    created: String,
    ordering: Option<i32>,
}

impl GetProject {
    pub async fn from_id(id: &str) -> Option<Self> {
        let res = reqwest::get(format!("https://api.modrinth.com/v2/project/{}", id)).await;
        if res.is_err() {
            error!("Error getting project: {}", res.err().unwrap());
            return None;
        }
        let res = res.unwrap();
        let text = res.text().await.unwrap();
        debug!(text);
        let res: Result<GetProject> = serde_json::from_str(&text).map_err(Error::SerdeErr);
        if res.is_err() {
            error!("Error parsing project: {}", res.err().unwrap());
            return None;
        }
        Some(res.unwrap())
    }
    pub fn get_title(&self) -> String {
        self.title.clone()
    }
    pub fn get_categories(&self) -> Vec<String> {
        self.categories.clone()
    }
    pub fn get_slug(&self) -> String {
        self.slug.clone()
    }
}

pub struct Modrinth;

impl Modrinth {
    async fn get_version_data(
        mod_name: &str,
        version: &str,
        mod_loader: &str,
    ) -> Result<Vec<VersionData>> {
        debug!(mod_name = ?mod_name, version = ?version, mod_loader = ?mod_loader);
        let versions = reqwest::get(format!(
        "https://api.modrinth.com/v2/project/{}/version?game_versions=[\"{}\"]&loaders=[\"{}\"]",
        mod_name, version, mod_loader.to_lowercase()
    ))
    .await
    .expect("Failed to get versions");

        let versions = versions.text().await.unwrap();
        debug!(versions = ?versions);
        serde_json::from_str(&versions).map_err(Error::SerdeErr)
    }
    pub async fn search_mods(query: &str, limit: u16, offset: u16) -> ProjectSearch {
        let client = reqwest::Client::new();
        let res = client .get(format!("https://api.modrinth.com/v2/search?query={}&limit={}&index=relevance&facets=%5B%5B%22project_type%3Amod%22%5D%5D&offset={}",query,limit, offset )) .send().await.unwrap();

        let res_text = res.text().await.unwrap();

        let parsed: ProjectSearch = serde_json::from_str(&res_text).unwrap();
        parsed
    }

    pub async fn get_version(
        mod_name: &str,
        version: &str,
        loader: ModLoader,
    ) -> Option<VersionData> {
        let versions = Modrinth::get_version_data(mod_name, version, &loader.to_string()).await;
        if versions.is_err() {
            error!(
                "Error parsing versions for mod {}: {}. This may mean that this mod is not available for this version",
                mod_name,
                versions.err().unwrap()
            );
            return None;
        }
        let versions = versions.unwrap();

        if versions.is_empty() {
            error!("No versions found for mod {} for {}", mod_name, version);
            return None;
        }
        Some(versions[0].clone())
    }

    pub async fn get_top_mods(limit: u16) -> Vec<Project> {
        let mut mods = Vec::new();
        let mut handles = Vec::new();
        let temp_mods = Arc::new(Mutex::new(Vec::new()));
        for i in 0..(limit / 100) {
            let temp_mods = Arc::clone(&temp_mods);
            let handle = tokio::spawn(async move {
                let parsed = Modrinth::search_mods("", 100, i * 100).await;
                let hits = parsed.hits;

                let mut temp_mods_guard = temp_mods.lock().await;
                temp_mods_guard.extend(hits);
            });
            handles.push(handle);
        }
        info!(temp_mods = ?temp_mods.lock().await.len(), "Got mods");

        if limit % 100 != 0 {
            let temp_mods = Arc::clone(&temp_mods.clone());
            handles.push(tokio::spawn(async move {
                let res = Modrinth::search_mods("", limit % 100, (limit / 100) * 100).await;
                let hits = res.hits;
                let mut temp_mods = temp_mods.lock().await;
                temp_mods.extend(hits);
            }));
        }
        for handle in handles {
            handle.await.unwrap();
        }
        mods.extend(
            Arc::clone(&temp_mods)
                .lock()
                .await
                .iter()
                .cloned()
                .collect::<Vec<Project>>(),
        );
        mods
    }
    pub async fn download_dependencies(
        mod_: &Mod,
        version: &str,
        prev_deps: Arc<Mutex<Vec<Dependency>>>,
        prefix: &str,
        loader: ModLoader,
    ) {
        let mod_ = Modrinth::get_version(&mod_.slug, version, loader.clone()).await;
        let mut prev_deps = prev_deps.lock().await;
        let mut handles = Vec::new();

        if let Some(mod_) = mod_ {
            for dependency in mod_.dependencies.unwrap() {
                let loader = loader.clone();
                if prev_deps.contains(&dependency) {
                    info!(
                        "Skipping dependency {}",
                        dependency.file_name.unwrap_or("Unknown".to_string())
                    );
                    continue;
                }
                prev_deps.push(dependency.clone());
                let dependency =
                    Modrinth::get_version(&dependency.project_id.unwrap(), version, loader).await;

                if let Some(dependency) = dependency {
                    info!(
                        "Downloading dependency {}",
                        dependency.clone().files.unwrap()[0].filename
                    );
                    let prefix = prefix.to_string();
                    let handle = tokio::spawn(async move {
                        download_file(&dependency.files.unwrap()[0], &prefix).await;
                    });
                    handles.push(handle);
                }
            }
        }
        for handle in handles {
            handle.await.unwrap();
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct Project {
    pub slug: String,
    pub title: String,
    pub description: String,
    pub categories: Vec<String>,
    pub client_side: SupportLevel,
    pub server_side: SupportLevel,
    pub project_type: ProjectType,
    pub downloads: u64,
    pub icon_url: Option<String>,
    pub color: Option<u32>,
    pub thread_id: Option<String>,
    pub monetization_status: Option<MonetizationStatus>,
    pub project_id: String,
    pub author: String,
    pub display_categories: Vec<String>,
    pub versions: Vec<String>,
    pub follows: u64,
    pub date_created: String,
    pub date_modified: String,
    pub latest_version: Option<String>,
    pub license: String,
    pub gallery: Vec<String>,
    pub featured_gallery: Option<String>,
}

impl Display for Project {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.title)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "lowercase")]
pub enum SupportLevel {
    Required,
    Optional,
    Unsupported,
    Unknown,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "lowercase")]
pub enum ProjectType {
    Mod,
    Modpack,
    Resourcepack,
    Shader,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub enum MonetizationStatus {
    Monetized,
    Demonetized,
    ForceDemonetized,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProjectSearch {
    pub hits: Vec<Project>,
    offset: u16,
    limit: u16,
    total_hits: u16,
}

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct Mod {
    pub slug: String,
    pub title: String,
}

impl From<Project> for Mod {
    fn from(project: Project) -> Self {
        Mod {
            slug: project.slug,
            title: project.title,
        }
    }
}

impl Display for Mod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.title)
    }
}

impl VersionData {
    pub async fn from_hash(hash: String) -> Result<Self> {
        // TODO: Add this to the API
        let res = reqwest::get(format!("https://api.modrinth.com/v2/version_file/{hash}"))
            .await
            .unwrap();
        let res = res.text().await.unwrap();
        let res: Result<VersionData> = serde_json::from_str(&res).map_err(Error::SerdeErr);
        res
    }
    pub fn format_verbose(&self, mod_name: &str, categories: &[String]) -> String {
        let mut output = String::new();
        let url = format!("https://modrinth.com/mod/{}", self.project_id);
        let link = Link::new(url.clone(), url);
        output.push_str(&format!(
            "{} {}\n",
            mod_name.bold(),
            self.version_number
                .clone()
                .unwrap()
                .truecolor(GRAY.0, GRAY.1, GRAY.2)
        ));
        output.push_str(&format!("\tURL: {}\n", link.to_string().blue(),));
        output.push_str(&format!("\tMod version: {}\n", self.name.clone().unwrap()));
        output.push_str(&format!(
            "\tGame versions: {}\n",
            self.game_versions.clone().unwrap().join(", ").green()
        ));
        output.push_str(&format!(
            "\tLoaders: {}\n",
            self.loaders.clone().unwrap().join(", ").cyan()
        ));
        output.push_str(&format!(
            "\tCategories: {}\n",
            categories.join(", ").yellow()
        ));
        output.push_str(&format!("\tStatus: {}\n", self.status.clone().unwrap()));
        output.push_str(&format!(
            "\tDate published: {}\n",
            self.date_published.clone()
        ));
        output.push_str(&format!("\tDownloads: {}\n\n", self.downloads.clone()));

        output
    }
    pub fn format(&self, mod_name: &str) -> String {
        let mut output = String::new();
        let url = format!("https://modrinth.com/mod/{}", self.project_id);
        let link = Link::new(mod_name.to_string(), url);
        let version_type = match self
            .version_type
            .clone()
            .unwrap_or_default()
            .to_uppercase()
            .as_str()
        {
            "RELEASE" => "RELEASE".green(),
            "BETA" => "BETA".yellow(),
            "ALPHA" => "ALPHA".red(),
            _ => "UNKNOWN".cyan(),
        };
        output.push_str(&format!(
            "{}\t{}\t{}\n",
            version_type,
            self.project_id.truecolor(GRAY.0, GRAY.1, GRAY.2),
            link.to_string().bold()
        ));

        output
    }
    pub fn get_game_versions(&self) -> Option<Vec<String>> {
        self.game_versions.clone()
    }
    pub fn get_version(&self) -> String {
        self.version_number.clone().unwrap_or_default()
    }
    pub fn get_version_type(&self) -> String {
        self.version_type.clone().unwrap_or_default()
    }
}

pub async fn update_from_file(
    filename: &str,
    new_version: &str,
    prefix: &str,
    loader: Option<ModLoader>,
) -> Result<()> {
    let hash = calc_sha512(filename);
    let version_data = VersionData::from_hash(hash).await?;
    let loader = if let Some(loader) = loader {
        loader
    } else {
        match version_data
            .loaders
            .unwrap_or(vec!["fabric".to_string()])
            .first()
            .context("No loader found")?
            .as_str()
        {
            "fabric" => ModLoader::Fabric,
            "forge" => ModLoader::Forge,
            "quilt" => ModLoader::Quilt,
            "neoforge" => ModLoader::NeoForge,
            "cauldron" => ModLoader::Cauldron,
            "liteloader" => ModLoader::LiteLoader,
            _ => ModLoader::Any,
        }
    };

    let new_version_data =
        Modrinth::get_version(&version_data.project_id, new_version, loader).await;

    let Some(new_version_data) = new_version_data else {
        return Err(Error::NoVersionsFound(filename.to_string()));
    };

    download_file(&new_version_data.clone().files.unwrap()[0], prefix).await;

    Ok(())
}

pub async fn download_file(file: &File, prefix: &str) {
    let file_content = reqwest::get(file.url.clone()).await.unwrap();
    fs::write(
        format!("{}/{}", prefix, file.filename.clone()),
        file_content.bytes().await.unwrap(),
    )
    .unwrap();
}
