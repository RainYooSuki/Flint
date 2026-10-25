//! Provider-neutral loader catalog and installation contract.

use crate::{Provider, ProviderCapability, ProviderDescriptor, ProviderRequirement};
use flint_core::minecraft::ScopedWorkspace;
use flint_core::ports::{
    Cancellation, Deadline, JavaRunner, LoggerPort, NetworkPort, PathPort, ProgressSink,
};
use std::{
    path::Path,
    time::Duration,
};

pub const LOADER_INSTALLATION_CAPABILITY_ID: &str = "loaders.installation";
pub const LOADER_CAPABILITY_API: crate::ApiVersion = crate::ApiVersion::new(1, 0);
pub const LOADER_PROVIDER_API: crate::ApiVersion = crate::ApiVersion::new(1, 0);
pub const LOADER_PROVIDER_VERSION: &str = "1.0.0";
pub const LOADER_PROVIDER_VERSION_REQUIREMENT: &str = "^1.0.0";
pub const LOADER_METADATA_TIMEOUT: Duration = Duration::from_secs(75);
pub const LOADER_INSTALLER_HTTP_TIMEOUT: Duration = Duration::from_secs(600);
pub const LOADER_JAVA_PROCESS_TIMEOUT: Duration = Duration::from_secs(600);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoaderTimeouts {
    pub metadata: Duration,
    pub installer_http: Duration,
    pub java_process: Duration,
}

impl LoaderTimeouts {
    pub const fn new(metadata: Duration, installer_http: Duration, java_process: Duration) -> Self {
        Self {
            metadata,
            installer_http,
            java_process,
        }
    }
}

pub const DEFAULT_LOADER_TIMEOUTS: LoaderTimeouts = LoaderTimeouts::new(
    LOADER_METADATA_TIMEOUT,
    LOADER_INSTALLER_HTTP_TIMEOUT,
    LOADER_JAVA_PROCESS_TIMEOUT,
);

impl Default for LoaderTimeouts {
    fn default() -> Self {
        DEFAULT_LOADER_TIMEOUTS
    }
}

macro_rules! loader_contract {
    ($provider_id:ident, $selection_key:ident, $selection_keys:ident, $capabilities:ident, $id:literal, $key:literal) => {
        pub const $provider_id: &str = $id;
        pub const $selection_key: &str = $key;
        pub const $selection_keys: &[&str] = &[$selection_key];
        pub const $capabilities: &[ProviderCapability] = &[ProviderCapability {
            id: LOADER_INSTALLATION_CAPABILITY_ID,
            api_version: LOADER_CAPABILITY_API,
            selection_keys: $selection_keys,
        }];
    };
}

loader_contract!(
    FABRIC_PROVIDER_ID,
    FABRIC_SELECTION_KEY,
    FABRIC_SELECTION_KEYS,
    FABRIC_CAPABILITIES,
    "fabric",
    "loader-kind:fabric"
);
loader_contract!(
    FORGE_PROVIDER_ID,
    FORGE_SELECTION_KEY,
    FORGE_SELECTION_KEYS,
    FORGE_CAPABILITIES,
    "forge",
    "loader-kind:forge"
);
loader_contract!(
    LITELOADER_PROVIDER_ID,
    LITELOADER_SELECTION_KEY,
    LITELOADER_SELECTION_KEYS,
    LITELOADER_CAPABILITIES,
    "liteloader",
    "loader-kind:liteloader"
);
loader_contract!(
    NEOFORGE_PROVIDER_ID,
    NEOFORGE_SELECTION_KEY,
    NEOFORGE_SELECTION_KEYS,
    NEOFORGE_CAPABILITIES,
    "neoforge",
    "loader-kind:neoforge"
);
loader_contract!(
    OPTIFINE_PROVIDER_ID,
    OPTIFINE_SELECTION_KEY,
    OPTIFINE_SELECTION_KEYS,
    OPTIFINE_CAPABILITIES,
    "optifine",
    "loader-kind:optifine"
);
loader_contract!(
    QUILT_PROVIDER_ID,
    QUILT_SELECTION_KEY,
    QUILT_SELECTION_KEYS,
    QUILT_CAPABILITIES,
    "quilt",
    "loader-kind:quilt"
);

pub const LOADER_PROVIDER_IDS: &[&str] = &[
    FABRIC_PROVIDER_ID,
    FORGE_PROVIDER_ID,
    LITELOADER_PROVIDER_ID,
    NEOFORGE_PROVIDER_ID,
    OPTIFINE_PROVIDER_ID,
    QUILT_PROVIDER_ID,
];

pub const LOADER_SELECTION_KEYS: &[&str] = &[
    FABRIC_SELECTION_KEY,
    FORGE_SELECTION_KEY,
    LITELOADER_SELECTION_KEY,
    NEOFORGE_SELECTION_KEY,
    OPTIFINE_SELECTION_KEY,
    QUILT_SELECTION_KEY,
];

