//! Explicit host composition for mod provider implementations.

use flint_mod_provider_curseforge::CurseForgeModProvider;
use flint_mod_provider_modrinth::ModrinthModProvider;
use flint_provider_api::{
    mod_provider_requirement, ModProvider, ProviderDescriptor, ProviderRegistry,
    ProviderRequirement, CURSEFORGE_SELECTION_KEY, MODRINTH_SELECTION_KEY, MOD_PROVIDER_API,
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

fn build_mod_provider_registry() -> Result<ProviderRegistry<dyn ModProvider>, String> {
    let providers: Vec<Box<dyn ModProvider>> = vec![
        Box::new(CurseForgeModProvider::new()),
        Box::new(ModrinthModProvider::new()),
    ];
    ProviderRegistry::new(providers, MOD_PROVIDER_API).map_err(|error| error.to_string())
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
}
