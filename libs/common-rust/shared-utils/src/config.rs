use anyhow::{Context, Result};
use dotenvy::dotenv;
use std::env;

const DEFAULT_PORT: u16 = 3000;
const DEFAULT_HOST: &str = "127.0.0.1";
const DEFAULT_SERVICE_NAME: &str = "unknown_service";
const DEFAULT_OTLP_ENDPOINT: &str = "http://localhost:9090/api/v1/otlp/v1/metrics";

#[derive(Debug)]
pub struct Config {
    pub port: u16,
    pub host: String,
    pub otlp_endpoint: String,
    pub service_name: String,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            port: Self::get_port()?,
            host: Self::get_host()?,
            otlp_endpoint: Self::get_otlp_endpoint()?,
            service_name: Self::get_service_name()?,
        })
    }

    fn get_port() -> Result<u16> {
        match env::var("PORT") {
            Ok(port_str) => {
                let port = port_str
                    .parse::<u16>()
                    .with_context(|| format!("Invalid PORT: {}", port_str))?;
                if port == 0 {
                    return Err(anyhow::anyhow!("Port cannot be 0"));
                }
                Ok(port)
            }
            Err(_) => Ok(DEFAULT_PORT),
        }
    }

    fn get_host() -> Result<String> {
        let host = env::var("HOST").unwrap_or_else(|_| DEFAULT_HOST.to_string());
        if host.trim().is_empty() {
            return Err(anyhow::anyhow!("Host cannot be empty"));
        }

        Ok(host)
    }

    fn get_otlp_endpoint() -> Result<String> {
        let endpoint =
            env::var("OTLP_ENDPOINT").unwrap_or_else(|_| DEFAULT_OTLP_ENDPOINT.to_string());

        if !endpoint.starts_with("http://") && !endpoint.starts_with("https://") {
            return Err(anyhow::anyhow!("OTLP endpoint must be HTTP/HTTPS"));
        }
        Ok(endpoint)
    }

    fn get_service_name() -> Result<String> {
        let base_name =
            env::var("SERVICE_NAME").unwrap_or_else(|_| DEFAULT_SERVICE_NAME.to_string());

        let trimmed = base_name.trim();
        if trimmed.is_empty() {
            return Err(anyhow::anyhow!("Service name cannot be empty"));
        }

        Ok(trimmed.to_string())
    }

    pub fn bind_address(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }

    pub fn log_config(&self) {
        tracing::info!(
            host=%self.host, port=%self.port, service=%self.service_name,
            version=%env!("CARGO_PKG_VERSION"), "✅ Configuration"
        );
        tracing::info!("  OTLP Endpoint: {}", self.otlp_endpoint);
    }
}

pub fn config() -> anyhow::Result<Config> {
    dotenv().ok();

    Config::from_env()
        .inspect(|config| {
            tracing::debug!("Configuration loaded successfully");
            tracing::debug!("Service name: {}", config.service_name);
        })
        .map_err(|e| anyhow::anyhow!("Failed to load configuration: {}", e))
}
