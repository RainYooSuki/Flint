//! Explicit host composition for provider implementations.

use flint_loader_provider_fabric::{FabricLoaderProvider, DESCRIPTOR as FABRIC_LOADER_DESCRIPTOR};
use flint_loader_provider_forge::{ForgeLoaderProvider, DESCRIPTOR as FORGE_LOADER_DESCRIPTOR};
use flint_loader_provider_liteloader::{
    LiteLoaderProvider, DESCRIPTOR as LITELOADER_LOADER_DESCRIPTOR,
};
use flint_loader_provider_neoforge::{
    NeoForgeLoaderProvider, DESCRIPTOR as NEOFORGE_LOADER_DESCRIPTOR,
};
use flint_loader_provider_optifine::{
    OptiFineProvider, DESCRIPTOR as OPTIFINE_LOADER_DESCRIPTOR,
};
use flint_loader_provider_quilt::{QuiltLoaderProvider, DESCRIPTOR as QUILT_LOADER_DESCRIPTOR};
use flint_mod_provider_curseforge::CurseForgeModProvider;
use flint_mod_provider_modrinth::ModrinthModProvider;
use flint_provider_api::{
    loader_provider_requirement, mod_provider_requirement, LoaderProvider, ModProvider,
    ProviderDescriptor, ProviderRegistry, ProviderRequirement, CURSEFORGE_SELECTION_KEY,
    FABRIC_SELECTION_KEY, FORGE_SELECTION_KEY, LITELOADER_SELECTION_KEY, LOADER_PROVIDER_API,
    MODRINTH_SELECTION_KEY, MOD_PROVIDER_API, NEOFORGE_SELECTION_KEY, OPTIFINE_SELECTION_KEY,
    QUILT_SELECTION_KEY,
};
use std::sync::Arc;

pub(crate) fn mod_provider_descriptors() -> Vec<ProviderDescriptor> {
    vec![
        flint_mod_provider_curseforge::DESCRIPTOR,
        flint_mod_provider_modrinth::DESCRIPTOR,
    ]
}

pub(crate) fn mod_provider_requirements() -> Vec<ProviderRequirement> {
    vec![
        mod_provider_requirement(Some(CURSEFORGE_SELECTION_KEY)),
        mod_provider_requirement(Some(MODRINTH_SELECTION_KEY)),
    ]
}

pub(crate) fn loader_provider_descriptors() -> Vec<ProviderDescriptor> {
    vec![
        FABRIC_LOADER_DESCRIPTOR,
        FORGE_LOADER_DESCRIPTOR,
        LITELOADER_LOADER_DESCRIPTOR,
        NEOFORGE_LOADER_DESCRIPTOR,
        OPTIFINE_LOADER_DESCRIPTOR,
        QUILT_LOADER_DESCRIPTOR,
    ]
}

pub(crate) fn loader_provider_requirements() -> Vec<ProviderRequirement> {
    vec![
        loader_provider_requirement(Some(FABRIC_SELECTION_KEY)),
        loader_provider_requirement(Some(FORGE_SELECTION_KEY)),
        loader_provider_requirement(Some(LITELOADER_SELECTION_KEY)),
        loader_provider_requirement(Some(NEOFORGE_SELECTION_KEY)),
        loader_provider_requirement(Some(OPTIFINE_SELECTION_KEY)),
        loader_provider_requirement(Some(QUILT_SELECTION_KEY)),
    ]
}

fn build_mod_provider_registry() -> Result<ProviderRegistry<dyn ModProvider>, String> {
    let providers: Vec<Box<dyn ModProvider>> = vec![
        Box::new(CurseForgeModProvider::new()),
        Box::new(ModrinthModProvider::new()),
    ];
    ProviderRegistry::new(providers, MOD_PROVIDER_API).map_err(|error| error.to_string())
}

fn build_loader_provider_registry() -> Result<ProviderRegistry<dyn LoaderProvider>, String> {
    let providers: Vec<Box<dyn LoaderProvider>> = vec![
        Box::new(FabricLoaderProvider::new()),
        Box::new(ForgeLoaderProvider::new()),
        Box::new(LiteLoaderProvider::new()),
        Box::new(NeoForgeLoaderProvider::new()),
        Box::new(OptiFineProvider::new()),
        Box::new(QuiltLoaderProvider::new()),
    ];
    ProviderRegistry::new(providers, LOADER_PROVIDER_API).map_err(|error| error.to_string())
}

pub struct ModProviderState {
    registry: Result<Arc<ProviderRegistry<dyn ModProvider>>, String>,
}

impl ModProviderState {
    pub(crate) fn new() -> Self {
        Self {
            registry: build_mod_provider_registry()
                .map(Arc::new)
                .map_err(|error| format!("Cannot initialize the mod provider registry: {error}")),
        }
    }

