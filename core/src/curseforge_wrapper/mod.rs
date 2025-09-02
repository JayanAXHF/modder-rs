mod file_utils;
mod hash;
mod structs;
use crate::ModLoader;
use color_eyre::eyre::Context;
pub use file_utils::get_jar_contents;
pub use hash::*;
use percent_encoding::percent_decode;
use reqwest::{
    Method,
    header::{HeaderMap, HeaderName, HeaderValue},
};
use serde_json::json;
use std::{fs, path::PathBuf, sync::LazyLock};
pub use structs::*;
use tracing::debug;
use url::Url;

type Result<T> = color_eyre::Result<T, CurseForgeError>;
pub const GAME_ID: u32 = 432;
pub const BASE_URL: &str = "https://api.curseforge.com/v1";
pub const API_KEY: &str = env!("CURSEFORGE_API_KEY");
pub static HEADERS: LazyLock<HeaderMap> = LazyLock::new(|| {
    let mut headers = HeaderMap::new();
    headers.insert(
        HeaderName::from_static("x-api-key"),
        HeaderValue::from_str(API_KEY)
            .context("Invalid API key")
            .unwrap(),
    );
    headers.insert(
        HeaderName::from_static("accept"),
        HeaderValue::from_static("application/json"),
    );
    headers
});

pub trait AsModIdVec {
    fn as_mod_id_vec(&self) -> Vec<u32>;
}

impl AsModIdVec for &[u32] {
    fn as_mod_id_vec(&self) -> Vec<u32> {
        self.to_vec()
    }
}
impl AsModIdVec for u32 {
    fn as_mod_id_vec(&self) -> Vec<u32> {
        vec![*self]
    }
}

#[derive(Clone)]
pub struct CurseForgeAPI {
    pub client: reqwest::Client,
    pub api_key: String,
}

