#![allow(dead_code)]
pub mod cli;
pub mod gh_releases;
pub mod metadata;
mod modrinth_wrapper;
use hmac_sha512::Hash;
use modrinth_wrapper::modrinth;
use serde::Deserialize;
use std::ffi::OsStr;
use std::fmt;
use std::{env, path::PathBuf};
use std::{fmt::Display, fs, io::Read};
use tracing::{self, info};

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

pub async fn update_dir(dir: &str, new_version: &str, del_prev: bool, prefix: &str) {
    let mut handles = Vec::new();
    for entry in fs::read_dir(dir).unwrap() {
        let new_version = new_version.to_string();
        let prefix = prefix.to_string();
        let handle = tokio::spawn(async move {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_file() && path.extension().unwrap_or(OsStr::new("")) == "jar" {
                info!("Updating {:?}", path);
                modrinth::update_from_file(path.to_str().unwrap(), &new_version, del_prev, &prefix)
                    .await;
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

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
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
