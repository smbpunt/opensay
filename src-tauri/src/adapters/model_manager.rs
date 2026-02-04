use std::fs::{self, File};
use std::io::{BufReader, Read};
use std::path::PathBuf;

use async_trait::async_trait;
use parking_lot::RwLock;
use sha2::{Digest, Sha256};
use tracing::{debug, info, warn};

use crate::adapters::PrivacyGuard;
use crate::domain::{
    DomainError, DownloadProgress, InstalledModel, ModelCatalog, Quantization,
};
use crate::ports::{HttpClient, ModelManager};

/// Embedded model catalog JSON.
const CATALOG_JSON: &str = include_str!("../../resources/model_catalog.json");

/// Local model manager using filesystem storage.
pub struct LocalModelManager {
    catalog: ModelCatalog,
    models_dir: PathBuf,
    installed: RwLock<Vec<InstalledModel>>,
}

impl LocalModelManager {
    /// Create a new local model manager.
    pub fn new(data_dir: PathBuf) -> Result<Self, DomainError> {
        // Parse embedded catalog
        let catalog: ModelCatalog = serde_json::from_str(CATALOG_JSON)
            .map_err(|e| DomainError::Model(format!("Failed to parse model catalog: {}", e)))?;

        let models_dir = data_dir.join("models");
        fs::create_dir_all(&models_dir)?;

        let manager = Self {
            catalog,
            models_dir,
            installed: RwLock::new(Vec::new()),
        };

        // Scan for installed models
        manager.scan_installed()?;

        info!(
            models_dir = ?manager.models_dir,
            catalog_version = manager.catalog.version,
            installed_count = manager.installed.read().len(),
            "LocalModelManager initialized"
        );

        Ok(manager)
    }

    /// Scan the models directory for installed models.
    fn scan_installed(&self) -> Result<(), DomainError> {
        let mut installed = self.installed.write();
        installed.clear();

        if !self.models_dir.exists() {
            return Ok(());
        }

        for entry in fs::read_dir(&self.models_dir)? {
            let entry = entry?;
            let path = entry.path();

            if !path.is_file() {
                continue;
            }

            let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

            // Parse filename: {model_id}-{quantization}.bin
            if let Some(model) = self.parse_model_file(filename, &path) {
                debug!(model_id = %model.id, quant = %model.quantization, "Found installed model");
                installed.push(model);
            }
        }

        Ok(())
    }

    /// Parse a model filename into an InstalledModel.
    /// Expected format: {model_id}-{quant}.bin (our download naming scheme)
    fn parse_model_file(&self, filename: &str, path: &PathBuf) -> Option<InstalledModel> {
        let stem = filename.strip_suffix(".bin")?;

        // Parse: {model_id}-{quant} where model_id may contain hyphens
        let last_hyphen = stem.rfind('-')?;
        let model_id = &stem[..last_hyphen];
        let quant_str = &stem[last_hyphen + 1..];

        let quant = Quantization::from_suffix(quant_str)?;
        let model_info = self.catalog.get(model_id)?;
        let variant = model_info.variant(quant)?;
        let size = fs::metadata(path).ok()?.len();

        Some(InstalledModel {
            id: model_id.to_string(),
            quantization: quant,
            path: path.clone(),
            sha256: variant.sha256.clone(),
            size_bytes: size,
        })
    }

    /// Get the path for a model file.
    fn get_model_path(&self, model_id: &str, quant: Quantization) -> PathBuf {
        self.models_dir
            .join(format!("{}-{}.bin", model_id, quant.suffix()))
    }

    /// Calculate SHA-256 hash of a file.
    fn calculate_sha256(path: &PathBuf) -> Result<String, DomainError> {
        let file = File::open(path)?;
        let mut reader = BufReader::new(file);
        let mut hasher = Sha256::new();

        let mut buffer = [0u8; 8192];
        loop {
            let bytes_read = reader
                .read(&mut buffer)
                .map_err(|e| DomainError::Io(e.to_string()))?;
            if bytes_read == 0 {
                break;
            }
            hasher.update(&buffer[..bytes_read]);
        }

        let result = hasher.finalize();
        Ok(format!("{:x}", result))
    }
}

#[async_trait]
impl ModelManager for LocalModelManager {
    fn catalog(&self) -> &ModelCatalog {
        &self.catalog
    }

    fn list_installed(&self) -> Result<Vec<InstalledModel>, DomainError> {
        Ok(self.installed.read().clone())
    }

    fn is_installed(&self, model_id: &str, quant: Quantization) -> bool {
        self.installed
            .read()
            .iter()
            .any(|m| m.id == model_id && m.quantization == quant)
    }

