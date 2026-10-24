use std::{error::Error, fmt};

use flint_core::contract::{ApiVersion, ProviderRequirement};
use semver::{Version, VersionReq};

use crate::{ContributionId, FeatureId};

/// Whether a feature is required for a usable host or is optional under degradation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartupPolicy {
    Required,
    Optional,
}

impl StartupPolicy {
    pub const fn is_required(self) -> bool {
        matches!(self, Self::Required)
    }
}

/// A backend contribution category.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ContributionKind {
    Service,
    Event,
    Capability,
}

/// A declared backend contribution owned by one feature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContributionDescriptor {
    pub id: ContributionId,
    pub kind: ContributionKind,
}

impl ContributionDescriptor {
    pub fn new(id: ContributionId, kind: ContributionKind) -> Self {
        Self { id, kind }
    }
}

/// A feature dependency with a semantic-version requirement on the dependency implementation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeatureRequirement {
    pub id: FeatureId,
    pub version_req: VersionReq,
}

impl FeatureRequirement {
    pub fn new(id: FeatureId, version_req: VersionReq) -> Self {
        Self { id, version_req }
    }
}

/// Cloneable, data-only feature metadata. Initialization is registered separately.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeatureDescriptor {
    pub id: FeatureId,
    pub feature_version: Version,
    pub api_version: ApiVersion,
    pub startup_policy: StartupPolicy,
    pub requires: Vec<FeatureRequirement>,
    pub provider_requirements: Vec<ProviderRequirement>,
    pub contributions: Vec<ContributionDescriptor>,
}

impl FeatureDescriptor {
    pub fn is_api_compatible_with(&self, host_api: ApiVersion) -> bool {
        self.api_version.is_compatible_with(host_api)
    }
}

/// Framework-neutral context passed once to a feature initializer.
///
/// Concrete initializers own their explicit typed dependencies via constructor capture. The context
/// carries feature identity and host API version only; it is not a service locator.
#[derive(Debug, Clone)]
pub struct FeatureInitContext {
    feature_id: FeatureId,
    host_api: ApiVersion,
}

impl FeatureInitContext {
    pub(crate) fn new(feature_id: FeatureId, host_api: ApiVersion) -> Self {
        Self {
            feature_id,
            host_api,
        }
    }

    pub fn feature_id(&self) -> &FeatureId {
        &self.feature_id
    }

    pub fn host_api(&self) -> ApiVersion {
        self.host_api
    }
}

/// Read-only runtime health reported by an initialized feature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FeatureHealth {
    Healthy,
    Unhealthy { reason: String },
}

/// Runtime handle returned by a successful feature initialization.
pub trait FeatureRuntime: Send {
    /// Read-only runtime diagnostic; it does not mutate coordinator state.
    fn health(&self) -> FeatureHealth;
    /// Stop owned work and release owned resources. Shutdown errors are retained as diagnostics.
    fn shutdown(&mut self) -> Result<(), String>;
}

/// A feature initialization failure. The coordinator sanitizes public availability reasons.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeatureInitError {
    reason: String,
}

impl FeatureInitError {
    pub fn new(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
        }
    }

    pub fn reason(&self) -> &str {
        &self.reason
    }
}

impl From<String> for FeatureInitError {
    fn from(reason: String) -> Self {
        Self::new(reason)
    }
}

impl<'a> From<&'a str> for FeatureInitError {
    fn from(reason: &'a str) -> Self {
        Self::new(reason)
    }
}

impl fmt::Display for FeatureInitError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.reason)
    }
}

impl Error for FeatureInitError {}

/// Explicit initialization entry point kept separate from the data-only descriptor.
///
/// The coordinator invokes this at most once. If initialization starts workers or acquires
/// partial resources and then fails, the initializer must release those resources before
/// returning the error; the coordinator has no runtime handle to clean up for that feature.
pub trait FeatureInitializer: Send + Sync {
    fn initialize(
        &self,
        context: &FeatureInitContext,
    ) -> Result<Box<dyn FeatureRuntime>, FeatureInitError>;
}

/// Convenience wrapper for initializers that do not require a dedicated type.
pub struct FnFeatureInitializer<F>(F);

impl<F> FnFeatureInitializer<F> {
    pub fn new(function: F) -> Self {
        Self(function)
    }
}

impl<F> FeatureInitializer for FnFeatureInitializer<F>
where
    F: Fn(&FeatureInitContext) -> Result<Box<dyn FeatureRuntime>, FeatureInitError>
        + Send
        + Sync,
{
    fn initialize(
        &self,
        context: &FeatureInitContext,
    ) -> Result<Box<dyn FeatureRuntime>, FeatureInitError> {
        (self.0)(context)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn descriptor(api_version: ApiVersion) -> FeatureDescriptor {
        FeatureDescriptor {
            id: FeatureId::new("flint.example").unwrap(),
            feature_version: Version::new(1, 0, 0),
            api_version,
            startup_policy: StartupPolicy::Required,
            requires: Vec::new(),
            provider_requirements: Vec::new(),
            contributions: Vec::new(),
        }
    }

    #[test]
    fn feature_api_compatibility_requires_matching_major_and_older_or_equal_minor() {
        assert!(descriptor(ApiVersion::new(1, 0)).is_api_compatible_with(ApiVersion::new(1, 2)));
        assert!(descriptor(ApiVersion::new(1, 2)).is_api_compatible_with(ApiVersion::new(1, 2)));
        assert!(!descriptor(ApiVersion::new(1, 3)).is_api_compatible_with(ApiVersion::new(1, 2)));
        assert!(!descriptor(ApiVersion::new(2, 0)).is_api_compatible_with(ApiVersion::new(1, 9)));
    }

    #[test]
    fn initialization_errors_preserve_private_reason_text() {
        let error = FeatureInitError::from("worker startup failed");
        assert_eq!(error.reason(), "worker startup failed");
        assert_eq!(error.to_string(), "worker startup failed");
    }
}