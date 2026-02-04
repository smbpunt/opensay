use std::sync::OnceLock;

use tracing::{debug, info};

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
use tracing::warn;

use crate::domain::{
    CpuArch, DomainError, HardwareProfile, ModelCatalog, ModelRecommendation, OsType,
    Quantization, SimdCapabilities,
};
use crate::ports::HardwareDetector;

/// CPU-based hardware detector.
///
/// Detects CPU architecture, cores, SIMD capabilities, and RAM.
/// Results are cached after the first detection.
pub struct CpuHardwareDetector {
    profile: OnceLock<HardwareProfile>,
}

impl CpuHardwareDetector {
    /// Create a new hardware detector.
    pub fn new() -> Self {
        Self {
            profile: OnceLock::new(),
        }
    }

    /// Perform the actual hardware detection.
    fn detect_hardware() -> Result<HardwareProfile, DomainError> {
        let arch = CpuArch::detect();
        let simd = SimdCapabilities::detect();
        let os = OsType::detect();

        // Get CPU thread count (logical processors)
        let threads = std::thread::available_parallelism()
            .map(|p| p.get() as u32)
            .unwrap_or(1);

        // Use thread count as core count since hyperthreading detection is unreliable
        // (Apple Silicon doesn't use HT, AMD has different HT ratios).
        // For transcription workload, using all threads is generally fine.
        let cores = threads;

        // Detect RAM
        let ram_bytes = Self::detect_ram()?;

        let profile = HardwareProfile {
            arch,
            cores,
            threads,
            simd,
            ram_bytes,
            os,
        };

        info!(
            arch = %profile.arch,
            cores = profile.cores,
            threads = profile.threads,
            ram_gb = profile.ram_gb(),
            avx2 = profile.simd.avx2,
            neon = profile.simd.neon,
            "Hardware profile detected"
        );

        Ok(profile)
    }

    /// Detect total system RAM.
    #[cfg(target_os = "macos")]
    fn detect_ram() -> Result<u64, DomainError> {
        use std::process::Command;

        let output = Command::new("sysctl")
            .args(["-n", "hw.memsize"])
            .output()
            .map_err(|e| DomainError::Hardware(format!("Failed to run sysctl: {}", e)))?;

        if !output.status.success() {
            return Err(DomainError::Hardware("sysctl command failed".to_string()));
        }

        let mem_str = String::from_utf8_lossy(&output.stdout);
        let ram_bytes: u64 = mem_str
            .trim()
            .parse()
            .map_err(|e| DomainError::Hardware(format!("Failed to parse memory size: {}", e)))?;

        debug!(ram_bytes, "Detected RAM via sysctl");
        Ok(ram_bytes)
    }

    /// Detect total system RAM.
    #[cfg(target_os = "linux")]
    fn detect_ram() -> Result<u64, DomainError> {
        use std::fs;

        let meminfo = fs::read_to_string("/proc/meminfo")
            .map_err(|e| DomainError::Hardware(format!("Failed to read /proc/meminfo: {}", e)))?;

        for line in meminfo.lines() {
            if line.starts_with("MemTotal:") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    let kb: u64 = parts[1].parse().map_err(|e| {
                        DomainError::Hardware(format!("Failed to parse MemTotal: {}", e))
                    })?;
                    let ram_bytes = kb * 1024;
                    debug!(ram_bytes, "Detected RAM via /proc/meminfo");
                    return Ok(ram_bytes);
                }
            }
        }

        Err(DomainError::Hardware(
            "Could not find MemTotal in /proc/meminfo".to_string(),
        ))
    }

    /// Detect total system RAM.
    #[cfg(target_os = "windows")]
    fn detect_ram() -> Result<u64, DomainError> {
        use std::process::Command;

        let output = Command::new("wmic")
            .args(["ComputerSystem", "get", "TotalPhysicalMemory"])
            .output()
            .map_err(|e| DomainError::Hardware(format!("Failed to run wmic: {}", e)))?;

        if !output.status.success() {
            return Err(DomainError::Hardware("wmic command failed".to_string()));
        }

        let output_str = String::from_utf8_lossy(&output.stdout);
        for line in output_str.lines() {
            let trimmed = line.trim();
            if !trimmed.is_empty() && trimmed != "TotalPhysicalMemory" {
                let ram_bytes: u64 = trimmed.parse().map_err(|e| {
                    DomainError::Hardware(format!("Failed to parse memory size: {}", e))
                })?;
                debug!(ram_bytes, "Detected RAM via wmic");
                return Ok(ram_bytes);
            }
        }

        Err(DomainError::Hardware(
            "Could not parse wmic output".to_string(),
        ))
    }

    /// Detect total system RAM (fallback for unsupported platforms).
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    fn detect_ram() -> Result<u64, DomainError> {
        warn!("RAM detection not supported on this platform, defaulting to 8GB");
        Ok(8 * 1024 * 1024 * 1024)
    }
}

