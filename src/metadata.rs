use itertools::Itertools;
use std::{
    collections::HashMap,
    env::temp_dir,
    fs::{self, File},
    io::{Cursor, Read, Write},
    path::PathBuf,
};
use tracing::info;
use zip::{ZipWriter, write::FileOptions};

use crate::cli::Source;

pub struct Metadata;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Error reading or writing the metadata file: {0}")]
    IOErr(#[from] std::io::Error),
    #[error("Error unarchiving the jar file: {0}")]
    Unzip(#[from] zip::result::ZipError),
    #[error("Error parsing the metadata file into UTF-8 String: {0}")]
    ParseErr(#[from] std::string::FromUtf8Error),
    #[error("No key found")]
    NoKeyFound,
    #[error("Error deserializing the metadata file: {0}")]
    SerdeErr(#[from] serde_json::Error),
}

type Result<T> = std::result::Result<T, Error>;

impl Metadata {
    pub fn add_metadata(path: PathBuf, source: Source, key: &str, value: &str) -> Result<()> {
        let mut file = File::open(path.clone())?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;
        let mut zip = zip::ZipArchive::new(Cursor::new(buffer))?;
        let metadata = format!("source: {}\n{}: {}", source.to_string(), key, value);
        let tmp_file_path = temp_dir().join("temp.jar");
        let mut tmp_file = File::create(tmp_file_path.clone())?;
        let mut zipwriter = ZipWriter::new(&mut tmp_file);
        let options: FileOptions<()> = FileOptions::default();
        for i in 0..zip.len() {
            let mut file = zip.by_index(i)?;
            let mut contents = Vec::new();
            file.read_to_end(&mut contents)?;

            zipwriter.start_file(file.name(), options)?;
            zipwriter.write_all(&contents)?;
        }
        zipwriter.start_file("META-INF/MODDER-RS.MF", options)?;
        zipwriter.write_all(metadata.as_bytes())?;
        zipwriter.finish()?;
        fs::copy(tmp_file_path.clone(), path.clone()).unwrap();
        fs::remove_file(tmp_file_path).unwrap();

        Ok(())
    }

    pub fn get_source(path: PathBuf) -> Result<Source> {
        let mut file = File::open(path)?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;
        let mut zip = zip::ZipArchive::new(Cursor::new(buffer))?;
        let mut metadata = zip.by_name("META-INF/MODDER-RS.MF")?;
        let mut contents = Vec::new();
        metadata.read_to_end(&mut contents)?;
        let metadata = String::from_utf8(contents)?;
        let source = metadata
            .lines()
            .find(|l| l.split(":").next().unwrap_or("") == "source")
            .unwrap()
            .split(":")
            .collect_vec()[1];
        Ok(source.try_into().unwrap_or(Source::Modrinth))
    }
    pub fn get_kv(path: PathBuf, key: &str) -> Result<String> {
        let mut file = File::open(path)?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;
        let mut zip = zip::ZipArchive::new(Cursor::new(buffer))?;
        let mut metadata = zip.by_name("META-INF/MODDER-RS.MF")?;
        let mut contents = Vec::new();
        metadata.read_to_end(&mut contents)?;
        let metadata = String::from_utf8(contents)?;
        let kv = metadata
            .lines()
            .find(|l| l.split(":").next().unwrap_or("") == key);
        match kv {
            Some(kv) => Ok(kv.split(":").collect_vec()[1].to_string()),
            None => Err(Error::NoKeyFound),
        }
    }
    pub fn get_all_metadata(path: PathBuf) -> Result<HashMap<String, String>> {
        let mut file = File::open(path)?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;
        let mut zip = zip::ZipArchive::new(Cursor::new(buffer))?;
        let mut metadata = zip.by_name("META-INF/MODDER-RS.MF")?;
        let mut contents = Vec::new();
        metadata.read_to_end(&mut contents)?;
        let metadata = String::from_utf8(contents)?;
        let hashmap = metadata
            .lines()
            .map(|l| {
                let split = l.split(":").map(str::trim).collect_vec();
                (split[0].to_string(), split[1].to_string())
            })
            .collect::<HashMap<String, String>>();
        Ok(hashmap)
    }
}
