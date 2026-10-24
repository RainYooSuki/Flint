use super::{
    install_forge, install_liteloader, install_meta_profile, install_neoforge, install_optifine,
    list_fabric, list_forge, list_liteloader, list_neoforge, list_optifine, list_quilt,
    FABRIC_LOADER_LIST, QUILT_LOADER_LIST,
};
use crate::{
    models::{LoaderInstallRequest, LoaderKind, LoaderVersionInfo},
    provider_api::{
        ApiVersion, Provider, ProviderCapability, ProviderDescriptor, ProviderRequirement,
        ProviderRegistry,
    },
};
use reqwest::blocking::Client;
use std::path::Path;

const LOADER_INSTALLATION_CAPABILITY_ID: &str = "loaders.installation";
const LOADER_CAPABILITY_API: ApiVersion = ApiVersion::new(1, 0);
const LOADER_PROVIDER_API: ApiVersion = ApiVersion::new(1, 0);
const LOADER_PROVIDER_VERSION: &str = "1.0.0";
const LOADER_PROVIDER_VERSION_REQUIREMENT: &str = "^1.0.0";

const FABRIC_PROVIDER_ID: &str = "fabric";
const FABRIC_SELECTION_KEY: &str = "loader-kind:fabric";
const FABRIC_SELECTION_KEYS: &[&str] = &[FABRIC_SELECTION_KEY];
const FABRIC_CAPABILITIES: &[ProviderCapability] = &[ProviderCapability {
    id: LOADER_INSTALLATION_CAPABILITY_ID,
    api_version: LOADER_CAPABILITY_API,
    selection_keys: FABRIC_SELECTION_KEYS,
}];

const FORGE_PROVIDER_ID: &str = "forge";
const FORGE_SELECTION_KEY: &str = "loader-kind:forge";
const FORGE_SELECTION_KEYS: &[&str] = &[FORGE_SELECTION_KEY];
const FORGE_CAPABILITIES: &[ProviderCapability] = &[ProviderCapability {
    id: LOADER_INSTALLATION_CAPABILITY_ID,
    api_version: LOADER_CAPABILITY_API,
    selection_keys: FORGE_SELECTION_KEYS,
}];

const LITELOADER_PROVIDER_ID: &str = "liteloader";
const LITELOADER_SELECTION_KEY: &str = "loader-kind:liteloader";
const LITELOADER_SELECTION_KEYS: &[&str] = &[LITELOADER_SELECTION_KEY];
const LITELOADER_CAPABILITIES: &[ProviderCapability] = &[ProviderCapability {
    id: LOADER_INSTALLATION_CAPABILITY_ID,
    api_version: LOADER_CAPABILITY_API,
    selection_keys: LITELOADER_SELECTION_KEYS,
}];

const NEOFORGE_PROVIDER_ID: &str = "neoforge";
const NEOFORGE_SELECTION_KEY: &str = "loader-kind:neoforge";
const NEOFORGE_SELECTION_KEYS: &[&str] = &[NEOFORGE_SELECTION_KEY];
const NEOFORGE_CAPABILITIES: &[ProviderCapability] = &[ProviderCapability {
    id: LOADER_INSTALLATION_CAPABILITY_ID,
    api_version: LOADER_CAPABILITY_API,
    selection_keys: NEOFORGE_SELECTION_KEYS,
}];

const OPTIFINE_PROVIDER_ID: &str = "optifine";
const OPTIFINE_SELECTION_KEY: &str = "loader-kind:optifine";
const OPTIFINE_SELECTION_KEYS: &[&str] = &[OPTIFINE_SELECTION_KEY];
const OPTIFINE_CAPABILITIES: &[ProviderCapability] = &[ProviderCapability {
    id: LOADER_INSTALLATION_CAPABILITY_ID,
    api_version: LOADER_CAPABILITY_API,
    selection_keys: OPTIFINE_SELECTION_KEYS,
}];

const QUILT_PROVIDER_ID: &str = "quilt";
const QUILT_SELECTION_KEY: &str = "loader-kind:quilt";
const QUILT_SELECTION_KEYS: &[&str] = &[QUILT_SELECTION_KEY];
const QUILT_CAPABILITIES: &[ProviderCapability] = &[ProviderCapability {
    id: LOADER_INSTALLATION_CAPABILITY_ID,
    api_version: LOADER_CAPABILITY_API,
    selection_keys: QUILT_SELECTION_KEYS,
}];