impl CurseForgeAPI {
    pub fn new(api_key: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key,
        }
    }
    pub async fn search_mods(
        &self,
        game_version: &str,
        loader: ModLoader,
        search: &str,
        page_size: u32,
    ) -> Result<Vec<Mod>> {
        let params = [
            ("gameId", GAME_ID.to_string()),
            ("index", 0.to_string()),
            ("searchFilter", search.to_string()),
            ("gameVersion", game_version.to_string()),
            ("pageSize", page_size.to_string()),
            ("sortField", 6.to_string()),
            ("gameFlavors[0]", loader.as_num().to_string()),
            ("sortOrder", "desc".to_string()),
        ];
        let params_str = params
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<String>>()
            .join("&");
        let url = format!("{}/mods/search?{params_str}", BASE_URL);
        debug!(url = ?url);
        let headers = HEADERS.clone();
        let response = self
            .client
            .request(Method::GET, Url::parse(&url)?)
            .headers(headers)
            .send()
            .await?;
        let response = response.error_for_status()?;
        let body = response.text().await?;
        debug!(body = ?body);
        let root: Root = serde_json::from_str(&body)?;
        debug!(root_data = ?root.data);
        Ok(root.data)
    }
    pub async fn get_mods<T>(&self, mod_ids: T) -> Result<Vec<Mod>>
    where
        T: AsModIdVec,
    {
        let mod_ids = mod_ids.as_mod_id_vec();
        let body = json!({
            "modIds": mod_ids,
            "filterPcOnly": true,
        });
        let url = format!("{}/mods", BASE_URL);
        let mut headers = HEADERS.clone();
        headers.insert(
            HeaderName::from_static("content-type"),
            HeaderValue::from_static("application/json"),
        );
        let response = self
            .client
            .request(Method::POST, Url::parse(&url)?)
            .headers(headers)
            .body(serde_json::to_string(&body)?)
            .send()
            .await?;
        let response = response.error_for_status()?;
        let body = response.text().await?;
        let root: Root = serde_json::from_str(&body)?;
        Ok(root.data)
    }
    pub async fn get_mod_files(
        &self,
        mod_id: u32,
        game_version: &str,
        mod_loader: ModLoader,
    ) -> Result<Vec<File>> {
        let params = [
            ("index", 0.to_string()),
            ("gameVersion", game_version.to_string()),
            ("pageSize", 1.to_string()),
            ("modLoaderType", mod_loader.as_num().to_string()),
        ];
        let params_str = params
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<String>>()
            .join("&");
        let url = format!("{BASE_URL}/mods/{mod_id}/files?{params_str}");
        let response = self
            .client
            .request(Method::GET, Url::parse(&url)?)
            .headers(HEADERS.clone())
            .send()
            .await?;
        let response = response.error_for_status()?;
        let body = response.text().await?;
        let root = serde_json::from_str::<FileSearchRoot>(&body)?;
        Ok(root.data)
    }
    pub async fn download_mod(&self, mod_id: u32, file_id: u32, dir: PathBuf) -> Result<()> {
        let url = format!(
            "{}/mods/{}/files/{}/download-url",
            BASE_URL, mod_id, file_id
        );
        let response = self
            .client
            .request(Method::GET, Url::parse(&url)?)
            .headers(HEADERS.clone())
            .send()
            .await?;
        let response = response.error_for_status()?;
        let body = response.text().await?;
        let json = serde_json::from_str::<DownloadFile>(&body)?;
        let url = json.data;
        let file_data = reqwest::get(url).await?;
        let file_name = file_data.url().path_segments().unwrap().last().unwrap();
        let file_name = percent_decode(file_name.as_bytes()).decode_utf8_lossy();
        let path = dir.join(file_name.to_string());
        fs::create_dir_all(path.parent().unwrap())?;
        fs::write(&path, file_data.bytes().await?)?;
        Ok(())
    }
    pub async fn get_version_from_file(&self, file: PathBuf) -> Result<File> {
        let f = file.clone();
        let f_name = f.file_name().unwrap().to_str().unwrap();
        let contents = get_jar_contents(&file)?;
        let fingerprint = MurmurHash2::hash(&contents);
        let url = format!("{BASE_URL}/fingerprints/{GAME_ID}");
        let mut headers = HEADERS.clone();
        headers.insert(
            HeaderName::from_static("content-type"),
            HeaderValue::from_static("application/json"),
        );
        let body = json!({
        "fingerprints": [
            fingerprint
        ]});
        let body = serde_json::to_string(&body)?;
        let response = self
            .client
            .request(Method::POST, Url::parse(&url)?)
            .headers(headers)
            .body(body)
            .send()
            .await?;
        let response = response.error_for_status()?;
        let body = response.text().await?;

        let res: FingerprintResponseRoot = serde_json::from_str(&body)?;
        let res = res.data;
        if res.exact_matches.is_empty() {
            return Err(CurseForgeError::NoFingerprintFound(f_name.to_string()));
        }
        let exact_match = res.exact_matches.first().unwrap();
        let file = exact_match.file.clone();
        Ok(file)
    }
    pub async fn get_mod_from_file(&self, file: PathBuf) -> Result<Mod> {
        let f = file.clone();
        let f_name = f.file_name().unwrap().to_str().unwrap();
        let contents = get_jar_contents(&file)?;
        let fingerprint = MurmurHash2::hash(&contents);
        let url = format!("{BASE_URL}/fingerprints/{GAME_ID}");
        let mut headers = HEADERS.clone();
        headers.insert(
            HeaderName::from_static("content-type"),
            HeaderValue::from_static("application/json"),
        );
        let body = json!({
        "fingerprints": [
            fingerprint
        ]});
        let body = serde_json::to_string(&body)?;
        let response = self
            .client
            .request(Method::POST, Url::parse(&url)?)
            .headers(headers)
            .body(body)
            .send()
            .await?;
        let response = response.error_for_status()?;
        let body = response.text().await?;
        let res: FingerprintResponseRoot = serde_json::from_str(&body)?;
        let res = res.data;
        if res.exact_matches.is_empty() {
            return Err(CurseForgeError::NoFingerprintFound(f_name.to_string()));
        }
        let exact_match = res.exact_matches.first().unwrap();
        let file = exact_match.file.clone();
        let mod_id = file.mod_id;
        let mod_ = self.get_mods(mod_id).await?;
        mod_.first().cloned().ok_or(CurseForgeError::NoModFound)
    }
    pub async fn get_dependencies(&self, mod_id: u32, version: &str) -> Result<Vec<Mod>> {
        let mod_ = self.get_mods(mod_id).await?;
        let mod_ = mod_.first().cloned().ok_or(CurseForgeError::NoModFound)?;
        let file_index = mod_
            .latest_files_indexes
            .iter()
            .find(|file| file.game_version == version)
            .cloned()
            .ok_or(CurseForgeError::NoGameVersionFound(version.to_string()))?;
        let url = format!("{}/mods/{}/files/{}", BASE_URL, mod_id, file_index.file_id);
        let file = self.client.get(url).headers(HEADERS.clone()).send().await?;
        let file = file.error_for_status()?;
        let body = file.text().await?;
        let file: GetModFileResponse = serde_json::from_str(&body)?;
        let file = file.data;

        let dep_ids = file.dependencies;
        let mut deps = Vec::with_capacity(dep_ids.len());
        for dep in dep_ids {
            let mod_ = self.get_mods(dep.mod_id).await?;
            let mod_ = mod_.first().cloned().ok_or(CurseForgeError::NoModFound)?;
            deps.push(mod_);
        }
        Result::Ok(deps)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    #[tokio::test]
    async fn test_search_mods() {
        let api = CurseForgeAPI::new(API_KEY.to_string());
        let loader = ModLoader::Fabric;
        let mods = api
            .search_mods("1.21.4", loader, "Carpet", 10)
            .await
            .unwrap();
        assert_eq!(!mods.is_empty(), true);
    }
    #[tokio::test]
    async fn test_get_mods() {
        std::panic::set_hook(Box::new(move |panic_info| {
            better_panic::Settings::auto()
                .most_recent_first(false)
                .lineno_suffix(true)
                .verbosity(better_panic::Verbosity::Full)
                .create_panic_handler()(panic_info);
        }));

        let api = CurseForgeAPI::new(API_KEY.to_string());
        let mods = api
            .get_mods(&[349239u32, 349240u32] as &[u32])
            .await
            .unwrap();
        assert_eq!(!mods.is_empty(), true);
    }
    #[tokio::test]
    async fn full_test() -> Result<()> {
        color_eyre::install()?;
        let search = "carpet";
        let loader = ModLoader::Fabric;
        let v = "1.21.4";
        let api = CurseForgeAPI::new(API_KEY.to_string());
        let mods = api.search_mods(v, loader, search, 10).await.unwrap();
        println!("{:#?}", mods);
        let prompt = inquire::MultiSelect::new("Select mods", mods);
        let selected = prompt.prompt().unwrap();
        for mod_ in selected {
            let version = mod_.get_version_and_loader(v).unwrap();
            api.download_mod(mod_.id, version.file_id, PathBuf::from("mods"))
                .await?;
        }
        Ok(())
    }
    #[tokio::test]
    async fn test_get_dependencies() {
        std::panic::set_hook(Box::new(move |panic_info| {
            better_panic::Settings::auto()
                .most_recent_first(false)
                .lineno_suffix(true)
                .verbosity(better_panic::Verbosity::Full)
                .create_panic_handler()(panic_info);
        }));
        let api = CurseForgeAPI::new(API_KEY.to_string());
        // 447673 --> Sodium Extra
        let deps = api.get_dependencies(447673, "1.21.4").await.unwrap();
        assert_eq!(!deps.is_empty(), true);
    }
    #[tokio::test]
    async fn test_fingerprint_specific_jar() {
        color_eyre::install().unwrap();
        let api = CurseForgeAPI::new(API_KEY.to_string());
        let jar_path = PathBuf::from(
            "/Users/jayansunil/Dev/rust/modder/tui/test/createaddition-1.19.2-1.2.3.jar",
        );
        let fingerprint = MurmurHash2::hash(&get_jar_contents(&jar_path).unwrap());
        dbg!(&fingerprint);
        // The fingerprint will be debug-printed by dbg!(&fingerprint) inside get_mod_from_file
        let mod_ = api.get_mod_from_file(jar_path).await.unwrap();
        assert_eq!(mod_.name, "CreateAddition");
    }
}
