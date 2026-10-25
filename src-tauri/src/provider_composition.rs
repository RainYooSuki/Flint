//! Explicit host composition for provider implementations.

use flint_feature_api::{ProviderAvailability, ProviderCatalog};
use flint_loader_provider_fabric::FabricLoaderProvider;
use flint_loader_provider_forge::ForgeLoaderProvider;
use flint_loader_provider_liteloader::LiteLoaderProvider;
use flint_loader_provider_neoforge::NeoForgeLoaderProvider;
use flint_loader_provider_optifine::OptiFineProvider;
use flint_loader_provider_quilt::QuiltLoaderProvider;
use flint_mod_provider_curseforge::CurseForgeModProvider;
use flint_mod_provider_modrinth::ModrinthModProvider;
use flint_provider_api::{
    loader_provider_requirement, mod_provider_requirement, LoaderProvider, ModProvider, Provider,
    ProviderRegistry, ProviderRequirement, ProviderSelectionError, CURSEFORGE_SELECTION_KEY,
    FABRIC_SELECTION_KEY, FORGE_SELECTION_KEY, LITELOADER_SELECTION_KEY, LOADER_PROVIDER_API,
    MODRINTH_SELECTION_KEY, MOD_PROVIDER_API, NEOFORGE_SELECTION_KEY, OPTIFINE_SELECTION_KEY,
    QUILT_SELECTION_KEY,
};
use std::{
    collections::BTreeSet,
    sync::{Arc, Mutex},
};

type ModRegistry = ProviderRegistry<dyn ModProvider>;
type LoaderRegistry = ProviderRegistry<dyn LoaderProvider>;

