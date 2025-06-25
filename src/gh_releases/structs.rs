use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};
use url::Url;

use crate::{cli::Source, metadata::Metadata};

use super::Error;

type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Release {
    pub url: Url,
    pub html_url: Url,
    pub assets_url: Url,
    pub upload_url: String,
    pub tarball_url: Option<Url>,
    pub zipball_url: Option<Url>,
    pub id: u64,
    pub node_id: String,
    pub tag_name: String,
    pub target_commitish: String,
    pub name: Option<String>,
    pub body: Option<String>,
    pub draft: bool,
    pub prerelease: bool,
    pub created_at: DateTime<Utc>,
    pub published_at: Option<DateTime<Utc>>,
    pub author: User,
    pub assets: Vec<ReleaseAsset>,
    pub body_html: Option<String>,
    pub body_text: Option<String>,
    pub mentions_count: Option<i64>,
    pub discussion_url: Option<Url>,
    pub reactions: Option<ReactionRollup>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct User {
    pub name: Option<String>,
    pub email: Option<String>,
    pub login: String,
    pub id: u64,
    pub node_id: String,
    pub avatar_url: Url,
    pub gravatar_id: Option<String>,
    pub url: Url,
    pub html_url: Url,
    pub followers_url: Url,
    pub following_url: String,
    pub gists_url: String,
    pub starred_url: Option<String>,
    pub subscriptions_url: Url,
    pub organizations_url: Url,
    pub repos_url: Url,
    pub events_url: String,
    pub received_events_url: Url,
    pub r#type: String,
    pub site_admin: bool,
    pub starred_at: Option<String>,
    pub user_view_type: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ReleaseAsset {
    pub url: Url,
    pub browser_download_url: Url,
    pub id: u64,
    pub node_id: String,
    pub name: String,
    pub label: Option<String>,
    pub state: AssetState,
    pub content_type: String,
    pub size: i64,
    pub digest: Option<String>,
    pub download_count: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub uploader: Option<User>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "lowercase")]
pub enum AssetState {
    Uploaded,
    Open,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ReactionRollup {
    pub url: Url,
    pub total_count: i64,
    #[serde(rename = "+1")]
    pub plus_one: i64,
    #[serde(rename = "-1")]
    pub minus_one: i64,
    pub laugh: i64,
    pub confused: i64,
    pub heart: i64,
    pub hooray: i64,
    pub eyes: i64,
    pub rocket: i64,
}

impl ReleaseAsset {
    pub fn get_download_url(&self) -> Option<Url> {
        Some(self.browser_download_url.clone())
    }
    pub async fn download(&self, path: PathBuf, repo: String) -> Result<()> {
        let url = self.get_download_url().expect("Asset has no download url");
        let file_content = reqwest::get(url.clone()).await.unwrap();
        fs::write(&path, file_content.bytes().await.unwrap())?;
        let handle = tokio::spawn(async move {
            /// Adds metadata to the file for later use with `update` option
            Metadata::add_metadata(path.clone(), Source::Github, "repo", &repo).unwrap();
        });
        handle.await.unwrap();
        Ok(())
    }
}
