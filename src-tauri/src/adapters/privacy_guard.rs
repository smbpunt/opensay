use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use async_trait::async_trait;
use once_cell::sync::OnceCell;
use parking_lot::RwLock;
use reqwest::Client;
use serde::de::DeserializeOwned;
use serde::Serialize;
use tracing::{info, warn};
use url::Url;

use crate::domain::config::PrivacyConfig;
use crate::domain::DomainError;
use crate::ports::HttpClient;

/// Global singleton instance of PrivacyGuard.
static INSTANCE: OnceCell<PrivacyGuard> = OnceCell::new();

/// PrivacyGuard is an internal firewall that controls all HTTP requests.
/// In local-only mode (default), all network requests are blocked.
/// When network access is enabled, only whitelisted domains are allowed.
pub struct PrivacyGuard {
    client: Client,
    local_only: AtomicBool,
    allowed_domains: RwLock<Vec<String>>,
}

impl PrivacyGuard {
    /// Get the global PrivacyGuard instance.
    /// Creates a new instance with default settings if none exists.
    /// Panics if HTTP client creation fails (should not happen in practice).
    pub fn global() -> &'static PrivacyGuard {
        INSTANCE.get_or_init(|| {
            Self::try_new()
                .expect("Failed to create HTTP client - this should not happen")
        })
    }

    /// Initialize the global PrivacyGuard with custom settings.
    /// Returns error if already initialized or HTTP client creation fails.
    pub fn init(local_only: bool, allowed_domains: Vec<String>) -> Result<&'static PrivacyGuard, DomainError> {
        let guard = Self::try_with_config(local_only, allowed_domains)?;
        INSTANCE
            .set(guard)
            .map_err(|_| DomainError::Config("PrivacyGuard already initialized".to_string()))?;
        Ok(INSTANCE.get().unwrap())
    }

    /// Create a new PrivacyGuard with default settings (local-only mode).
    fn try_new() -> Result<Self, DomainError> {
        Self::try_with_config(true, Self::default_allowed_domains())
    }

    /// Create a new PrivacyGuard with custom settings.
    fn try_with_config(local_only: bool, allowed_domains: Vec<String>) -> Result<Self, DomainError> {
        let client = Client::builder()
            .use_rustls_tls()
            .user_agent(format!("OpenSay/{}", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|e| DomainError::HttpRequest(format!("Failed to create HTTP client: {}", e)))?;

        info!(
            local_only = local_only,
            allowed_domains = ?allowed_domains,
            "PrivacyGuard initialized"
        );

        Ok(Self {
            client,
            local_only: AtomicBool::new(local_only),
            allowed_domains: RwLock::new(allowed_domains),
        })
    }

    /// Default allowed domains for API access.
    fn default_allowed_domains() -> Vec<String> {
        PrivacyConfig::default_allowed_domains()
    }

    /// Set local-only mode.
    pub fn set_local_only(&self, local_only: bool) {
        let previous = self.local_only.swap(local_only, Ordering::SeqCst);
        if previous != local_only {
            info!(local_only = local_only, "PrivacyGuard mode changed");
        }
    }

    /// Update allowed domains.
    pub fn set_allowed_domains(&self, domains: Vec<String>) {
        let mut guard = self.allowed_domains.write();
        *guard = domains;
        info!(allowed_domains = ?*guard, "PrivacyGuard allowed domains updated");
    }

    /// Check if a URL is allowed based on current settings.
    fn is_url_allowed(&self, url: &str) -> Result<(), DomainError> {
        if self.local_only.load(Ordering::SeqCst) {
            warn!(url = url, "Network request blocked: local-only mode enabled");
            return Err(DomainError::NetworkBlocked {
                reason: "Local-only mode is enabled. All network requests are blocked.".to_string(),
            });
        }

        let parsed = Url::parse(url).map_err(|e| DomainError::HttpRequest(e.to_string()))?;
        let host = parsed
            .host_str()
            .ok_or_else(|| DomainError::HttpRequest("Invalid URL: no host".to_string()))?;

        let allowed = self.allowed_domains.read();
        if !allowed.iter().any(|d| host == d || host.ends_with(&format!(".{}", d))) {
            warn!(url = url, host = host, "Network request blocked: domain not in whitelist");
            return Err(DomainError::NetworkBlocked {
                reason: format!("Domain '{}' is not in the allowed list", host),
            });
        }

        info!(url = url, "Network request allowed");
        Ok(())
    }
}

#[async_trait]
impl HttpClient for PrivacyGuard {
    async fn get(&self, url: &str) -> Result<String, DomainError> {
        self.is_url_allowed(url)?;

        let response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| DomainError::HttpRequest(e.to_string()))?;

        let status = response.status();
        if !status.is_success() {
            return Err(DomainError::HttpRequest(format!(
                "HTTP {} for {}",
                status, url
            )));
        }

