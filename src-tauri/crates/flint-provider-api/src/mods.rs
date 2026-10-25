//! Provider-neutral typed mod catalog contract.

use flint_core::ports::{Cancellation, Deadline, LoggerPort, NetworkPort, ProgressSink};
use std::{fmt, time::Duration};

use crate::{
    ApiVersion, Provider, ProviderCapability, ProviderDescriptor, ProviderRequirement,
};

pub const MOD_CATALOG_CAPABILITY_ID: &str = "mods.catalog";
pub const MOD_CATALOG_API: ApiVersion = ApiVersion::new(1, 0);
pub const MOD_PROVIDER_API: ApiVersion = ApiVersion::new(1, 0);
pub const MOD_PROVIDER_VERSION: &str = "1.0.0";
pub const MOD_PROVIDER_VERSION_REQUIREMENT: &str = "^1.0.0";

pub const CURSEFORGE_PROVIDER_ID: &str = "curseforge";
pub const CURSEFORGE_SELECTION_KEY: &str = "mod-source:curseforge";
const CURSEFORGE_SELECTION_KEYS: &[&str] = &[CURSEFORGE_SELECTION_KEY];
pub const CURSEFORGE_CAPABILITIES: &[ProviderCapability] = &[ProviderCapability {
    id: MOD_CATALOG_CAPABILITY_ID,
    api_version: MOD_CATALOG_API,
    selection_keys: CURSEFORGE_SELECTION_KEYS,
}];

pub const MODRINTH_PROVIDER_ID: &str = "modrinth";
pub const MODRINTH_SELECTION_KEY: &str = "mod-source:modrinth";
const MODRINTH_SELECTION_KEYS: &[&str] = &[MODRINTH_SELECTION_KEY];
pub const MODRINTH_CAPABILITIES: &[ProviderCapability] = &[ProviderCapability {
    id: MOD_CATALOG_CAPABILITY_ID,
    api_version: MOD_CATALOG_API,
    selection_keys: MODRINTH_SELECTION_KEYS,
}];

pub const fn mod_provider_requirement(selection_key: Option<&'static str>) -> ProviderRequirement {
    ProviderRequirement {
        capability: MOD_CATALOG_CAPABILITY_ID,
        api_version: MOD_CATALOG_API,
        provider_version_req: MOD_PROVIDER_VERSION_REQUIREMENT,
        selection_key,
    }
}

pub fn validate_mod_provider_id(value: &str) -> Result<(), String> {
    let value = value.trim();
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err("The provider project or version id is invalid.".to_string());
    }
    Ok(())
}

#[derive(Clone, PartialEq, Eq)]
pub struct ScopedCredential {
    value: String,
}

impl ScopedCredential {
    pub fn new(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
        }
    }

    pub fn expose(&self) -> &str {
        &self.value
    }
}

