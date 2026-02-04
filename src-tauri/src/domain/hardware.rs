use serde::{Deserialize, Serialize};

use super::model::Quantization;

/// CPU architecture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CpuArch {
    /// x86-64 (AMD64/Intel 64).
    X86_64,
    /// ARM64 (AArch64, Apple Silicon).
    Arm64,
    /// Unknown or unsupported architecture.
    Unknown,
}

impl CpuArch {
    /// Detect the current CPU architecture.
    pub fn detect() -> Self {
        match std::env::consts::ARCH {
            "x86_64" => CpuArch::X86_64,
            "aarch64" => CpuArch::Arm64,
            _ => CpuArch::Unknown,
        }
    }
}

impl std::fmt::Display for CpuArch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CpuArch::X86_64 => write!(f, "x86_64"),
            CpuArch::Arm64 => write!(f, "arm64"),
            CpuArch::Unknown => write!(f, "unknown"),
        }
    }
}

/// SIMD capabilities of the CPU.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct SimdCapabilities {
    /// x86: AVX support.
    pub avx: bool,
    /// x86: AVX2 support.
    pub avx2: bool,
    /// x86: AVX-512 support.
    pub avx512: bool,
    /// ARM: NEON support.
    pub neon: bool,
}

impl SimdCapabilities {
    /// Detect SIMD capabilities for the current CPU.
    #[cfg(target_arch = "x86_64")]
    pub fn detect() -> Self {
        Self {
            avx: std::arch::is_x86_feature_detected!("avx"),
            avx2: std::arch::is_x86_feature_detected!("avx2"),
            avx512: std::arch::is_x86_feature_detected!("avx512f"),
            neon: false,
        }
    }

    /// Detect SIMD capabilities for the current CPU.
    #[cfg(target_arch = "aarch64")]
    pub fn detect() -> Self {
        // NEON is mandatory on AArch64
        Self {
            avx: false,
            avx2: false,
            avx512: false,
            neon: true,
        }
    }

    /// Detect SIMD capabilities for the current CPU.
    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    pub fn detect() -> Self {
        Self::default()
    }

    /// Check if this CPU has good vector processing support.
    pub fn has_good_simd(&self) -> bool {
        self.avx2 || self.neon
    }
}

/// Operating system type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OsType {
    MacOS,
    Windows,
    Linux,
    Unknown,
}

impl OsType {
    /// Detect the current operating system.
    pub fn detect() -> Self {
        match std::env::consts::OS {
            "macos" => OsType::MacOS,
            "windows" => OsType::Windows,
            "linux" => OsType::Linux,
            _ => OsType::Unknown,
        }
    }
}

impl std::fmt::Display for OsType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OsType::MacOS => write!(f, "macOS"),
            OsType::Windows => write!(f, "Windows"),
            OsType::Linux => write!(f, "Linux"),
            OsType::Unknown => write!(f, "Unknown"),
        }
    }
}

/// Hardware profile of the system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareProfile {
    /// CPU architecture.
    pub arch: CpuArch,
    /// Number of physical CPU cores.
    pub cores: u32,
    /// Number of logical threads.
    pub threads: u32,
    /// SIMD capabilities.
    pub simd: SimdCapabilities,
    /// Total RAM in bytes.
    pub ram_bytes: u64,
    /// Operating system.
    pub os: OsType,
}

impl HardwareProfile {
    /// Get RAM in gigabytes.
    pub fn ram_gb(&self) -> u32 {
        (self.ram_bytes / (1024 * 1024 * 1024)) as u32
    }

    /// Get recommended thread count for transcription.
    /// Uses cores - 1 to leave one core for the system, minimum 1.
    pub fn recommended_threads(&self) -> u32 {
        std::cmp::max(1, self.cores.saturating_sub(1))
    }
}

/// Model recommendation based on hardware profile.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRecommendation {
    /// Recommended model ID.
    pub model_id: String,
    /// Recommended quantization level.
    pub quantization: Quantization,
    /// Reason for the recommendation.
    pub reason: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cpu_arch_detect() {
        let arch = CpuArch::detect();
        // Just verify it doesn't crash and returns something sensible
        assert!(matches!(arch, CpuArch::X86_64 | CpuArch::Arm64 | CpuArch::Unknown));
    }

    #[test]
    fn test_simd_detect() {
        let simd = SimdCapabilities::detect();
        // On modern x86_64, we expect at least AVX
        #[cfg(target_arch = "x86_64")]
        assert!(simd.avx || simd.avx2);

        // On ARM64 (Apple Silicon), NEON is always available
        #[cfg(target_arch = "aarch64")]
        assert!(simd.neon);
    }

    #[test]
    fn test_hardware_profile_threads() {
        let profile = HardwareProfile {
            arch: CpuArch::X86_64,
            cores: 8,
            threads: 8,
            simd: SimdCapabilities::default(),
            ram_bytes: 16 * 1024 * 1024 * 1024,
            os: OsType::MacOS,
        };
        // recommended_threads = cores - 1 = 7
        assert_eq!(profile.recommended_threads(), 7);
        assert_eq!(profile.ram_gb(), 16);
    }
}
