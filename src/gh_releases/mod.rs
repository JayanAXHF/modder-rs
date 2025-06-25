use crate::UrlBuilder;

mod structs;

const GH_RELEASES_API: &str = "https://api.github.com/repos";

#[derive(Default)]
pub struct GHReleasesAPI {
    pub client: reqwest::Client,
    pub token: Option<Box<str>>,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Error sending the request. This may mean that the request was malformed: {0:?}")]
    Reqwest(#[from] reqwest::Error),
    #[error("Error deserializing the response: {0:?}")]
    Serde(#[from] serde_json::Error),
    #[error("No releases found")]
    NoReleases,
    #[error("Authorization failed: {0}")]
    AuthFailed(String),
    #[error("Mod not found for the particular game version or loader")]
    ModNotFound,
    #[error("Error writing the mod to a file: {0}")]
    WriteFileErr(#[from] std::io::Error),
}
type Result<T> = std::result::Result<T, Error>;

impl GHReleasesAPI {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
            token: None,
        }
    }
    pub fn token(&mut self, token: String) {
        self.token = Some(token.into_boxed_str());
    }
    #[tracing::instrument(level = "info", skip(self))]
    pub async fn get_releases(&self, owner: &str, repo: &str) -> Result<Vec<structs::Release>> {
        let url = UrlBuilder::new(GH_RELEASES_API, &format!("/{}/{}/releases", owner, repo));
        let mut headers = reqwest::header::HeaderMap::new();
        let response = self.client.get(url.to_string());
        if let Some(token) = self.token.as_ref() {
            headers.insert(
                reqwest::header::AUTHORIZATION,
                reqwest::header::HeaderValue::from_str(&format!("Bearer {}", token)).unwrap(),
            );
        }
        headers.insert(
            reqwest::header::USER_AGENT,
            reqwest::header::HeaderValue::from_static("modder-rs"),
        );
        let response = response.headers(headers).send().await?;
        let response = match response.error_for_status() {
            Ok(response) => Ok(response),
            Err(e) => {
                let code = e.status().unwrap().as_u16();
                if code == 401 || code == 403 {
                    Err(Error::AuthFailed(e.to_string()))
                } else {
                    return Err(Error::Reqwest(e));
                }
            }
        }?;
        let res_text: String = response.text().await?;
        let releases: Vec<structs::Release> = serde_json::from_str(&res_text)?;
        if releases.is_empty() {
            return Err(Error::NoReleases);
        }

        Ok(releases)
    }
}

pub async fn get_mod_from_release(
    releases: &[structs::Release],
    loader: &str,
    version: &str,
) -> Result<structs::ReleaseAsset> {
    let mut found = false;
    let mut asset_found = None;
    let correct_asset = {
        for release in releases {
            let asset = release
                .assets
                .iter()
                .find(|asset| asset.name.contains(loader) && asset.name.contains(version));
            if asset.is_some() {
                found = true;
                asset_found = asset;
                break;
            }
        }
        if found { asset_found } else { None }
    };
    match correct_asset {
        Some(release) => Ok(release.clone()),
        None => Err(Error::ModNotFound),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn test_get_mod_from_release() {
        let gh_api = GHReleasesAPI::new();
        let releases = gh_api.get_releases("fabricmc", "fabric").await.unwrap();
        let r1_21_4 = get_mod_from_release(&releases, "fabric", "1.21.4").await;
        println!("{:#?}", r1_21_4);
        assert!(r1_21_4.is_ok());
    }
}