pub(crate) fn mod_provider_requirements() -> Vec<ProviderRequirement> {
    vec![
        mod_provider_requirement(Some(CURSEFORGE_SELECTION_KEY)),
        mod_provider_requirement(Some(MODRINTH_SELECTION_KEY)),
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

fn build_mod_provider_registry() -> Result<ModRegistry, String> {
    let providers: Vec<Box<dyn ModProvider>> = vec![
        Box::new(CurseForgeModProvider::new()),
        Box::new(ModrinthModProvider::new()),
    ];
    ProviderRegistry::new(providers, MOD_PROVIDER_API).map_err(|error| error.to_string())
}

fn build_loader_provider_registry() -> Result<LoaderRegistry, String> {
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

struct ProviderRuntimeInner {
    mod_registry: Result<Arc<ModRegistry>, String>,
    loader_registry: Result<Arc<LoaderRegistry>, String>,
    unavailable: Mutex<BTreeSet<String>>,
}

/// Shared lifecycle and command-facing truth for both immutable provider registries.
///
/// Clones share the same registries and provider availability state. Commands use the typed
/// registry accessors while the feature lifecycle consumes the same state through
/// [`ProviderCatalog`]; there is no separate metadata-only catalog.
#[derive(Clone)]
pub(crate) struct ProviderRuntimeState {
    inner: Arc<ProviderRuntimeInner>,
}

impl ProviderRuntimeState {
    pub(crate) fn new() -> Self {
        Self::from_registry_results(
            build_mod_provider_registry()
                .map(Arc::new)
                .map_err(|error| format!("Cannot initialize the mod provider registry: {error}")),
            build_loader_provider_registry()
                .map(Arc::new)
                .map_err(|error| format!("Cannot initialize the loader provider registry: {error}")),
        )
    }

    fn from_registry_results(
        mod_registry: Result<Arc<ModRegistry>, String>,
        loader_registry: Result<Arc<LoaderRegistry>, String>,
    ) -> Self {
        Self {
            inner: Arc::new(ProviderRuntimeInner {
                mod_registry,
                loader_registry,
                unavailable: Mutex::new(BTreeSet::new()),
            }),
        }
    }

    pub(crate) fn mod_registry(&self) -> Result<Arc<ModRegistry>, String> {
        match &self.inner.mod_registry {
            Ok(registry) => Ok(Arc::clone(registry)),
            Err(error) => Err(error.clone()),
        }
    }

    pub(crate) fn loader_registry(&self) -> Result<Arc<LoaderRegistry>, String> {
        match &self.inner.loader_registry {
            Ok(registry) => Ok(Arc::clone(registry)),
            Err(error) => Err(error.clone()),
        }
    }

    fn unavailable_ids(&self) -> BTreeSet<String> {
        match self.inner.unavailable.lock() {
            Ok(unavailable) => unavailable.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        }
    }

    fn provider_is_known(&self, provider_id: &str) -> bool {
        let mod_known = self
            .inner
            .mod_registry
            .as_ref()
            .map(|registry| registry.provider_ids().any(|known| known == provider_id))
            .unwrap_or(false);
        let loader_known = self
            .inner
            .loader_registry
            .as_ref()
            .map(|registry| registry.provider_ids().any(|known| known == provider_id))
            .unwrap_or(false);
        mod_known || loader_known
    }

    fn lookup_mod(
        &self,
        requirement: &ProviderRequirement,
        unavailable: &BTreeSet<String>,
    ) -> RegistryLookup {
        match &self.inner.mod_registry {
            Ok(registry) => registry_lookup(registry, requirement, unavailable),
            Err(error) => RegistryLookup::RegistryError(error.clone()),
        }
    }

    fn lookup_loader(
        &self,
        requirement: &ProviderRequirement,
        unavailable: &BTreeSet<String>,
    ) -> RegistryLookup {
        match &self.inner.loader_registry {
            Ok(registry) => registry_lookup(registry, requirement, unavailable),
            Err(error) => RegistryLookup::RegistryError(error.clone()),
        }
    }
}

#[cfg(test)]
pub(crate) mod test_support {
    use super::*;

    pub(crate) fn construction_failure_for_test() -> ProviderRuntimeState {
        let mod_error = match ProviderRegistry::new(
            vec![
                Box::new(CurseForgeModProvider::new()) as Box<dyn ModProvider>,
                Box::new(CurseForgeModProvider::new()) as Box<dyn ModProvider>,
            ],
            MOD_PROVIDER_API,
        ) {
            Ok(_) => panic!("duplicate mod provider registration unexpectedly succeeded"),
            Err(error) => error.to_string(),
        };
        let loader_error = match ProviderRegistry::new(
            vec![
                Box::new(FabricLoaderProvider::new()) as Box<dyn LoaderProvider>,
                Box::new(FabricLoaderProvider::new()) as Box<dyn LoaderProvider>,
            ],
            LOADER_PROVIDER_API,
        ) {
            Ok(_) => panic!("duplicate loader provider registration unexpectedly succeeded"),
            Err(error) => error.to_string(),
        };
        ProviderRuntimeState::from_registry_results(
            Err(format!("Cannot initialize the mod provider registry: {mod_error}")),
            Err(format!(
                "Cannot initialize the loader provider registry: {loader_error}"
            )),
        )
    }
}

impl ProviderCatalog for ProviderRuntimeState {
    fn provider_ids(&self) -> Vec<String> {
        let mut provider_ids = BTreeSet::new();
        if let Ok(registry) = &self.inner.mod_registry {
            provider_ids.extend(registry.provider_ids().map(str::to_string));
        }
        if let Ok(registry) = &self.inner.loader_registry {
            provider_ids.extend(registry.provider_ids().map(str::to_string));
        }
        provider_ids.into_iter().collect()
    }

    fn availability(&self, requirement: &ProviderRequirement) -> ProviderAvailability {
        let unavailable = self.unavailable_ids();
        let mut ambiguous = None;
        let mut errors = Vec::new();
        let mut reasons = Vec::new();
        for lookup in [
            self.lookup_mod(requirement, &unavailable),
            self.lookup_loader(requirement, &unavailable),
        ] {
            match lookup {
                RegistryLookup::Available => return ProviderAvailability::Available,
                RegistryLookup::Ambiguous { provider_ids } => {
                    if ambiguous.is_none() {
                        ambiguous = Some(provider_ids);
                    }
                }
                RegistryLookup::Unavailable { reason } => reasons.push(reason),
                RegistryLookup::RegistryError { error } => errors.push(error),
            }
        }
        if let Some(provider_ids) = ambiguous {
            return ProviderAvailability::Ambiguous { provider_ids };
        }
        if !errors.is_empty() {
            return ProviderAvailability::Unavailable {
                reason: errors.join("; "),
            };
        }
        ProviderAvailability::Unavailable {
            reason: reasons
                .into_iter()
                .next()
                .unwrap_or_else(|| "no provider matches the requirement".to_string()),
        }
    }

    fn initialize(&mut self, provider_id: &str) -> Result<(), String> {
        if !self.provider_is_known(provider_id) {
            return Err(format!(
                "Provider '{provider_id}' is not present in the shared provider runtime."
            ));
        }
        if self.unavailable_ids().contains(provider_id) {
            return Err(format!("Provider '{provider_id}' is unavailable."));
        }
        Ok(())
    }

    fn shutdown(&mut self, provider_id: &str) -> Result<(), String> {
        if !self.provider_is_known(provider_id) {
            return Err(format!(
                "Provider '{provider_id}' is not present in the shared provider runtime."
            ));
        }
        self.mark_unavailable(provider_id, "provider stopped");
        Ok(())
    }

    fn mark_unavailable(&mut self, provider_id: &str, _reason: &str) {
        match self.inner.unavailable.lock() {
            Ok(mut unavailable) => {
                unavailable.insert(provider_id.to_string());
            }
            Err(poisoned) => {
                poisoned
                    .into_inner()
                    .insert(provider_id.to_string());
            }
        }
    }
}

enum RegistryLookup {
    Available,
    Unavailable { reason: String },
    Ambiguous { provider_ids: Vec<String> },
    RegistryError { error: String },
}

fn registry_lookup<P: Provider + ?Sized>(
    registry: &ProviderRegistry<P>,
    requirement: &ProviderRequirement,
    unavailable: &BTreeSet<String>,
) -> RegistryLookup {
    match registry.select(requirement) {
        Ok(provider) if unavailable.contains(provider.descriptor().id) => RegistryLookup::Unavailable {
            reason: "selected provider is unavailable".to_string(),
        },
        Ok(_) => RegistryLookup::Available,
        Err(ProviderSelectionError::Ambiguous { provider_ids, .. }) => {
            RegistryLookup::Ambiguous { provider_ids }
        }
        Err(ProviderSelectionError::Unavailable { .. }) => RegistryLookup::Unavailable {
            reason: "no provider matches the requirement".to_string(),
        },
        Err(ProviderSelectionError::InvalidProviderVersionRequirement { .. }) => {
            RegistryLookup::Unavailable {
                reason: "invalid provider version requirement".to_string(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flint_feature_api::ProviderCatalog as _;
    use flint_provider_api::Provider as _;

    #[test]
    fn requirements_select_their_exact_composed_provider() {
        let state = ProviderRuntimeState::new();
        let mod_registry = state.mod_registry().expect("mod provider registry");
        for requirement in mod_provider_requirements() {
            let provider = mod_registry
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
    fn provider_runtime_reuses_both_registries_across_clones_and_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<ProviderRuntimeState>();
        let state = ProviderRuntimeState::new();
        let clone = state.clone();
        let first_mod = state.mod_registry().expect("mod provider registry");
        let second_mod = clone.mod_registry().expect("mod provider registry");
        let first_loader = state.loader_registry().expect("loader provider registry");
        let second_loader = clone
            .loader_registry()
            .expect("loader provider registry");
        assert!(Arc::ptr_eq(&first_mod, &second_mod));
        assert!(Arc::ptr_eq(&first_loader, &second_loader));
    }

    #[test]
    fn loader_requirements_select_their_exact_composed_provider() {
        let state = ProviderRuntimeState::new();
        let registry = state.loader_registry().expect("loader provider registry");
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
    fn provider_catalog_uses_actual_registry_descriptors_and_marks_loss_unavailable() {
        let mut state = ProviderRuntimeState::new();
        assert_eq!(
            state.provider_ids(),
            vec![
                "curseforge",
                "fabric",
                "forge",
                "liteloader",
                "modrinth",
                "neoforge",
                "optifine",
                "quilt",
            ]
        );
        for requirement in loader_provider_requirements() {
            assert!(matches!(
                state.availability(&requirement),
                ProviderAvailability::Available
            ));
        }
        for requirement in mod_provider_requirements() {
            assert!(matches!(
                state.availability(&requirement),
                ProviderAvailability::Available
            ));
        }

        state.mark_unavailable("fabric", "test provider loss");
        let fabric = loader_provider_requirements()
            .into_iter()
            .find(|requirement| requirement.selection_key == Some(FABRIC_SELECTION_KEY))
            .expect("fabric requirement");
        assert!(matches!(
            state.availability(&fabric),
            ProviderAvailability::Unavailable { .. }
        ));
        let forge = loader_provider_requirements()
            .into_iter()
            .find(|requirement| requirement.selection_key == Some(FORGE_SELECTION_KEY))
            .expect("forge requirement");
        assert!(matches!(
            state.availability(&forge),
            ProviderAvailability::Available
        ));
    }

    #[test]
    fn registry_construction_failure_is_stored_and_reported_fail_closed() {
        let state = super::test_support::construction_failure_for_test();
        match state.mod_registry() {
            Ok(_) => panic!("stored mod failure must fail closed"),
            Err(error) => {
                assert!(error.starts_with("Cannot initialize the mod provider registry: "));
                assert!(error.contains("registered more than once"));
            }
        }
        match state.loader_registry() {
            Ok(_) => panic!("stored loader failure must fail closed"),
            Err(error) => {
                assert!(error.starts_with("Cannot initialize the loader provider registry: "));
                assert!(error.contains("registered more than once"));
            }
        }
        assert!(state.provider_ids().is_empty());
        for requirement in loader_provider_requirements()
            .into_iter()
            .chain(mod_provider_requirements())
        {
            assert!(matches!(
                state.availability(&requirement),
                ProviderAvailability::Unavailable { .. }
            ));
        }
    }

    #[test]
    fn provider_initialize_and_shutdown_are_fail_closed_and_shutdown_is_idempotent() {
        let mut state = ProviderRuntimeState::new();
        state
            .initialize("fabric")
            .expect("initialize known provider");
        assert!(state.initialize("missing-provider").is_err());
        state.shutdown("fabric").expect("shutdown known provider");
        state.shutdown("fabric").expect("repeated shutdown");
        let fabric = loader_provider_requirements()
            .into_iter()
            .find(|requirement| requirement.selection_key == Some(FABRIC_SELECTION_KEY))
            .expect("fabric requirement");
        assert!(matches!(
            state.availability(&fabric),
            ProviderAvailability::Unavailable { .. }
        ));
        assert!(state.shutdown("missing-provider").is_err());
    }
}