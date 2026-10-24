use super::{
    curseforge_list_versions, curseforge_search, modrinth_list_versions, modrinth_search,
    modrinth_version_by_id, validate_provider_id,
};
use crate::models::{ModDependencyRef, ModSearchPage, ModSource, ModVersionInfo};
use flint_provider_api::{
    ApiVersion, Provider, ProviderCapability, ProviderDescriptor, ProviderRequirement,
    ProviderRegistry,
};
use reqwest::blocking::Client;

const MOD_CATALOG_CAPABILITY_ID: &str = "mods.catalog";
const MOD_CATALOG_API: ApiVersion = ApiVersion::new(1, 0);
const MOD_PROVIDER_API: ApiVersion = ApiVersion::new(1, 0);
const MOD_PROVIDER_VERSION: &str = "1.0.0";
const MOD_PROVIDER_VERSION_REQUIREMENT: &str = "^1.0.0";

const CURSEFORGE_PROVIDER_ID: &str = "curseforge";
const CURSEFORGE_SELECTION_KEY: &str = "mod-source:curseforge";
const CURSEFORGE_SELECTION_KEYS: &[&str] = &[CURSEFORGE_SELECTION_KEY];
const CURSEFORGE_CAPABILITIES: &[ProviderCapability] = &[ProviderCapability {
    id: MOD_CATALOG_CAPABILITY_ID,
    api_version: MOD_CATALOG_API,
    selection_keys: CURSEFORGE_SELECTION_KEYS,
}];

const MODRINTH_PROVIDER_ID: &str = "modrinth";
const MODRINTH_SELECTION_KEY: &str = "mod-source:modrinth";
const MODRINTH_SELECTION_KEYS: &[&str] = &[MODRINTH_SELECTION_KEY];
const MODRINTH_CAPABILITIES: &[ProviderCapability] = &[ProviderCapability {
    id: MOD_CATALOG_CAPABILITY_ID,
    api_version: MOD_CATALOG_API,
    selection_keys: MODRINTH_SELECTION_KEYS,
}];

/// Operation-scoped provider I/O context. Providers receive no host handles,
/// configuration-file locations, or provider-specific state outside this boundary.
pub(super) struct ModProviderContext<'a> {
    client: &'a Client,
    api_key: Option<String>,
}

impl<'a> ModProviderContext<'a> {
    fn client(&self) -> &'a Client {
        self.client
    }

    fn api_key(&self) -> Option<&str> {
        self.api_key.as_deref()
    }
}

pub(super) trait ModProvider: Provider {
    fn search(
        &self,
        context: &ModProviderContext<'_>,
        query: &str,
        game_version: &str,
        loader: &str,
        offset: u32,
        limit: u32,
    ) -> Result<ModSearchPage, String>;

    fn list_versions(
        &self,
        context: &ModProviderContext<'_>,
        project_id: &str,
        game_version: &str,
        loader: &str,
    ) -> Result<Vec<ModVersionInfo>, String>;

    fn version_by_id(
        &self,
        context: &ModProviderContext<'_>,
        project_id: &str,
        version_id: &str,
        game_version: &str,
        loader: &str,
    ) -> Result<ModVersionInfo, String>;

    fn dependency_version(
        &self,
        context: &ModProviderContext<'_>,
        dependency: &ModDependencyRef,
        game_version: &str,
        loader: &str,
    ) -> Result<Option<ModVersionInfo>, String>;
}

struct CurseForgeProvider;
struct ModrinthProvider;

impl Provider for CurseForgeProvider {
    fn descriptor(&self) -> ProviderDescriptor {
        ProviderDescriptor {
            id: CURSEFORGE_PROVIDER_ID,
            provider_version: MOD_PROVIDER_VERSION,
            provider_api: MOD_PROVIDER_API,
            capabilities: CURSEFORGE_CAPABILITIES,
        }
    }
}

impl Provider for ModrinthProvider {
    fn descriptor(&self) -> ProviderDescriptor {
        ProviderDescriptor {
            id: MODRINTH_PROVIDER_ID,
            provider_version: MOD_PROVIDER_VERSION,
            provider_api: MOD_PROVIDER_API,
            capabilities: MODRINTH_CAPABILITIES,
        }
    }
}

pub(crate) fn provider_descriptors() -> Vec<ProviderDescriptor> {
    vec![CurseForgeProvider.descriptor(), ModrinthProvider.descriptor()]
}

impl ModProvider for CurseForgeProvider {
    fn search(
        &self,
        context: &ModProviderContext<'_>,
        query: &str,
        game_version: &str,
        loader: &str,
        offset: u32,
        limit: u32,
    ) -> Result<ModSearchPage, String> {
        let api_key = require_context_api_key(context, "this operation")?;
        curseforge_search(
            context.client(),
            api_key,
            query,
            game_version,
            loader,
            offset,
            limit,
        )
    }