    fn model_path(&self, model_id: &str, quant: Quantization) -> Option<PathBuf> {
        self.installed
            .read()
            .iter()
            .find(|m| m.id == model_id && m.quantization == quant)
            .map(|m| m.path.clone())
    }

    async fn download(
        &self,
        model_id: &str,
        quant: Quantization,
        progress: Option<Box<dyn Fn(DownloadProgress) + Send + Sync>>,
    ) -> Result<InstalledModel, DomainError> {
        // Look up model in catalog
        let model_info = self
            .catalog
            .get(model_id)
            .ok_or_else(|| DomainError::ModelNotFound(model_id.to_string()))?;

        let variant = model_info
            .variant(quant)
            .ok_or_else(|| {
                DomainError::ModelNotFound(format!(
                    "Model {} has no {} variant",
                    model_id,
                    quant.suffix()
                ))
            })?;

        let target_path = self.get_model_path(model_id, quant);

        info!(
            model_id = model_id,
            quant = %quant,
            url = %variant.url,
            target = ?target_path,
            "Starting model download"
        );

        // Create progress wrapper
        let progress_wrapper: Option<Box<dyn Fn(u64, u64) + Send + Sync>> = progress.map(|p| {
            let model_id = model_id.to_string();
            let wrapper: Box<dyn Fn(u64, u64) + Send + Sync> = Box::new(move |downloaded, total| {
                let mut dp = DownloadProgress::new(model_id.clone(), quant);
                dp.update(downloaded, total);
                p(dp);
            });
            wrapper
        });

        // Download via PrivacyGuard
        PrivacyGuard::global()
            .download_file(&variant.url, &target_path, progress_wrapper)
            .await?;

        // Verify checksum
        info!(target = ?target_path, "Download complete, verifying checksum");
        let actual_sha256 = Self::calculate_sha256(&target_path)?;
        if actual_sha256 != variant.sha256 {
            // Delete the corrupted file
            let _ = fs::remove_file(&target_path);
            return Err(DomainError::ModelVerification {
                expected: variant.sha256.clone(),
                actual: actual_sha256,
            });
        }

        let size = fs::metadata(&target_path)?.len();
        let installed = InstalledModel {
            id: model_id.to_string(),
            quantization: quant,
            path: target_path,
            sha256: variant.sha256.clone(),
            size_bytes: size,
        };

        // Add to installed list
        self.installed.write().push(installed.clone());

        info!(
            model_id = model_id,
            quant = %quant,
            size_mb = size / (1024 * 1024),
            "Model installed successfully"
        );

        Ok(installed)
    }

    fn verify(&self, model_id: &str, quant: Quantization) -> Result<bool, DomainError> {
        let path = self
            .model_path(model_id, quant)
            .ok_or_else(|| DomainError::ModelNotFound(format!("{}-{}", model_id, quant)))?;

        let model_info = self
            .catalog
            .get(model_id)
            .ok_or_else(|| DomainError::ModelNotFound(model_id.to_string()))?;

        let variant = model_info
            .variant(quant)
            .ok_or_else(|| {
                DomainError::ModelNotFound(format!("{} has no {} variant", model_id, quant))
            })?;

        let actual_sha256 = Self::calculate_sha256(&path)?;
        let valid = actual_sha256 == variant.sha256;

        if !valid {
            warn!(
                model_id = model_id,
                expected = %variant.sha256,
                actual = %actual_sha256,
                "Model verification failed"
            );
        }

        Ok(valid)
    }

    fn delete(&self, model_id: &str, quant: Quantization) -> Result<(), DomainError> {
        let path = self
            .model_path(model_id, quant)
            .ok_or_else(|| DomainError::ModelNotFound(format!("{}-{}", model_id, quant)))?;

        fs::remove_file(&path)?;

        // Remove from installed list
        let mut installed = self.installed.write();
        installed.retain(|m| !(m.id == model_id && m.quantization == quant));

        info!(model_id = model_id, quant = %quant, "Model deleted");
        Ok(())
    }

    fn models_dir(&self) -> PathBuf {
        self.models_dir.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_catalog_parsing() {
        let catalog: ModelCatalog = serde_json::from_str(CATALOG_JSON).unwrap();
        assert!(catalog.version >= 1);
        assert!(!catalog.models.is_empty());

        // Check that whisper-small exists
        let small = catalog.get("whisper-small");
        assert!(small.is_some());
    }

    #[test]
    fn test_model_path_generation() {
        let temp_dir = env::temp_dir().join("opensay_model_test");
        let manager = LocalModelManager::new(temp_dir.clone()).unwrap();

        let path = manager.get_model_path("whisper-small", Quantization::Q5_1);
        assert!(path.to_string_lossy().contains("whisper-small-q5_1.bin"));

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
    }
}