        response
            .text()
            .await
            .map_err(|e| DomainError::HttpRequest(e.to_string()))
    }

    async fn get_json<T: DeserializeOwned>(&self, url: &str) -> Result<T, DomainError> {
        self.is_url_allowed(url)?;

        let response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| DomainError::HttpRequest(e.to_string()))?;

        let status = response.status();
        if !status.is_success() {
            return Err(DomainError::HttpRequest(format!(
                "HTTP {} for {}",
                status, url
            )));
        }

        response
            .json()
            .await
            .map_err(|e| DomainError::HttpRequest(e.to_string()))
    }

    async fn post_json<T: Serialize + Send + Sync, R: DeserializeOwned>(
        &self,
        url: &str,
        body: &T,
    ) -> Result<R, DomainError> {
        self.is_url_allowed(url)?;

        let response = self
            .client
            .post(url)
            .json(body)
            .send()
            .await
            .map_err(|e| DomainError::HttpRequest(e.to_string()))?;

        let status = response.status();
        if !status.is_success() {
            return Err(DomainError::HttpRequest(format!(
                "HTTP {} for {}",
                status, url
            )));
        }

        response
            .json()
            .await
            .map_err(|e| DomainError::HttpRequest(e.to_string()))
    }

    async fn download_file(
        &self,
        url: &str,
        path: &Path,
        progress_callback: Option<Box<dyn Fn(u64, u64) + Send + Sync>>,
    ) -> Result<(), DomainError> {
        use futures_util::StreamExt;
        use tokio::io::AsyncWriteExt;

        self.is_url_allowed(url)?;

        let response = self
            .client
            .get(url)
            .timeout(std::time::Duration::from_secs(3600)) // 1 hour timeout for large models
            .send()
            .await
            .map_err(|e| DomainError::HttpRequest(e.to_string()))?;

        let status = response.status();
        if !status.is_success() {
            return Err(DomainError::HttpRequest(format!(
                "HTTP {} for {}",
                status, url
            )));
        }

        let total_size = response.content_length().unwrap_or(0);

        // Create parent directory if needed
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Write to temp file first, then rename atomically
        let temp_path = path.with_extension("download");

        // Helper to clean up temp file on error
        let cleanup_temp = || {
            let temp = temp_path.clone();
            async move { let _ = tokio::fs::remove_file(&temp).await; }
        };

        let mut file = match tokio::fs::File::create(&temp_path).await {
            Ok(f) => f,
            Err(e) => {
                cleanup_temp().await;
                return Err(DomainError::Io(e.to_string()));
            }
        };

        let mut downloaded: u64 = 0;
        let mut stream = response.bytes_stream();

        while let Some(chunk_result) = stream.next().await {
            let chunk = match chunk_result {
                Ok(c) => c,
                Err(e) => {
                    drop(file);
                    cleanup_temp().await;
                    return Err(DomainError::HttpRequest(e.to_string()));
                }
            };

            if let Err(e) = file.write_all(&chunk).await {
                drop(file);
                cleanup_temp().await;
                return Err(DomainError::Io(e.to_string()));
            }

            downloaded += chunk.len() as u64;

            if let Some(callback) = &progress_callback {
                callback(downloaded, total_size);
            }
        }

        if let Err(e) = file.flush().await {
            drop(file);
            cleanup_temp().await;
            return Err(DomainError::Io(e.to_string()));
        }
        drop(file);

        // Atomic rename from temp to final path
        if let Err(e) = tokio::fs::rename(&temp_path, path).await {
            cleanup_temp().await;
            return Err(DomainError::Io(e.to_string()));
        }

        info!(path = ?path, size = downloaded, "File downloaded successfully");
        Ok(())
    }

    fn is_network_blocked(&self) -> bool {
        self.local_only.load(Ordering::SeqCst)
    }

    fn allowed_domains(&self) -> Vec<String> {
        self.allowed_domains.read().clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_local_only_blocks_requests() {
        let guard = PrivacyGuard::try_with_config(true, vec!["example.com".to_string()]).unwrap();
        assert!(guard.is_network_blocked());

        let result = guard.is_url_allowed("https://example.com/api");
        assert!(result.is_err());
    }

    #[test]
    fn test_allowed_domain_passes() {
        let guard = PrivacyGuard::try_with_config(false, vec!["api.openai.com".to_string()]).unwrap();
        assert!(!guard.is_network_blocked());

        let result = guard.is_url_allowed("https://api.openai.com/v1/chat");
        assert!(result.is_ok());
    }

    #[test]
    fn test_disallowed_domain_blocked() {
        let guard = PrivacyGuard::try_with_config(false, vec!["api.openai.com".to_string()]).unwrap();

        let result = guard.is_url_allowed("https://malicious.com/steal");
        assert!(result.is_err());
    }

    #[test]
    fn test_subdomain_allowed() {
        let guard = PrivacyGuard::try_with_config(false, vec!["huggingface.co".to_string()]).unwrap();

        let result = guard.is_url_allowed("https://cdn-lfs.huggingface.co/file");
        assert!(result.is_ok());
    }
}
