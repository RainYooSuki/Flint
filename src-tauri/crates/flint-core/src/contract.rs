use std::fmt;

/// A version of a provider-facing or capability-facing API.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ApiVersion {
    pub major: u16,
    pub minor: u16,
}

impl ApiVersion {
    pub const fn new(major: u16, minor: u16) -> Self {
        Self { major, minor }
    }

    /// Returns true when this version (a provider or a requirement) can run
    /// against `host_or_capability`. The major versions must match and this
    /// minor version must not be newer than the host/capability minor.
    pub const fn is_compatible_with(self, host_or_capability: ApiVersion) -> bool {
        self.major == host_or_capability.major && self.minor <= host_or_capability.minor
    }

    /// Inverse compatibility check for callers that read more naturally as
    /// host/capability first.
    pub const fn supports(self, provider_or_requirement: ApiVersion) -> bool {
        provider_or_requirement.is_compatible_with(self)
    }
}

impl fmt::Display for ApiVersion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}.{}", self.major, self.minor)
    }
}

/// A complete feature requirement resolved against the provider registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderRequirement {
    pub capability: &'static str,
    pub api_version: ApiVersion,
    pub provider_version_req: &'static str,
    pub selection_key: Option<&'static str>,
}
