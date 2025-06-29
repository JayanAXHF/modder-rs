//!WARN: ON HOLD for now because this API is tom-fucking-beaurocratic shit
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
}
