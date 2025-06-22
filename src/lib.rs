#![allow(dead_code)]
use futures::lock::Mutex;
use hmac_sha512::Hash;
use serde::{Deserialize, Serialize};
use serde_json::Result;
use std::sync::Arc;
use std::{env, path::PathBuf};
use std::{fmt::Display, fs, io::Read};
use tracing::{self, error, info};

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

pub async fn get_version(mod_name: &str, version: &str) -> Option<VersionData> {
    let versions = reqwest::get(format!(
        "https://api.modrinth.com/v2/project/{}/version?game_versions=[\"{}\"]&loaders=[\"fabric\"]",
        mod_name, version
    ))
    .await
    .expect("Failed to get versions");

    let versions = versions.text().await.unwrap();
    let versions: Result<Vec<VersionData>> = serde_json::from_str(&versions);
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
        error!("No versions found for mod {}", mod_name);
        return None;
    }
    Some(versions[0].clone())
}

pub async fn download_file(file: &File, prefix: &str) {
    let file_content = reqwest::get(file.url.clone()).await.unwrap();
    fs::write(
        format!("{}/{}", prefix, file.filename.clone()),
        file_content.bytes().await.unwrap(),
    )
    .unwrap();
}

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

pub async fn get_top_mods(limit: u16) -> Vec<Project> {
    let mut mods = Vec::new();
    let client = reqwest::Client::new();
    let mut handles = Vec::new();
    let temp_mods = Arc::new(Mutex::new(Vec::new()));
    for i in 0..(limit / 100) {
        let temp_mods = Arc::clone(&temp_mods);
        let handle = tokio::spawn(async move {
            let client = reqwest::Client::new();
            let res = client
                        .get(format!("https://api.modrinth.com/v2/search?limit=100&index=relevance&facets=%5B%5B%22project_type%3Amod%22%5D%5D&offset={}", i * 100))
                            .send().await.unwrap();
            let res_text = res.text().await.unwrap();

            let parsed: ProjectSearch = serde_json::from_str(&res_text).unwrap();
            let hits = parsed.hits;

            let mut temp_mods_guard = temp_mods.lock().await;
            temp_mods_guard.extend(hits);
        });
        handles.push(handle);
    }
    info!(temp_mods = ?temp_mods.lock().await.len(), "Got mods");

    if limit % 100 != 0 {
        let temp_mods = Arc::clone(&temp_mods.clone());
        handles.push        (

        tokio::spawn(async move {
            let res = client.get(
                format!("https://api.modrinth.com/v2/search?limit={}&index=relevance&facets=%5B%5B%22project_type%3Amod%22%5D%5D&offset={}", limit % 100, (limit / 100) * 100)
            ).send().await.unwrap();
            let res = res.text().await.unwrap();
            let res: Result<ProjectSearch> = serde_json::from_str(&res);
            let res = res.unwrap();
            let hits = res.hits;
            let mut temp_mods = temp_mods.lock().await;
            temp_mods.extend(hits);
        })
    );
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

pub async fn download_dependencies(
    mod_: &Mod,
    version: &str,
    prev_deps: Arc<Mutex<Vec<Dependency>>>,
    prefix: &str,
) {
    let mod_ = get_version(&mod_.slug, version).await;
    let mut prev_deps = prev_deps.lock().await;
    let mut handles = Vec::new();
    if let Some(mod_) = mod_ {
        for dependency in mod_.dependencies.unwrap() {
            if prev_deps.contains(&dependency) {
                info!(
                    "Skipping dependency {}",
                    dependency.file_name.unwrap_or("Unknown".to_string())
                );
                continue;
            }
            prev_deps.push(dependency.clone());
            let dependency = get_version(&dependency.project_id.unwrap(), version).await;

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

pub fn calc_sha512(filename: &str) -> String {
    let mut file = fs::File::open(filename).unwrap();
    let mut text = Vec::new();
    file.read_to_end(&mut text).unwrap();
    let hash = Hash::hash(text);
    hex::encode(hash)
}

impl VersionData {
    pub async fn from_hash(hash: String) -> Self {
        let res = reqwest::get(format!("https://api.modrinth.com/v2/version_file/{hash}"))
            .await
            .unwrap();
        let res = res.text().await.unwrap();
        let res: Result<VersionData> = serde_json::from_str(&res);
        if res.is_err() {
            panic!("Error parsing version data: {}", res.err().unwrap());
        }
        res.unwrap()
    }
}

pub async fn update_from_file(filename: &str, new_version: &str, del_prev: bool, prefix: &str) {
    let hash = calc_sha512(filename);
    let version_data = VersionData::from_hash(hash).await;
    let new_version_data = get_version(&version_data.project_id, new_version).await;

    if new_version_data.is_none() {
        error!("Could not find version {} for {}", new_version, filename);
        return;
    }
    let new_version_data = new_version_data.unwrap();
    download_file(&new_version_data.clone().files.unwrap()[0], prefix).await;
    if del_prev
        && filename.split('/').last().unwrap() != new_version_data.files.unwrap()[0].filename
    {
        fs::remove_file(filename).unwrap();
    }
}

pub async fn update_dir(dir: &str, new_version: &str, del_prev: bool, prefix: &str) {
    let mut handles = Vec::new();
    for entry in fs::read_dir(dir).unwrap() {
        let new_version = new_version.to_string();
        let prefix = prefix.to_string();
        let handle = tokio::spawn(async move {
            let entry = entry.unwrap();
            let path = entry.path();
            info!("Updating {:?}", path);
            if path.is_file() {
                update_from_file(path.to_str().unwrap(), &new_version, del_prev, &prefix).await;
            }
        });
        handles.push(handle);
    }
    for handle in handles {
        handle.await.unwrap();
    }
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
