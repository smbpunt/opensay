use crate::domain::{DomainError, HardwareProfile, ModelCatalog, ModelRecommendation};

/// Port for hardware detection operations.
///
/// Implementations detect system hardware capabilities and provide
/// recommendations for model selection.
pub trait HardwareDetector: Send + Sync {
    /// Detect the current system's hardware profile.
    ///
    /// This may be cached after the first call.
    fn detect(&self) -> Result<HardwareProfile, DomainError>;

    /// Get a model recommendation based on the hardware profile.
    fn recommend_model(&self, catalog: &ModelCatalog) -> Result<ModelRecommendation, DomainError>;

    /// Get the cached hardware profile.
    ///
    /// Returns the result of the last `detect()` call, or detects if not yet called.
    fn profile(&self) -> Result<&HardwareProfile, DomainError>;
}
