#![forbid(unsafe_code)]

//! Framework-neutral Rust feature lifecycle contracts, registry, and coordinator.

mod descriptor;
mod id;
mod lifecycle;
mod registry;
mod snapshot;
mod state;

pub use descriptor::{
    ContributionDescriptor, ContributionKind, FeatureDescriptor, FeatureHealth,
    FeatureInitContext, FeatureInitError, FeatureInitializer, FeatureRequirement, FeatureRuntime,
    FnFeatureInitializer, StartupPolicy,
};
pub use flint_core::contract::{ApiVersion, ProviderRequirement};
pub use semver::{Version, VersionReq};
pub use id::{ContributionId, ContributionIdError, FeatureId, StableIdError};
pub use lifecycle::{
    CoreCleanup, FailureImpact, FeatureUnavailable, LifecycleCoordinator, LifecycleError,
    ProviderAvailability, ProviderCatalog, StartupOutcome,
};
pub use registry::{FeatureRegistration, FeatureRegistry, FeatureRegistryError};
pub use snapshot::{FeatureAvailability, FeatureAvailabilitySnapshot};
pub use state::{FeatureState, HostState};