/// Best-effort provider progress publication. The Tauri implementation lives
/// in the feature host layer; provider code only sees this typed sink.
pub(super) trait LoaderProgressSink {
    fn progress(&self, stage: &str, completed: u64, total: u64, message: &str);
}

/// Operation-scoped list context. Listing carries only the metadata HTTP
/// client, the requested game version, and a progress sink; installation-only
/// data is deliberately absent.
pub(super) struct LoaderListContext<'a> {
    metadata_client: &'a Client,
    game_version: &'a str,
    progress: &'a dyn LoaderProgressSink,
}

impl<'a> LoaderListContext<'a> {
    pub(super) fn new(
        metadata_client: &'a Client,
        game_version: &'a str,
        progress: &'a dyn LoaderProgressSink,
    ) -> Self {
        Self {
            metadata_client,
            game_version,
            progress,
        }
    }

    pub(super) fn metadata_client(&self) -> &'a Client {
        self.metadata_client
    }

    pub(super) fn game_version(&self) -> &'a str {
        self.game_version
    }

    pub(super) fn emit_progress(
        &self,
        stage: &str,
        completed: u64,
        total: u64,
        message: &str,
    ) {
        self.progress.progress(stage, completed, total, message);
    }
}

/// Operation-scoped installation context. Metadata/artifact I/O and Java
/// installer download I/O use separate shared clients so the pre-provider
/// 75-second and 600-second timeout boundaries remain explicit.
pub(super) struct LoaderProviderContext<'a> {
    metadata_client: &'a Client,
    installer_client: &'a Client,
    root: &'a Path,
    request: &'a LoaderInstallRequest,
    progress: &'a dyn LoaderProgressSink,
}

impl<'a> LoaderProviderContext<'a> {
    pub(super) fn new(
        metadata_client: &'a Client,
        installer_client: &'a Client,
        root: &'a Path,
        request: &'a LoaderInstallRequest,
        progress: &'a dyn LoaderProgressSink,
    ) -> Self {
        Self {
            metadata_client,
            installer_client,
            root,
            request,
            progress,
        }
    }

