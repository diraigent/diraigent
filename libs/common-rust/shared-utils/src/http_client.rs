use anyhow::{Context, Result};
use serde::de::DeserializeOwned;
use std::time::Duration;

pub struct HttpClientBuilder {
    timeout: Duration,
    user_agent: String,
}

impl HttpClientBuilder {
    pub fn new() -> Self {
        Self {
            timeout: Duration::from_secs(30),
            user_agent: "zivue-services/1.0".to_string(),
        }
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn user_agent(mut self, user_agent: impl Into<String>) -> Self {
        self.user_agent = user_agent.into();
        self
    }

    pub fn build(self) -> Result<reqwest::Client> {
        reqwest::Client::builder()
            .timeout(self.timeout)
            .user_agent(self.user_agent)
            .build()
            .context("Failed to build HTTP client")
    }
}

impl Default for HttpClientBuilder {
    fn default() -> Self {
        Self::new()
    }
}

pub fn optional_api_key(env_var: &str) -> Option<String> {
    std::env::var(env_var).ok().filter(|k| !k.trim().is_empty())
}

pub async fn get_json_with_bearer<T: DeserializeOwned>(
    client: &reqwest::Client,
    url: &str,
    token: &str,
) -> Result<T> {
    let response = client
        .get(url)
        .bearer_auth(token)
        .send()
        .await
        .with_context(|| format!("Failed to send request to {}", url))?
        .error_for_status()
        .with_context(|| format!("HTTP error from {}", url))?;

    response
        .json::<T>()
        .await
        .with_context(|| format!("Failed to parse JSON response from {}", url))
}
