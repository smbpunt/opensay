use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Quantization level for GGUF models.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Quantization {
    /// 4-bit quantization - smallest size, lowest quality.
    Q4_0,
    /// 5-bit quantization (variant 0) - good balance.
    Q5_0,
    /// 5-bit quantization (variant 1) - slightly better quality than Q5_0.
    Q5_1,
    /// 8-bit quantization - better quality, larger size.
    Q8_0,
    /// 16-bit float - highest quality, largest size.
    F16,
}

impl Quantization {
    /// Get the file suffix for this quantization level.
    pub fn suffix(&self) -> &'static str {
        match self {
            Quantization::Q4_0 => "q4_0",
            Quantization::Q5_0 => "q5_0",
            Quantization::Q5_1 => "q5_1",
            Quantization::Q8_0 => "q8_0",
            Quantization::F16 => "f16",
        }
    }

    /// Parse quantization from a string suffix.
    pub fn from_suffix(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "q4_0" => Some(Quantization::Q4_0),
            "q5_0" => Some(Quantization::Q5_0),
            "q5_1" => Some(Quantization::Q5_1),
            "q8_0" => Some(Quantization::Q8_0),
            "f16" => Some(Quantization::F16),
            _ => None,
        }
    }
}

impl std::fmt::Display for Quantization {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.suffix())
    }
}

/// A specific variant of a model with a particular quantization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelVariant {
    /// Quantization level.
    pub quantization: Quantization,
    /// File size in bytes.
    pub size_bytes: u64,
    /// SHA-256 checksum of the file.
    pub sha256: String,
    /// Download URL.
    pub url: String,
}

/// Information about a model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    /// Unique identifier (e.g., "whisper-small").
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// Description of the model.
    pub description: String,
    /// Minimum RAM required in GB.
    pub min_ram_gb: u32,
    /// Available variants (quantization levels).
    pub variants: Vec<ModelVariant>,
}

impl ModelInfo {
    /// Find a specific variant by quantization.
    pub fn variant(&self, quant: Quantization) -> Option<&ModelVariant> {
        self.variants.iter().find(|v| v.quantization == quant)
    }

    /// Get the default (recommended) variant.
    pub fn default_variant(&self) -> Option<&ModelVariant> {
        // Prefer Q5_1 as a good balance, fall back to first available
        self.variant(Quantization::Q5_1)
            .or_else(|| self.variants.first())
    }
}

/// Catalog of available models.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelCatalog {
    /// Catalog version for compatibility checking.
    pub version: u32,
    /// Available models.
    pub models: Vec<ModelInfo>,
}

impl ModelCatalog {
    /// Find a model by ID.
    pub fn get(&self, model_id: &str) -> Option<&ModelInfo> {
        self.models.iter().find(|m| m.id == model_id)
    }

    /// List all model IDs.
    pub fn model_ids(&self) -> Vec<&str> {
        self.models.iter().map(|m| m.id.as_str()).collect()
    }
}

/// An installed model on the local filesystem.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledModel {
    /// Model ID.
    pub id: String,
    /// Quantization level of this installation.
    pub quantization: Quantization,
    /// Path to the model file.
    pub path: PathBuf,
    /// SHA-256 checksum (verified at install time).
    pub sha256: String,
    /// File size in bytes.
    pub size_bytes: u64,
}

/// Progress information for model download.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadProgress {
    /// Model being downloaded.
    pub model_id: String,
    /// Quantization level.
    pub quantization: Quantization,
    /// Bytes downloaded so far.
    pub bytes_downloaded: u64,
    /// Total bytes to download (0 if unknown).
    pub total_bytes: u64,
    /// Download progress as percentage (0.0 - 100.0).
    pub percent: f32,
}

impl DownloadProgress {
    /// Create a new download progress.
    pub fn new(model_id: String, quantization: Quantization) -> Self {
        Self {
            model_id,
            quantization,
            bytes_downloaded: 0,
            total_bytes: 0,
            percent: 0.0,
        }
    }

    /// Update progress with downloaded bytes.
    pub fn update(&mut self, downloaded: u64, total: u64) {
        self.bytes_downloaded = downloaded;
        self.total_bytes = total;
        self.percent = if total > 0 {
            (downloaded as f32 / total as f32) * 100.0
        } else {
            0.0
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quantization_suffix() {
        assert_eq!(Quantization::Q5_1.suffix(), "q5_1");
        assert_eq!(Quantization::from_suffix("Q5_1"), Some(Quantization::Q5_1));
    }

    #[test]
    fn test_download_progress() {
        let mut progress = DownloadProgress::new("whisper-small".to_string(), Quantization::Q5_1);
        progress.update(50, 100);
        assert_eq!(progress.percent, 50.0);
    }
}
