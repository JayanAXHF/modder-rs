use crate::ModLoader;
use serde::Deserialize;
use std::fmt::Display;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CurseForgeError {
    #[error("Invalid response from CurseForge")]
    InvalidResponse,
    #[error("JSON Parsing error: {0}")]
    JsonParsingError(#[from] serde_json::Error),
    #[error("HTTP Error: {0}")]
    HttpError(#[from] reqwest::Error),
    #[error("IO Error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("No game version found for mod {0}")]
    NoGameVersionFound(String),
    #[error("No fingerprint found for mod {0}")]
    NoFingerprintFound(String),
    #[error("No mod found")]
    NoModFound,
    #[error("URL Parse error: {0}")]
    UrlParseError(#[from] url::ParseError),
    #[error("Unknown error: {0}")]
    UnknownError(#[from] color_eyre::eyre::Report),
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Root {
    pub data: Vec<Mod>,
    pub pagination: Option<Pagination>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Mod {
    pub id: u32,
    pub game_id: u32,
    pub name: String,
    pub slug: String,
    pub links: Links,
    pub summary: String,
    pub status: u32,
    pub download_count: u32,
    pub is_featured: bool,
    pub primary_category_id: u32,
    pub categories: Vec<Category>,
    pub class_id: u32,
    pub authors: Vec<Author>,
    pub logo: Logo,
    pub screenshots: Vec<Screenshot>,
    pub main_file_id: u32,
    pub latest_files: Vec<File>,
    pub latest_files_indexes: Vec<FileIndex>,
    pub latest_early_access_files_indexes: Vec<FileIndex>,
    pub date_created: String,
    pub date_modified: String,
    pub date_released: String,
    pub allow_mod_distribution: bool,
    pub game_popularity_rank: u32,
    pub is_available: bool,
    pub thumbs_up_count: u32,
    pub rating: Option<u32>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Links {
    pub website_url: Option<String>,
    pub wiki_url: Option<String>,
    pub issues_url: Option<String>,
    pub source_url: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Category {
    pub id: u32,
    pub game_id: u32,
    pub name: String,
    pub slug: String,
    pub url: String,
    pub icon_url: String,
    pub date_modified: String,
    pub is_class: bool,
    pub class_id: u32,
    pub parent_category_id: u32,
    pub display_index: Option<u32>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Author {
    pub id: u32,
    pub name: String,
    pub url: String,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Logo {
    pub id: u32,
    pub mod_id: u32,
    pub title: String,
    pub description: String,
    pub thumbnail_url: String,
    pub url: String,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Screenshot {
    pub id: u32,
    pub mod_id: u32,
    pub title: String,
    pub description: String,
    pub thumbnail_url: String,
    pub url: String,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct File {
    pub id: u32,
    pub game_id: u32,
    pub mod_id: u32,
    pub is_available: bool,
    pub display_name: String,
    pub file_name: String,
    pub release_type: u32,
    pub file_status: u32,
    pub hashes: Vec<FileHash>,
    pub file_date: String,
    pub file_length: u64,
    pub download_count: u32,
    pub file_size_on_disk: Option<u64>,
    pub download_url: Option<String>,
    pub game_versions: Vec<String>,
    pub sortable_game_versions: Vec<SortableGameVersion>,
    pub dependencies: Vec<Dependency>,
    pub expose_as_alternative: Option<bool>,
    pub parent_project_file_id: Option<u32>,
    pub alternate_file_id: Option<u32>,
    pub is_server_pack: bool,
    pub server_pack_file_id: Option<u32>,
    pub is_early_access_content: Option<bool>,
    pub early_access_end_date: Option<String>,
    pub file_fingerprint: u64,
    pub modules: Vec<Module>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FileHash {
    pub value: String,
    pub algo: u32,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SortableGameVersion {
    pub game_version_name: String,
    pub game_version_padded: String,
    pub game_version: String,
    pub game_version_release_date: String,
    pub game_version_type_id: Option<u32>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Dependency {
    pub mod_id: u32,
    pub relation_type: u32,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Module {
    pub name: String,
    pub fingerprint: u64,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FileIndex {
    pub game_version: String,
    pub file_id: u32,
    pub filename: String,
    pub release_type: u32,
    pub game_version_type_id: Option<u32>,
    pub mod_loader: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pagination {
    pub index: u32,
    pub page_size: u32,
    pub result_count: u32,
    pub total_count: u32,
}

pub struct ModBuilder {
    pub game_version: String,
    pub search: String,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FingerprintResponse {
    #[serde(default)]
    pub is_cache_built: bool,
    pub exact_matches: Vec<ExactMatch>,
    pub exact_fingerprints: Vec<u32>,
    pub partial_matches: Vec<PartialMatch>,
    pub partial_match_fingerprints: std::collections::HashMap<String, Vec<u32>>,
    pub installed_fingerprints: Vec<u32>,
    pub unmatched_fingerprints: Vec<u32>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ExactMatch {
    pub id: u32,
    pub file: File,
    pub latest_files: Vec<File>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PartialMatch {
    pub id: u32,
    pub file: File,
    pub latest_files: Vec<File>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct DownloadFile {
    pub data: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct GetModFileResponse {
    pub data: File,
}

pub trait CurseForgeMod {
    fn get_version_and_loader(&self, game_version: &str) -> Option<FileIndex>;
}

impl CurseForgeMod for Mod {
    fn get_version_and_loader(&self, game_version: &str) -> Option<FileIndex> {
        println!("{:#?}", game_version);
        self.latest_files_indexes
            .iter()
            .find(|file_index| file_index.game_version == game_version)
            .cloned()
    }
}

pub trait AsNum {
    fn as_num(&self) -> u8;
}
impl AsNum for ModLoader {
    fn as_num(&self) -> u8 {
        match self {
            ModLoader::Forge => 1,
            ModLoader::Cauldron => 2,
            ModLoader::LiteLoader => 3,
            ModLoader::Fabric => 4,
            ModLoader::Quilt => 5,
            ModLoader::NeoForge => 6,
            ModLoader::Any => 0,
        }
    }
}

impl Display for Mod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}