    fn list_versions(
        &self,
        context: &ModProviderContext<'_>,
        project_id: &str,
        game_version: &str,
        loader: &str,
    ) -> Result<Vec<ModVersionInfo>, String> {
        let api_key = require_context_api_key(context, "this operation")?;
        curseforge_list_versions(
            context.client(),
            api_key,
            project_id,
            game_version,
            loader,
        )
    }

    fn version_by_id(
        &self,
        context: &ModProviderContext<'_>,
        project_id: &str,
        version_id: &str,
        game_version: &str,
        loader: &str,
    ) -> Result<ModVersionInfo, String> {
        validate_provider_id(version_id)?;
        let api_key = require_context_api_key(context, "this operation")?;
        let versions = curseforge_list_versions(
            context.client(),
            api_key,
            project_id,
            game_version,
            loader,
        )?;
        versions
            .into_iter()
            .find(|version| {
                version.id.eq_ignore_ascii_case(version_id)
                    && version.download_url.is_some()
                    && (game_version.is_empty()
                        || version.game_versions.is_empty()
                        || version
                            .game_versions
                            .iter()
                            .any(|candidate| candidate.eq_ignore_ascii_case(game_version)))
                    && (loader == "vanilla"
                        || version.loaders.is_empty()
                        || version
                            .loaders
                            .iter()
                            .any(|candidate| candidate.eq_ignore_ascii_case(loader)))
            })
            .ok_or_else(|| {
                "No compatible downloadable file was found for the selected mod version."
                    .to_string()
            })
    }

    fn dependency_version(
        &self,
        context: &ModProviderContext<'_>,
        dependency: &ModDependencyRef,
        game_version: &str,
        loader: &str,
    ) -> Result<Option<ModVersionInfo>, String> {
        if dependency.project_id.is_empty() {
            return Ok(None);
        }
        let api_key = require_context_api_key(context, "dependency resolution")?;
        let versions = curseforge_list_versions(
            context.client(),
            api_key,
            &dependency.project_id,
            game_version,
            loader,
        )?;
        Ok(versions.into_iter().next())
    }
}

impl ModProvider for ModrinthProvider {
    fn search(
        &self,
        context: &ModProviderContext<'_>,
        query: &str,
        game_version: &str,
        loader: &str,
        offset: u32,
        limit: u32,
    ) -> Result<ModSearchPage, String> {
        modrinth_search(
            context.client(),
            query,
            game_version,
            loader,
            offset,
            limit,
        )
    }

    fn list_versions(
        &self,
        context: &ModProviderContext<'_>,
        project_id: &str,
        game_version: &str,
        loader: &str,
    ) -> Result<Vec<ModVersionInfo>, String> {
        modrinth_list_versions(context.client(), project_id, game_version, loader)
    }

    fn version_by_id(
        &self,
        context: &ModProviderContext<'_>,
        _project_id: &str,
        version_id: &str,
        _game_version: &str,
        _loader: &str,
    ) -> Result<ModVersionInfo, String> {
        modrinth_version_by_id(context.client(), version_id)
    }

    fn dependency_version(
        &self,
        context: &ModProviderContext<'_>,
        dependency: &ModDependencyRef,
        game_version: &str,
        loader: &str,
    ) -> Result<Option<ModVersionInfo>, String> {
        if let Some(version_id) = dependency.version_id.as_deref() {
            return modrinth_version_by_id(context.client(), version_id).map(Some);
        }
        if dependency.project_id.is_empty() {
            return Ok(None);
        }
        let versions =
            modrinth_list_versions(context.client(), &dependency.project_id, game_version, loader)?;
        Ok(versions.into_iter().next())
    }
}

fn require_context_api_key<'a>(
    context: &'a ModProviderContext<'_>,
    operation: &str,
) -> Result<&'a str, String> {
    context.api_key().ok_or_else(|| {
        format!("CurseForge API key is required for {operation}.")
    })
}

fn mod_provider_requirement(selection_key: &'static str) -> ProviderRequirement {
    ProviderRequirement {
        capability: MOD_CATALOG_CAPABILITY_ID,
        api_version: MOD_CATALOG_API,
        provider_version_req: MOD_PROVIDER_VERSION_REQUIREMENT,
        selection_key: Some(selection_key),
    }
}

pub(crate) fn provider_requirements() -> Vec<ProviderRequirement> {
    vec![
        mod_provider_requirement(CURSEFORGE_SELECTION_KEY),
        mod_provider_requirement(MODRINTH_SELECTION_KEY),
    ]
}

struct SourceBinding {
    requirement: ProviderRequirement,
    api_key: Option<String>,
}

/// The sole serialized `ModSource` to provider-binding mapping. It resolves
/// the complete requirement and provider-specific credential context before
/// registry selection.
fn source_binding(source: ModSource) -> Result<SourceBinding, String> {
    match source {
        ModSource::Modrinth => Ok(SourceBinding {
            requirement: mod_provider_requirement(MODRINTH_SELECTION_KEY),
            api_key: None,
        }),
        ModSource::Curseforge => Ok(SourceBinding {
            requirement: mod_provider_requirement(CURSEFORGE_SELECTION_KEY),
            api_key: Some(super::require_curseforge_key()?),
        }),
    }
}

