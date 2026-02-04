use std::path::PathBuf;

use async_trait::async_trait;

use crate::domain::{
    DomainError, DownloadProgress, InstalledModel, ModelCatalog, Quantization,
};

/// Port for model management operations.
///
/// Implementations handle model catalog, downloads, verification, and storage.
#[async_trait]
pub trait ModelManager: Send + Sync {
    /// Get the model catalog.
    fn catalog(&self) -> &ModelCatalog;

    /// List all installed models.
    fn list_installed(&self) -> Result<Vec<InstalledModel>, DomainError>;

    /// Check if a specific model variant is installed.
    fn is_installed(&self, model_id: &str, quant: Quantization) -> bool;

    /// Get the path to an installed model.
    ///
    /// Returns None if the model is not installed.
    fn model_path(&self, model_id: &str, quant: Quantization) -> Option<PathBuf>;

    /// Download and install a model.
    ///
    /// The progress callback is called periodically with download progress.
    async fn download(
        &self,
        model_id: &str,
        quant: Quantization,
        progress: Option<Box<dyn Fn(DownloadProgress) + Send + Sync>>,
    ) -> Result<InstalledModel, DomainError>;

    /// Verify the integrity of an installed model.
    ///
    /// Returns true if the model's SHA-256 checksum matches.
    fn verify(&self, model_id: &str, quant: Quantization) -> Result<bool, DomainError>;

    /// Delete an installed model.
    fn delete(&self, model_id: &str, quant: Quantization) -> Result<(), DomainError>;

    /// Get the models directory path.
    fn models_dir(&self) -> PathBuf;
}