impl Default for CpuHardwareDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl HardwareDetector for CpuHardwareDetector {
    fn detect(&self) -> Result<HardwareProfile, DomainError> {
        if let Some(profile) = self.profile.get() {
            return Ok(profile.clone());
        }

        let profile = Self::detect_hardware()?;
        // Try to set, but don't fail if another thread beat us
        let _ = self.profile.set(profile.clone());
        Ok(profile)
    }

    fn recommend_model(&self, catalog: &ModelCatalog) -> Result<ModelRecommendation, DomainError> {
        let profile = self.profile()?;
        let ram_gb = profile.ram_gb();
        let has_good_simd = profile.simd.has_good_simd();

        // Recommendation logic:
        // - RAM < 4GB: tiny (Q5_1)
        // - RAM < 8GB: base (Q5_1)
        // - RAM >= 8GB with good SIMD: small (Q5_1, default)
        // - RAM >= 16GB: could use medium/large, but small is still default
        // Note: tiny/base/small use Q5_1, medium/large use Q5_0
        let (model_id, quantization, reason) = if ram_gb < 4 {
            (
                "whisper-tiny",
                Quantization::Q5_1,
                "Limited RAM (< 4GB) - using smallest model".to_string(),
            )
        } else if ram_gb < 8 {
            (
                "whisper-base",
                Quantization::Q5_1,
                format!("Moderate RAM ({} GB) - using base model", ram_gb),
            )
        } else if has_good_simd {
            (
                "whisper-small",
                Quantization::Q5_1,
                format!(
                    "Good hardware ({} GB RAM, {} SIMD) - recommended model",
                    ram_gb,
                    if profile.simd.avx2 {
                        "AVX2"
                    } else {
                        "NEON"
                    }
                ),
            )
        } else {
            (
                "whisper-small",
                Quantization::Q5_1,
                format!("{} GB RAM - using recommended model", ram_gb),
            )
        };

        // Verify the model exists in catalog
        if catalog.get(model_id).is_none() {
            return Err(DomainError::ModelNotFound(format!(
                "Recommended model '{}' not found in catalog",
                model_id
            )));
        }

        Ok(ModelRecommendation {
            model_id: model_id.to_string(),
            quantization,
            reason,
        })
    }

    fn profile(&self) -> Result<&HardwareProfile, DomainError> {
        if let Some(profile) = self.profile.get() {
            return Ok(profile);
        }

        // Need to detect first
        let profile = Self::detect_hardware()?;
        // This might race, but that's fine - we'll get a valid profile either way
        let _ = self.profile.set(profile);
        self.profile
            .get()
            .ok_or_else(|| DomainError::Hardware("Failed to cache hardware profile".to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hardware_detection() {
        let detector = CpuHardwareDetector::new();
        let profile = detector.detect().unwrap();

        assert!(profile.threads >= 1);
        assert!(profile.ram_bytes > 0);
    }

    #[test]
    fn test_profile_caching() {
        let detector = CpuHardwareDetector::new();

        let profile1 = detector.detect().unwrap();
        let profile2 = detector.detect().unwrap();

        // Should return the same cached profile
        assert_eq!(profile1.threads, profile2.threads);
        assert_eq!(profile1.ram_bytes, profile2.ram_bytes);
    }
}