/// Builds the immutable mod-provider registry in lexicographic provider-ID
/// order. The registry also sorts defensively, so registration order cannot
/// affect selection or iteration.
pub(super) fn build_registry() -> Result<ProviderRegistry<dyn ModProvider>, String> {
    let curseforge: Box<dyn ModProvider> = Box::new(CurseForgeProvider);
    let modrinth: Box<dyn ModProvider> = Box::new(ModrinthProvider);
    let providers = vec![curseforge, modrinth];
    ProviderRegistry::new(providers, MOD_PROVIDER_API)
        .map_err(|error| format!("Cannot initialize the mod provider registry: {error}"))
}

pub(super) struct SelectedModProvider<'a> {
    pub(super) provider: &'a dyn ModProvider,
    pub(super) context: ModProviderContext<'a>,
}

pub(super) fn select_for_source<'a>(
    registry: &'a ProviderRegistry<dyn ModProvider>,
    source: ModSource,
    client: &'a Client,
) -> Result<SelectedModProvider<'a>, String> {
    let binding = source_binding(source)?;
    let provider = registry.select(&binding.requirement).map_err(|error| {
        format!(
            "Cannot select the {} mod provider: {error}",
            source.as_str()
        )
    })?;
    Ok(SelectedModProvider {
        provider,
        context: ModProviderContext {
            client,
            api_key: binding.api_key,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use flint_provider_api::Provider as _;

    #[test]
    fn providers_declare_the_versioned_mod_catalog_contract() {
        let curseforge = CurseForgeProvider.descriptor();
        assert_eq!(curseforge.id, CURSEFORGE_PROVIDER_ID);
        assert_eq!(curseforge.provider_version, MOD_PROVIDER_VERSION);
        assert_eq!(curseforge.provider_api, MOD_PROVIDER_API);
        assert_eq!(curseforge.capabilities.len(), 1);
        assert_eq!(
            curseforge.capabilities[0].id,
            MOD_CATALOG_CAPABILITY_ID
        );
        assert_eq!(curseforge.capabilities[0].api_version, MOD_CATALOG_API);
        assert_eq!(
            curseforge.capabilities[0].selection_keys,
            CURSEFORGE_SELECTION_KEYS
        );

        let modrinth = ModrinthProvider.descriptor();
        assert_eq!(modrinth.id, MODRINTH_PROVIDER_ID);
        assert_eq!(modrinth.provider_version, MOD_PROVIDER_VERSION);
        assert_eq!(modrinth.provider_api, MOD_PROVIDER_API);
        assert_eq!(modrinth.capabilities.len(), 1);
        assert_eq!(modrinth.capabilities[0].id, MOD_CATALOG_CAPABILITY_ID);
        assert_eq!(modrinth.capabilities[0].api_version, MOD_CATALOG_API);
        assert_eq!(
            modrinth.capabilities[0].selection_keys,
            MODRINTH_SELECTION_KEYS
        );
    }

    #[test]
    fn mod_provider_registry_is_lexicographically_ordered() {
        let registry = build_registry().expect("mod provider registry");
        assert_eq!(
            registry.provider_ids().collect::<Vec<_>>(),
            vec![CURSEFORGE_PROVIDER_ID, MODRINTH_PROVIDER_ID]
        );

        let selected = registry
            .select(&ProviderRequirement {
                capability: MOD_CATALOG_CAPABILITY_ID,
                api_version: MOD_CATALOG_API,
                provider_version_req: MOD_PROVIDER_VERSION_REQUIREMENT,
                selection_key: Some(CURSEFORGE_SELECTION_KEY),
            })
            .expect("curseforge provider selection");
        assert_eq!(selected.descriptor().id, CURSEFORGE_PROVIDER_ID);

        let selected = registry
            .select(&ProviderRequirement {
                capability: MOD_CATALOG_CAPABILITY_ID,
                api_version: MOD_CATALOG_API,
                provider_version_req: MOD_PROVIDER_VERSION_REQUIREMENT,
                selection_key: Some(MODRINTH_SELECTION_KEY),
            })
            .expect("modrinth provider selection");
        assert_eq!(selected.descriptor().id, MODRINTH_PROVIDER_ID);
    }

    #[test]
    fn modrinth_source_binds_to_its_exact_selection_key() {
        let binding = source_binding(ModSource::Modrinth).expect("modrinth source binding");
        assert_eq!(binding.requirement.capability, MOD_CATALOG_CAPABILITY_ID);
        assert_eq!(binding.requirement.api_version, MOD_CATALOG_API);
        assert_eq!(
            binding.requirement.provider_version_req,
            MOD_PROVIDER_VERSION_REQUIREMENT
        );
        assert_eq!(
            binding.requirement.selection_key,
            Some(MODRINTH_SELECTION_KEY)
        );
        assert!(binding.api_key.is_none());
    }
}