impl fmt::Debug for ScopedCredential {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ScopedCredential([redacted])")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModDependencyKind {
    Required,
    Optional,
    Embedded,
    Incompatible,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModDependencyData {
    pub project_id: String,
    pub version_id: Option<String>,
    pub name: String,
    pub dependency_type: ModDependencyKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModVersionData {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub version_number: String,
    pub file_name: String,
    pub download_url: Option<String>,
    pub game_versions: Vec<String>,
    pub loaders: Vec<String>,
    pub release_type: String,
    pub published_at: Option<String>,
    pub dependencies: Vec<ModDependencyData>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModProjectData {
    pub id: String,
    pub slug: String,
    pub name: String,
    pub summary: String,
    pub author: String,
    pub downloads: u64,
    pub icon_url: Option<String>,
    pub categories: Vec<String>,
    pub project_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModSearchResultData {
    pub total: u64,
    pub offset: u32,
    pub limit: u32,
    pub hits: Vec<ModProjectData>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModSearchRequest {
    pub query: String,
    pub game_version: String,
    pub loader: String,
    pub offset: u32,
    pub limit: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModListVersionsRequest {
    pub project_id: String,
    pub game_version: String,
    pub loader: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModVersionRequest {
    pub project_id: String,
    pub version_id: String,
    pub game_version: String,
    pub loader: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModDependencyVersionRequest {
    pub dependency: ModDependencyData,
    pub game_version: String,
    pub loader: String,
}

pub struct ModProviderContext<'a> {
    network: &'a dyn NetworkPort,
    credential: Option<ScopedCredential>,
    progress: &'a dyn ProgressSink,
    logger: &'a dyn LoggerPort,
    timeout: Duration,
    deadline: Deadline,
    cancellation: Cancellation,
}

impl<'a> ModProviderContext<'a> {
    /// Creates an operation-scoped context. `timeout` is the per-request host operation timeout.
    pub fn new(
        network: &'a dyn NetworkPort,
        credential: Option<ScopedCredential>,
        progress: &'a dyn ProgressSink,
        logger: &'a dyn LoggerPort,
        timeout: Duration,
        deadline: Deadline,
        cancellation: Cancellation,
    ) -> Self {
        Self {
            network,
            credential,
            progress,
            logger,
            timeout,
            deadline,
            cancellation,
        }
    }

    pub fn network(&self) -> &'a dyn NetworkPort {
        self.network
    }

    pub fn credential(&self) -> Option<&str> {
        self.credential.as_ref().map(ScopedCredential::expose)
    }

    pub fn progress(&self) -> &'a dyn ProgressSink {
        self.progress
    }

    pub fn logger(&self) -> &'a dyn LoggerPort {
        self.logger
    }

    /// Operation timeout supplied by the host for provider HTTP requests.
    pub fn timeout(&self) -> Duration {
        self.timeout
    }

    pub fn deadline(&self) -> &Deadline {
        &self.deadline
    }

    pub fn cancellation(&self) -> &Cancellation {
        &self.cancellation
    }

    pub fn check_control(&self) -> Result<(), String> {
        self.deadline.check()?;
        self.cancellation.check()
    }
}

/// Object-safe source-neutral mod provider contract.
pub trait ModProvider: Provider + Send + Sync {
    fn search(
        &self,
        context: &ModProviderContext<'_>,
        request: &ModSearchRequest,
    ) -> Result<ModSearchResultData, String>;

    fn list_versions(
        &self,
        context: &ModProviderContext<'_>,
        request: &ModListVersionsRequest,
    ) -> Result<Vec<ModVersionData>, String>;

    fn version_by_id(
        &self,
        context: &ModProviderContext<'_>,
        request: &ModVersionRequest,
    ) -> Result<ModVersionData, String>;

    fn dependency_version(
        &self,
        context: &ModProviderContext<'_>,
        request: &ModDependencyVersionRequest,
    ) -> Result<Option<ModVersionData>, String>;
}

pub const fn descriptor_for(
    provider_id: &'static str,
    capabilities: &'static [ProviderCapability],
) -> ProviderDescriptor {
    ProviderDescriptor {
        id: provider_id,
        provider_version: MOD_PROVIDER_VERSION,
        provider_api: MOD_PROVIDER_API,
        capabilities,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mod_provider_contract_is_object_safe_and_send_sync() {
        fn assert_send_sync<T: ?Sized + Send + Sync>() {}
        assert_send_sync::<dyn ModProvider>();
    }

    #[test]
    fn scoped_credentials_are_redacted_in_debug_output() {
        let credential = ScopedCredential::new("top-secret");
        assert_eq!(credential.expose(), "top-secret");
        assert!(!format!("{credential:?}").contains("top-secret"));
    }

    #[test]
    fn neutral_mod_constants_form_the_expected_requirement() {
        assert_eq!(MOD_CATALOG_CAPABILITY_ID, "mods.catalog");
        assert_eq!(MOD_CATALOG_API, ApiVersion::new(1, 0));
        assert_eq!(MOD_PROVIDER_API, ApiVersion::new(1, 0));
        assert_eq!(MOD_PROVIDER_VERSION, "1.0.0");
        assert_eq!(MOD_PROVIDER_VERSION_REQUIREMENT, "^1.0.0");
        assert_eq!(CURSEFORGE_PROVIDER_ID, "curseforge");
        assert_eq!(CURSEFORGE_SELECTION_KEY, "mod-source:curseforge");
        assert_eq!(MODRINTH_PROVIDER_ID, "modrinth");
        assert_eq!(MODRINTH_SELECTION_KEY, "mod-source:modrinth");
        let requirement = mod_provider_requirement(Some(MODRINTH_SELECTION_KEY));
        assert_eq!(requirement.capability, MOD_CATALOG_CAPABILITY_ID);
        assert_eq!(requirement.api_version, MOD_CATALOG_API);
        assert_eq!(
            requirement.provider_version_req,
            MOD_PROVIDER_VERSION_REQUIREMENT
        );
        assert_eq!(requirement.selection_key, Some(MODRINTH_SELECTION_KEY));
    }
}