    pub(crate) fn registry(&self) -> Result<Arc<ProviderRegistry<dyn ModProvider>>, String> {
        match &self.registry {
            Ok(registry) => Ok(Arc::clone(registry)),
            Err(error) => Err(error.clone()),
        }
    }
}

pub struct LoaderProviderState {
    registry: Result<Arc<ProviderRegistry<dyn LoaderProvider>>, String>,
}

impl LoaderProviderState {
    pub(crate) fn new() -> Self {
        Self {
            registry: build_loader_provider_registry()
                .map(Arc::new)
                .map_err(|error| {
                    format!("Cannot initialize the loader provider registry: {error}")
                }),
        }
    }

    pub(crate) fn registry(&self) -> Result<Arc<ProviderRegistry<dyn LoaderProvider>>, String> {
        match &self.registry {
            Ok(registry) => Ok(Arc::clone(registry)),
            Err(error) => Err(error.clone()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flint_provider_api::Provider as _;

    #[test]
    fn requirements_select_their_exact_composed_provider() {
        let state = ModProviderState::new();
        let registry = state.registry().expect("mod provider registry");
        for requirement in mod_provider_requirements() {
            let provider = registry
                .select(&requirement)
                .expect("select composed mod provider");
            let expected_id = match requirement.selection_key {
                Some(CURSEFORGE_SELECTION_KEY) => "curseforge",
                Some(MODRINTH_SELECTION_KEY) => "modrinth",
                other => panic!("unexpected selection key: {other:?}"),
            };
            assert_eq!(provider.descriptor().id, expected_id);
        }
    }

    #[test]
    fn provider_state_reuses_one_registry_and_contains_both_providers() {
        let state = ModProviderState::new();
        let first = state.registry().expect("mod provider registry");
        let second = state.registry().expect("mod provider registry");
        assert!(Arc::ptr_eq(&first, &second));
        assert_eq!(
            first.provider_ids().collect::<Vec<_>>(),
            vec!["curseforge", "modrinth"]
        );
        assert_eq!(mod_provider_descriptors().len(), 2);
        assert_eq!(mod_provider_requirements().len(), 2);
    }

    #[test]
    fn loader_registry_and_managed_state_are_send_sync() {
        fn assert_send_sync<T: ?Sized + Send + Sync>() {}
        assert_send_sync::<dyn LoaderProvider>();
        assert_send_sync::<ProviderRegistry<dyn LoaderProvider>>();
        assert_send_sync::<LoaderProviderState>();
    }
    #[test]
    fn loader_provider_state_reports_stored_registry_failure_fail_closed() {
        let state = LoaderProviderState {
            registry: Err("registry initialization failed".to_string()),
        };
        assert_eq!(
            state.registry().expect_err("stored registry failure"),
            "registry initialization failed"
        );
    }
    #[test]
    fn loader_requirements_select_their_exact_composed_provider() {
        let state = LoaderProviderState::new();
        let registry = state.registry().expect("loader provider registry");
        let expected = [
            (FABRIC_SELECTION_KEY, "fabric"),
            (FORGE_SELECTION_KEY, "forge"),
            (LITELOADER_SELECTION_KEY, "liteloader"),
            (NEOFORGE_SELECTION_KEY, "neoforge"),
            (OPTIFINE_SELECTION_KEY, "optifine"),
            (QUILT_SELECTION_KEY, "quilt"),
        ];
        for (requirement, (selection_key, expected_id)) in
            loader_provider_requirements().into_iter().zip(expected)
        {
            assert_eq!(requirement.selection_key, Some(selection_key));
            let provider = registry
                .select(&requirement)
                .expect("select composed loader provider");
            assert_eq!(provider.descriptor().id, expected_id);
        }
    }

    #[test]
    fn loader_provider_state_reuses_one_registry_and_contains_exact_six() {
        let state = LoaderProviderState::new();
        let first = state.registry().expect("loader provider registry");
        let second = state.registry().expect("loader provider registry");
        assert!(Arc::ptr_eq(&first, &second));
        assert_eq!(
            first.provider_ids().collect::<Vec<_>>(),
            vec!["fabric", "forge", "liteloader", "neoforge", "optifine", "quilt"]
        );
        assert_eq!(
            loader_provider_descriptors()
                .iter()
                .map(|descriptor| descriptor.id)
                .collect::<Vec<_>>(),
            vec!["fabric", "forge", "liteloader", "neoforge", "optifine", "quilt"]
        );
        assert_eq!(
            loader_provider_requirements()
                .iter()
                .map(|requirement| requirement.selection_key)
                .collect::<Vec<_>>(),
            vec![
                Some(FABRIC_SELECTION_KEY),
                Some(FORGE_SELECTION_KEY),
                Some(LITELOADER_SELECTION_KEY),
                Some(NEOFORGE_SELECTION_KEY),
                Some(OPTIFINE_SELECTION_KEY),
                Some(QUILT_SELECTION_KEY),
            ]
        );
    }
}