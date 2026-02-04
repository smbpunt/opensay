use async_trait::async_trait;
use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::domain::DomainError;

/// HTTP client port for all network requests.
/// All network traffic must go through this interface.
#[async_trait]
pub trait HttpClient: Send + Sync {
    /// Perform a GET request.
    async fn get(&self, url: &str) -> Result<String, DomainError>;

    /// Perform a GET request and deserialize the response as JSON.
    async fn get_json<T: DeserializeOwned>(&self, url: &str) -> Result<T, DomainError>;

    /// Perform a POST request with JSON body.
    async fn post_json<T: Serialize + Send + Sync, R: DeserializeOwned>(
        &self,
        url: &str,
        body: &T,
    ) -> Result<R, DomainError>;

    /// Download a file to a specified path.
    async fn download_file(
        &self,
        url: &str,
        path: &std::path::Path,
        progress_callback: Option<Box<dyn Fn(u64, u64) + Send + Sync>>,
    ) -> Result<(), DomainError>;

    /// Check if network requests are currently blocked.
    fn is_network_blocked(&self) -> bool;

    /// Get the list of allowed domains (when not in local-only mode).
    fn allowed_domains(&self) -> Vec<String>;
}