pub const fn loader_provider_requirement(selection_key: Option<&'static str>) -> ProviderRequirement {
    ProviderRequirement {
        capability: LOADER_INSTALLATION_CAPABILITY_ID,
        api_version: LOADER_CAPABILITY_API,
        provider_version_req: LOADER_PROVIDER_VERSION_REQUIREMENT,
        selection_key,
    }
}

pub const fn loader_descriptor_for(
    provider_id: &'static str,
    capabilities: &'static [ProviderCapability],
) -> ProviderDescriptor {
    ProviderDescriptor {
        id: provider_id,
        provider_version: LOADER_PROVIDER_VERSION,
        provider_api: LOADER_PROVIDER_API,
        capabilities,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoaderVersionData {
    pub version: String,
    pub label: String,
    pub recommended: bool,
    pub game_version: String,
    pub release_time: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoaderInstallOutputData {
    pub installed_version_id: String,
    pub log_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoaderListRequest {
    pub game_version: String,
}

impl LoaderListRequest {
    pub fn new(game_version: impl Into<String>) -> Self {
        Self {
            game_version: game_version.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoaderInstallRequest {
    pub game_version: String,
    pub loader_version: String,
    pub java_path: String,
}

impl LoaderInstallRequest {
    pub fn new(
        game_version: impl Into<String>,
        loader_version: impl Into<String>,
        java_path: impl Into<String>,
    ) -> Self {
        Self {
            game_version: game_version.into(),
            loader_version: loader_version.into(),
            java_path: java_path.into(),
        }
    }
}

pub struct LoaderListContext<'a> {
    network: &'a dyn NetworkPort,
    workspace: &'a dyn PathPort,
    progress: &'a dyn ProgressSink,
    logger: &'a dyn LoggerPort,
    timeouts: LoaderTimeouts,
    deadline: Deadline,
    cancellation: Cancellation,
}

impl<'a> LoaderListContext<'a> {
    pub fn new(
        network: &'a dyn NetworkPort,
        workspace: &'a dyn PathPort,
        progress: &'a dyn ProgressSink,
        logger: &'a dyn LoggerPort,
        timeouts: LoaderTimeouts,
        deadline: Deadline,
        cancellation: Cancellation,
    ) -> Self {
        Self {
            network,
            workspace,
            progress,
            logger,
            timeouts,
            deadline,
            cancellation,
        }
    }

    pub fn network(&self) -> &'a dyn NetworkPort {
        self.network
    }

    /// Safe workspace port. `SafeWorkspace` is the production adapter.
    pub fn workspace(&self) -> &'a dyn PathPort {
        self.workspace
    }

    /// Explicit alias for the safe-workspace contract.
    pub fn safe_workspace(&self) -> &'a dyn PathPort {
        self.workspace
    }

    pub fn progress(&self) -> &'a dyn ProgressSink {
        self.progress
    }

    pub fn logger(&self) -> &'a dyn LoggerPort {
        self.logger
    }

    pub fn timeouts(&self) -> LoaderTimeouts {
        self.timeouts
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

pub struct LoaderInstallContext<'a> {
    metadata_network: &'a dyn NetworkPort,
    installer_network: &'a dyn NetworkPort,
    workspace: &'a dyn PathPort,
    java: &'a dyn JavaRunner,
    root: &'a Path,
    progress: &'a dyn ProgressSink,
    logger: &'a dyn LoggerPort,
    timeouts: LoaderTimeouts,
    deadline: Deadline,
    cancellation: Cancellation,
}

impl<'a> LoaderInstallContext<'a> {
    pub fn new(
        metadata_network: &'a dyn NetworkPort,
        installer_network: &'a dyn NetworkPort,
        workspace: &'a dyn PathPort,
        java: &'a dyn JavaRunner,
        root: &'a Path,
        progress: &'a dyn ProgressSink,
        logger: &'a dyn LoggerPort,
        timeouts: LoaderTimeouts,
        deadline: Deadline,
        cancellation: Cancellation,
    ) -> Self {
        Self {
            metadata_network,
            installer_network,
            workspace,
            java,
            root,
            progress,
            logger,
            timeouts,
            deadline,
            cancellation,
        }
    }

    pub fn network(&self) -> &'a dyn NetworkPort {
        self.metadata_network
    }

    pub fn metadata_network(&self) -> &'a dyn NetworkPort {
        self.metadata_network
    }

    pub fn installer_network(&self) -> &'a dyn NetworkPort {
        self.installer_network
    }

    /// Safe workspace port. `SafeWorkspace` is the production adapter.
    pub fn workspace(&self) -> &'a dyn PathPort {
        self.workspace
    }

    /// Explicit alias for the safe-workspace contract.
    pub fn safe_workspace(&self) -> &'a dyn PathPort {
        self.workspace
    }

    pub fn java(&self) -> &'a dyn JavaRunner {
        self.java
    }

    pub fn root(&self) -> &'a Path {
        self.root
    }

    pub fn scoped_workspace(&self) -> ScopedWorkspace<'a> {
        ScopedWorkspace::new(self.root, self.workspace)
    }

    pub fn progress(&self) -> &'a dyn ProgressSink {
        self.progress
    }

    pub fn logger(&self) -> &'a dyn LoggerPort {
        self.logger
    }

    pub fn timeouts(&self) -> LoaderTimeouts {
        self.timeouts
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

/// Object-safe, provider-neutral loader implementation contract.
pub trait LoaderProvider: Provider + Send + Sync {
    fn list_versions(
        &self,
        context: &LoaderListContext<'_>,
        request: &LoaderListRequest,
    ) -> Result<Vec<LoaderVersionData>, String>;

    fn install(
        &self,
        context: &LoaderInstallContext<'_>,
        request: &LoaderInstallRequest,
    ) -> Result<LoaderInstallOutputData, String>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{FakeJavaRunner, FakeNetworkPort, FakePathPort};
    use flint_core::ports::{
        JavaRunner, NetworkPort, NoopLogger, NoopProgress, PathPort,
    };

    #[test]
    fn loader_provider_contract_is_object_safe_and_timeouts_are_explicit() {
        fn assert_send_sync<T: ?Sized + Send + Sync>() {}
        assert_send_sync::<dyn LoaderProvider>();
        assert_eq!(LOADER_INSTALLATION_CAPABILITY_ID, "loaders.installation");
        assert_eq!(LOADER_CAPABILITY_API, crate::ApiVersion::new(1, 0));
        assert_eq!(LOADER_PROVIDER_API, crate::ApiVersion::new(1, 0));
        assert_eq!(LOADER_PROVIDER_VERSION, "1.0.0");
        assert_eq!(LOADER_PROVIDER_VERSION_REQUIREMENT, "^1.0.0");
        assert_eq!(LOADER_SELECTION_KEYS.len(), 6);
        let requirement = loader_provider_requirement(Some(FABRIC_SELECTION_KEY));
        assert_eq!(requirement.capability, LOADER_INSTALLATION_CAPABILITY_ID);
        assert_eq!(requirement.selection_key, Some(FABRIC_SELECTION_KEY));
        let timeouts = LoaderTimeouts::default();
        assert_eq!(timeouts.metadata, Duration::from_secs(75));
        assert_eq!(timeouts.installer_http, Duration::from_secs(600));
        assert_eq!(timeouts.java_process, Duration::from_secs(600));
    }

    #[test]
    fn stable_loader_ids_and_descriptors_cover_all_six_leaves() {
        assert_eq!(LOADER_PROVIDER_IDS.len(), 6);
        let expected = [
            (FABRIC_PROVIDER_ID, FABRIC_SELECTION_KEY, FABRIC_CAPABILITIES),
            (FORGE_PROVIDER_ID, FORGE_SELECTION_KEY, FORGE_CAPABILITIES),
            (LITELOADER_PROVIDER_ID, LITELOADER_SELECTION_KEY, LITELOADER_CAPABILITIES),
            (NEOFORGE_PROVIDER_ID, NEOFORGE_SELECTION_KEY, NEOFORGE_CAPABILITIES),
            (OPTIFINE_PROVIDER_ID, OPTIFINE_SELECTION_KEY, OPTIFINE_CAPABILITIES),
            (QUILT_PROVIDER_ID, QUILT_SELECTION_KEY, QUILT_CAPABILITIES),
        ];
        for (id, selection_key, capabilities) in expected {
            let descriptor = loader_descriptor_for(id, capabilities);
            assert_eq!(descriptor.id, id);
            assert_eq!(descriptor.provider_api, LOADER_PROVIDER_API);
            assert_eq!(descriptor.provider_version, LOADER_PROVIDER_VERSION);
            assert_eq!(descriptor.capabilities[0].id, LOADER_INSTALLATION_CAPABILITY_ID);
            assert_eq!(descriptor.capabilities[0].selection_keys[0], selection_key);
        }
    }

    #[test]
    fn loader_contexts_carry_typed_ports_and_timeout_values() {
        let network = FakeNetworkPort::new();
        let installer = FakeNetworkPort::new();
        let workspace = FakePathPort::new();
        let java = FakeJavaRunner::new();
        let progress = NoopProgress;
        let logger = NoopLogger;
        let timeouts = LoaderTimeouts::default();
        let list = LoaderListContext::new(
            &network,
            &workspace,
            &progress,
            &logger,
            timeouts,
            Deadline::after(Duration::from_secs(120)),
            Cancellation::new(),
        );
        assert_eq!(list.timeouts(), timeouts);
        assert_eq!(
            list.workspace()
                .safe_join(Path::new("root"), "file.bin")
                .unwrap(),
            Path::new("root").join("file.bin")
        );
        let install = LoaderInstallContext::new(
            &network,
            &installer,
            &workspace,
            &java,
            Path::new("game"),
            &progress,
            &logger,
            timeouts,
            Deadline::after(Duration::from_secs(120)),
            Cancellation::new(),
        );
        assert_eq!(install.timeouts(), timeouts);
        assert_eq!(install.root(), Path::new("game"));
        assert_eq!(install.scoped_workspace().root(), Path::new("game"));
        let _installer_network: &dyn NetworkPort = install.installer_network();
        let _java: &dyn JavaRunner = install.java();
    }
}