    pub(super) fn metadata_client(&self) -> &'a Client {
        self.metadata_client
    }

    pub(super) fn installer_client(&self) -> &'a Client {
        self.installer_client
    }

    pub(super) fn root(&self) -> &'a Path {
        self.root
    }

    pub(super) fn request(&self) -> &'a LoaderInstallRequest {
        self.request
    }

    pub(super) fn emit_progress(
        &self,
        stage: &str,
        completed: u64,
        total: u64,
        message: &str,
    ) {
        self.progress.progress(stage, completed, total, message);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct LoaderInstallOutput {
    pub(super) installed_version_id: String,
    pub(super) log_path: Option<String>,
}

impl LoaderInstallOutput {
    fn without_log(installed_version_id: String) -> Self {
        Self {
            installed_version_id,
            log_path: None,
        }
    }

    fn with_log(installed_version_id: String, log_path: Option<String>) -> Self {
        Self {
            installed_version_id,
            log_path,
        }
    }
}

pub(super) trait LoaderProvider: Provider {
    fn list_versions(
        &self,
        context: &LoaderListContext<'_>,
    ) -> Result<Vec<LoaderVersionInfo>, String>;

    fn install(
        &self,
        context: &LoaderProviderContext<'_>,
    ) -> Result<LoaderInstallOutput, String>;
}

struct FabricLoaderProvider;
struct ForgeLoaderProvider;
struct LiteLoaderProvider;
struct NeoForgeLoaderProvider;
struct OptiFineProvider;
struct QuiltLoaderProvider;

impl Provider for FabricLoaderProvider {
    fn descriptor(&self) -> ProviderDescriptor {
        ProviderDescriptor {
            id: FABRIC_PROVIDER_ID,
            provider_version: LOADER_PROVIDER_VERSION,
            provider_api: LOADER_PROVIDER_API,
            capabilities: FABRIC_CAPABILITIES,
        }
    }
}

impl Provider for ForgeLoaderProvider {
    fn descriptor(&self) -> ProviderDescriptor {
        ProviderDescriptor {
            id: FORGE_PROVIDER_ID,
            provider_version: LOADER_PROVIDER_VERSION,
            provider_api: LOADER_PROVIDER_API,
            capabilities: FORGE_CAPABILITIES,
        }
    }
}

impl Provider for LiteLoaderProvider {
    fn descriptor(&self) -> ProviderDescriptor {
        ProviderDescriptor {
            id: LITELOADER_PROVIDER_ID,
            provider_version: LOADER_PROVIDER_VERSION,
            provider_api: LOADER_PROVIDER_API,
            capabilities: LITELOADER_CAPABILITIES,
        }
    }
}

impl Provider for NeoForgeLoaderProvider {
    fn descriptor(&self) -> ProviderDescriptor {
        ProviderDescriptor {
            id: NEOFORGE_PROVIDER_ID,
            provider_version: LOADER_PROVIDER_VERSION,
            provider_api: LOADER_PROVIDER_API,
            capabilities: NEOFORGE_CAPABILITIES,
        }
    }
}

impl Provider for OptiFineProvider {
    fn descriptor(&self) -> ProviderDescriptor {
        ProviderDescriptor {
            id: OPTIFINE_PROVIDER_ID,
            provider_version: LOADER_PROVIDER_VERSION,
            provider_api: LOADER_PROVIDER_API,
            capabilities: OPTIFINE_CAPABILITIES,
        }
    }
}

impl Provider for QuiltLoaderProvider {
    fn descriptor(&self) -> ProviderDescriptor {
        ProviderDescriptor {
            id: QUILT_PROVIDER_ID,
            provider_version: LOADER_PROVIDER_VERSION,
            provider_api: LOADER_PROVIDER_API,
            capabilities: QUILT_CAPABILITIES,
        }
    }
}

impl LoaderProvider for FabricLoaderProvider {
    fn list_versions(
        &self,
        context: &LoaderListContext<'_>,
    ) -> Result<Vec<LoaderVersionInfo>, String> {
        list_fabric(context)
    }

    fn install(
        &self,
        context: &LoaderProviderContext<'_>,
    ) -> Result<LoaderInstallOutput, String> {
        let request = context.request();
        let profile_url = format!(
            "{FABRIC_LOADER_LIST}/{}/{}/profile/json",
            request.game_version, request.loader_version
        );
        let installed_version_id = install_meta_profile(context, "Fabric", profile_url)?;
        Ok(LoaderInstallOutput::without_log(installed_version_id))
    }
}

impl LoaderProvider for ForgeLoaderProvider {
    fn list_versions(
        &self,
        context: &LoaderListContext<'_>,
    ) -> Result<Vec<LoaderVersionInfo>, String> {
        list_forge(context)
    }

    fn install(
        &self,
        context: &LoaderProviderContext<'_>,
    ) -> Result<LoaderInstallOutput, String> {
        let (installed_version_id, log_path) = install_forge(context)?;
        Ok(LoaderInstallOutput::with_log(
            installed_version_id,
            log_path,
        ))
    }
}

impl LoaderProvider for LiteLoaderProvider {
    fn list_versions(
        &self,
        context: &LoaderListContext<'_>,
    ) -> Result<Vec<LoaderVersionInfo>, String> {
        list_liteloader(context)
    }

    fn install(
        &self,
        context: &LoaderProviderContext<'_>,
    ) -> Result<LoaderInstallOutput, String> {
        let installed_version_id = install_liteloader(context)?;
        Ok(LoaderInstallOutput::without_log(installed_version_id))
    }
}

impl LoaderProvider for NeoForgeLoaderProvider {
    fn list_versions(
        &self,
        context: &LoaderListContext<'_>,
    ) -> Result<Vec<LoaderVersionInfo>, String> {
        list_neoforge(context)
    }

    fn install(
        &self,
        context: &LoaderProviderContext<'_>,
    ) -> Result<LoaderInstallOutput, String> {
        let (installed_version_id, log_path) = install_neoforge(context)?;
        Ok(LoaderInstallOutput::with_log(
            installed_version_id,
            log_path,
        ))
    }
}

impl LoaderProvider for OptiFineProvider {
    fn list_versions(
        &self,
        context: &LoaderListContext<'_>,
    ) -> Result<Vec<LoaderVersionInfo>, String> {
        list_optifine(context)
    }

    fn install(
        &self,
        context: &LoaderProviderContext<'_>,
    ) -> Result<LoaderInstallOutput, String> {
        let (installed_version_id, log_path) = install_optifine(context)?;
        Ok(LoaderInstallOutput::with_log(
            installed_version_id,
            log_path,
        ))
    }
}

impl LoaderProvider for QuiltLoaderProvider {
    fn list_versions(
        &self,
        context: &LoaderListContext<'_>,
    ) -> Result<Vec<LoaderVersionInfo>, String> {
        list_quilt(context)
    }

    fn install(
        &self,
        context: &LoaderProviderContext<'_>,
    ) -> Result<LoaderInstallOutput, String> {
        let request = context.request();
        let profile_url = format!(
            "{QUILT_LOADER_LIST}/{}/{}/profile/json",
            request.game_version, request.loader_version
        );
        let installed_version_id = install_meta_profile(context, "Quilt", profile_url)?;
        Ok(LoaderInstallOutput::without_log(installed_version_id))
    }
}

struct LoaderBinding {
    requirement: ProviderRequirement,
    provider_id: &'static str,
    display_name: &'static str,
}

fn loader_requirement(selection_key: &'static str) -> ProviderRequirement {
    ProviderRequirement {
        capability: LOADER_INSTALLATION_CAPABILITY_ID,
        api_version: LOADER_CAPABILITY_API,
        provider_version_req: LOADER_PROVIDER_VERSION_REQUIREMENT,
        selection_key: Some(selection_key),
    }
}

/// The sole serialized `LoaderKind` to provider-binding mapping.
fn loader_binding(loader: LoaderKind) -> LoaderBinding {
    match loader {
        LoaderKind::Fabric => LoaderBinding {
            requirement: loader_requirement(FABRIC_SELECTION_KEY),
            provider_id: FABRIC_PROVIDER_ID,
            display_name: "Fabric",
        },
        LoaderKind::Forge => LoaderBinding {
            requirement: loader_requirement(FORGE_SELECTION_KEY),
            provider_id: FORGE_PROVIDER_ID,
            display_name: "Forge",
        },
        LoaderKind::Liteloader => LoaderBinding {
            requirement: loader_requirement(LITELOADER_SELECTION_KEY),
            provider_id: LITELOADER_PROVIDER_ID,
            display_name: "LiteLoader",
        },
        LoaderKind::Neoforge => LoaderBinding {
            requirement: loader_requirement(NEOFORGE_SELECTION_KEY),
            provider_id: NEOFORGE_PROVIDER_ID,
            display_name: "NeoForge",
        },
        LoaderKind::Optifine => LoaderBinding {
            requirement: loader_requirement(OPTIFINE_SELECTION_KEY),
            provider_id: OPTIFINE_PROVIDER_ID,
            display_name: "OptiFine",
        },
        LoaderKind::Quilt => LoaderBinding {
            requirement: loader_requirement(QUILT_SELECTION_KEY),
            provider_id: QUILT_PROVIDER_ID,
            display_name: "Quilt",
        },
    }
}

pub(super) fn loader_display_name(loader: LoaderKind) -> &'static str {
    loader_binding(loader).display_name
}

pub(super) fn build_registry() -> Result<ProviderRegistry<dyn LoaderProvider>, String> {
    let fabric: Box<dyn LoaderProvider> = Box::new(FabricLoaderProvider);
    let forge: Box<dyn LoaderProvider> = Box::new(ForgeLoaderProvider);
    let liteloader: Box<dyn LoaderProvider> = Box::new(LiteLoaderProvider);
    let neoforge: Box<dyn LoaderProvider> = Box::new(NeoForgeLoaderProvider);
    let optifine: Box<dyn LoaderProvider> = Box::new(OptiFineProvider);
    let quilt: Box<dyn LoaderProvider> = Box::new(QuiltLoaderProvider);
    let providers = vec![fabric, forge, liteloader, neoforge, optifine, quilt];
    ProviderRegistry::new(providers, LOADER_PROVIDER_API)
        .map_err(|error| format!("Cannot initialize the loader provider registry: {error}"))
}

pub(super) fn select_for_loader<'a>(
    registry: &'a ProviderRegistry<dyn LoaderProvider>,
    loader: LoaderKind,
) -> Result<&'a dyn LoaderProvider, String> {
    let binding = loader_binding(loader);
    registry.select(&binding.requirement).map_err(|error| {
        format!(
            "Cannot select the {} loader provider: {error}",
            binding.provider_id
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider_api::{Provider as _, ProviderSelectionError};


    #[test]
    fn all_loader_kinds_map_to_one_compatible_provider() {
        let registry = build_registry().expect("loader provider registry");
        let expected = [
            (LoaderKind::Fabric, "fabric", "loader-kind:fabric", "Fabric"),
            (LoaderKind::Forge, "forge", "loader-kind:forge", "Forge"),
            (
                LoaderKind::Liteloader,
                "liteloader",
                "loader-kind:liteloader",
                "LiteLoader",
            ),
            (
                LoaderKind::Neoforge,
                "neoforge",
                "loader-kind:neoforge",
                "NeoForge",
            ),
            (
                LoaderKind::Optifine,
                "optifine",
                "loader-kind:optifine",
                "OptiFine",
            ),
            (LoaderKind::Quilt, "quilt", "loader-kind:quilt", "Quilt"),
        ];

        for (loader, provider_id, selection_key, display_name) in expected {
            let binding = loader_binding(loader);
            assert_eq!(binding.provider_id, provider_id);
            assert_eq!(binding.display_name, display_name);
            assert_eq!(binding.requirement.capability, LOADER_INSTALLATION_CAPABILITY_ID);
            assert_eq!(binding.requirement.api_version, LOADER_CAPABILITY_API);
            assert_eq!(
                binding.requirement.provider_version_req,
                LOADER_PROVIDER_VERSION_REQUIREMENT
            );
            assert_eq!(binding.requirement.selection_key, Some(selection_key));

            let selected = registry
                .select(&binding.requirement)
                .expect("exactly one compatible loader provider");
            let descriptor = selected.descriptor();
            assert_eq!(descriptor.id, provider_id);
            assert_eq!(descriptor.provider_version, LOADER_PROVIDER_VERSION);
            assert_eq!(descriptor.provider_api, LOADER_PROVIDER_API);
            assert_eq!(descriptor.capabilities.len(), 1);
            assert_eq!(
                descriptor.capabilities[0].id,
                LOADER_INSTALLATION_CAPABILITY_ID
            );
            assert_eq!(
                descriptor.capabilities[0].api_version,
                LOADER_CAPABILITY_API
            );
            assert!(descriptor.capabilities[0]
                .selection_keys
                .contains(&selection_key));
            assert_eq!(loader_display_name(loader), display_name);
        }
    }

    #[test]
    fn loader_provider_registry_is_lexicographically_ordered() {
        let registry = build_registry().expect("loader provider registry");
        assert_eq!(
            registry.provider_ids().collect::<Vec<_>>(),
            vec![
                FABRIC_PROVIDER_ID,
                FORGE_PROVIDER_ID,
                LITELOADER_PROVIDER_ID,
                NEOFORGE_PROVIDER_ID,
                OPTIFINE_PROVIDER_ID,
                QUILT_PROVIDER_ID,
            ]
        );
    }

    #[test]
    fn loader_selection_fails_closed_for_zero_or_multiple_matches() {
        let registry = build_registry().expect("loader provider registry");

        let unavailable = registry.select(&ProviderRequirement {
            capability: "loaders.missing",
            api_version: LOADER_CAPABILITY_API,
            provider_version_req: LOADER_PROVIDER_VERSION_REQUIREMENT,
            selection_key: None,
        });
        assert!(matches!(
            unavailable,
            Err(ProviderSelectionError::Unavailable { .. })
        ));

        let ambiguous = registry.select(&ProviderRequirement {
            capability: LOADER_INSTALLATION_CAPABILITY_ID,
            api_version: LOADER_CAPABILITY_API,
            provider_version_req: LOADER_PROVIDER_VERSION_REQUIREMENT,
            selection_key: None,
        });
        assert!(matches!(
            ambiguous,
            Err(ProviderSelectionError::Ambiguous { .. })
        ));
    }
}
