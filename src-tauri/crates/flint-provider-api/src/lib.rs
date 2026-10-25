use semver::{Version, VersionReq};
use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
};

pub use flint_core::contract::{ApiVersion, ProviderRequirement};

pub mod mods;
pub use mods::*;

/// A capability exposed by a provider. Selection keys are scoped by the
/// capability ID and must be unique among providers exposing that capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderCapability {
    pub id: &'static str,
    pub api_version: ApiVersion,
    pub selection_keys: &'static [&'static str],
}

/// Immutable descriptor declared by a provider implementation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderDescriptor {
    pub id: &'static str,
    pub provider_version: &'static str,
    pub provider_api: ApiVersion,
    pub capabilities: &'static [ProviderCapability],
}

/// Contract implemented by any registrable provider.
pub trait Provider {
    fn descriptor(&self) -> ProviderDescriptor;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderRegistryError {
    InvalidProviderId {
        provider_id: String,
        reason: &'static str,
    },
    DuplicateProviderId {
        provider_id: String,
    },
    InvalidProviderVersion {
        provider_id: String,
        provider_version: String,
        reason: String,
    },
    IncompatibleProviderApi {
        provider_id: String,
        provider_api: ApiVersion,
        host_api: ApiVersion,
    },
    InvalidCapabilityId {
        provider_id: String,
        capability_id: String,
        reason: &'static str,
    },
    DuplicateCapabilityId {
        provider_id: String,
        capability_id: String,
    },
    InvalidSelectionKey {
        provider_id: String,
        capability_id: String,
        selection_key: String,
        reason: &'static str,
    },
    DuplicateSelectionKey {
        capability_id: String,
        selection_key: String,
        first_provider_id: String,
        duplicate_provider_id: String,
    },
}

impl fmt::Display for ProviderRegistryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidProviderId {
                provider_id,
                reason,
            } => write!(formatter, "provider ID '{provider_id}' is invalid: {reason}"),
            Self::DuplicateProviderId { provider_id } => {
                write!(formatter, "provider ID '{provider_id}' is registered more than once")
            }
            Self::InvalidProviderVersion {
                provider_id,
                provider_version,
                reason,
            } => write!(
                formatter,
                "provider '{provider_id}' declares invalid semantic version '{provider_version}': {reason}"
            ),
            Self::IncompatibleProviderApi {
                provider_id,
                provider_api,
                host_api,
            } => write!(
                formatter,
                "provider '{provider_id}' API {provider_api} is incompatible with host provider API {host_api}; the major versions must match and the provider minor must not be newer"
            ),
            Self::InvalidCapabilityId {
                provider_id,
                capability_id,
                reason,
            } => write!(
                formatter,
                "provider '{provider_id}' declares invalid capability ID '{capability_id}': {reason}"
            ),
            Self::DuplicateCapabilityId {
                provider_id,
                capability_id,
            } => write!(
                formatter,
                "provider '{provider_id}' declares capability '{capability_id}' more than once"
            ),
            Self::InvalidSelectionKey {
                provider_id,
                capability_id,
                selection_key,
                reason,
            } => write!(
                formatter,
                "provider '{provider_id}' capability '{capability_id}' declares invalid selection key '{selection_key}': {reason}"
            ),
            Self::DuplicateSelectionKey {
                capability_id,
                selection_key,
                first_provider_id,
                duplicate_provider_id,
            } => write!(
                formatter,
                "selection key '{selection_key}' for capability '{capability_id}' is registered by provider '{first_provider_id}' and provider '{duplicate_provider_id}'"
            ),
        }
    }
}

impl Error for ProviderRegistryError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderSelectionError {
    InvalidProviderVersionRequirement {
        provider_version_req: String,
        reason: String,
    },
    Unavailable {
        capability: String,
        api_version: ApiVersion,
        provider_version_req: String,
        selection_key: Option<String>,
    },
    Ambiguous {
        capability: String,
        api_version: ApiVersion,
        provider_version_req: String,
        selection_key: Option<String>,
        provider_ids: Vec<String>,
    },
}

impl fmt::Display for ProviderSelectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidProviderVersionRequirement {
                provider_version_req,
                reason,
            } => write!(
                formatter,
                "provider version requirement '{provider_version_req}' is invalid: {reason}"
            ),
            Self::Unavailable {
                capability,
                api_version,
                provider_version_req,
                selection_key,
            } => write!(
                formatter,
                "no active provider is available for capability '{capability}' (capability API {api_version}, provider requirement '{provider_version_req}', {}).",
                selection_key_description(selection_key)
            ),
            Self::Ambiguous {
                capability,
                api_version,
                provider_version_req,
                selection_key,
                provider_ids,
            } => write!(
                formatter,
                "provider selection is ambiguous for capability '{capability}' (capability API {api_version}, provider requirement '{provider_version_req}', {}); matching providers: {}",
                selection_key_description(selection_key),
                provider_ids.join(", ")
            ),
        }
    }
}

impl Error for ProviderSelectionError {}

fn selection_key_description(selection_key: &Option<String>) -> String {
    match selection_key {
        Some(key) => format!("selection key '{key}'"),
        None => "no selection key".to_string(),
    }
}

struct RegisteredProvider<P: Provider + ?Sized> {
    provider: Box<P>,
    descriptor: ProviderDescriptor,
}

/// An immutable, validated, deterministically ordered provider registry.
///
/// Every successfully constructed entry is considered active for selection.
/// Provider lifecycle integration can later remove or mark entries unavailable
/// without changing the selection predicate.
pub struct ProviderRegistry<P: Provider + ?Sized> {
    entries: Vec<RegisteredProvider<P>>,
    host_api: ApiVersion,
}

impl<P: Provider + ?Sized> ProviderRegistry<P> {
    pub fn new(
        providers: Vec<Box<P>>,
        host_api: ApiVersion,
    ) -> Result<Self, ProviderRegistryError> {
        let mut entries: Vec<RegisteredProvider<P>> = providers
            .into_iter()
            .map(|provider| {
                let descriptor = provider.descriptor();
                RegisteredProvider {
                    provider,
                    descriptor,
                }
            })
            .collect();
        entries.sort_by_key(|entry| entry.descriptor.id);

        for pair in entries.windows(2) {
            if pair[0].descriptor.id == pair[1].descriptor.id {
                return Err(ProviderRegistryError::DuplicateProviderId {
                    provider_id: pair[0].descriptor.id.to_string(),
                });
            }
        }

        // Selection keys are qualified by capability ID. A key may be reused
        // under a different capability, but the same capability/key binding
        // must identify at most one provider.
        let mut selection_keys: BTreeMap<(String, String), String> = BTreeMap::new();
        for entry in &entries {
            let descriptor = &entry.descriptor;
            if let Some(reason) = stable_id_error(descriptor.id) {
                return Err(ProviderRegistryError::InvalidProviderId {
                    provider_id: descriptor.id.to_string(),
                    reason,
                });
            }
            if let Err(error) = Version::parse(descriptor.provider_version) {
                return Err(ProviderRegistryError::InvalidProviderVersion {
                    provider_id: descriptor.id.to_string(),
                    provider_version: descriptor.provider_version.to_string(),
                    reason: error.to_string(),
                });
            }
            if !descriptor.provider_api.is_compatible_with(host_api) {
                return Err(ProviderRegistryError::IncompatibleProviderApi {
                    provider_id: descriptor.id.to_string(),
                    provider_api: descriptor.provider_api,
                    host_api,
                });
            }

            let mut capability_ids = BTreeSet::new();
            for capability in descriptor.capabilities {
                if let Some(reason) = stable_id_error(capability.id) {
                    return Err(ProviderRegistryError::InvalidCapabilityId {
                        provider_id: descriptor.id.to_string(),
                        capability_id: capability.id.to_string(),
                        reason,
                    });
                }
                if !capability_ids.insert(capability.id) {
                    return Err(ProviderRegistryError::DuplicateCapabilityId {
                        provider_id: descriptor.id.to_string(),
                        capability_id: capability.id.to_string(),
                    });
                }

                let mut local_selection_keys = BTreeSet::new();
                for selection_key in capability.selection_keys {
                    if let Some(reason) = stable_id_error(*selection_key) {
                        return Err(ProviderRegistryError::InvalidSelectionKey {
                            provider_id: descriptor.id.to_string(),
                            capability_id: capability.id.to_string(),
                            selection_key: selection_key.to_string(),
                            reason,
                        });
                    }
                    if !local_selection_keys.insert(*selection_key) {
                        return Err(ProviderRegistryError::DuplicateSelectionKey {
                            capability_id: capability.id.to_string(),
                            selection_key: selection_key.to_string(),
                            first_provider_id: descriptor.id.to_string(),
                            duplicate_provider_id: descriptor.id.to_string(),
                        });
                    }

                    let key_binding = (capability.id.to_string(), selection_key.to_string());
                    if let Some(first_provider_id) =
                        selection_keys.insert(key_binding, descriptor.id.to_string())
                    {
                        return Err(ProviderRegistryError::DuplicateSelectionKey {
                            capability_id: capability.id.to_string(),
                            selection_key: selection_key.to_string(),
                            first_provider_id,
                            duplicate_provider_id: descriptor.id.to_string(),
                        });
                    }
                }
            }
        }

        Ok(Self { entries, host_api })
    }

    pub fn select(
        &self,
        requirement: &ProviderRequirement,
    ) -> Result<&P, ProviderSelectionError> {
        let provider_version_req =
            VersionReq::parse(requirement.provider_version_req).map_err(|error| {
                ProviderSelectionError::InvalidProviderVersionRequirement {
                    provider_version_req: requirement.provider_version_req.to_string(),
                    reason: error.to_string(),
                }
            })?;

        let mut matching = Vec::new();
        for entry in &self.entries {
            let descriptor = &entry.descriptor;
            if !descriptor.provider_api.is_compatible_with(self.host_api) {
                continue;
            }
            let Some(capability) = descriptor
                .capabilities
                .iter()
                .find(|capability| capability.id == requirement.capability)
            else {
                continue;
            };
            if !requirement
                .api_version
                .is_compatible_with(capability.api_version)
            {
                continue;
            }
            let Ok(provider_version) = Version::parse(descriptor.provider_version) else {
                // Construction validates this invariant. Failing closed here
                // keeps a mutated provider descriptor from being selected.
                continue;
            };
            if !provider_version_req.matches(&provider_version) {
                continue;
            }
            if let Some(selection_key) = requirement.selection_key {
                if !capability.selection_keys.contains(&selection_key) {
                    continue;
                }
            }
            matching.push(entry);
        }

        if matching.is_empty() {
            return Err(ProviderSelectionError::Unavailable {
                capability: requirement.capability.to_string(),
                api_version: requirement.api_version,
                provider_version_req: requirement.provider_version_req.to_string(),
                selection_key: requirement.selection_key.map(str::to_string),
            });
        }
        if matching.len() > 1 {
            return Err(ProviderSelectionError::Ambiguous {
                capability: requirement.capability.to_string(),
                api_version: requirement.api_version,
                provider_version_req: requirement.provider_version_req.to_string(),
                selection_key: requirement.selection_key.map(str::to_string),
                provider_ids: matching
                    .iter()
                    .map(|entry| entry.descriptor.id.to_string())
                    .collect(),
            });
        }

        Ok(&*matching[0].provider)
    }

    pub fn provider_ids(&self) -> impl Iterator<Item = &str> + '_ {
        self.entries.iter().map(|entry| entry.descriptor.id)
    }

    pub fn descriptors(&self) -> impl ExactSizeIterator<Item = &ProviderDescriptor> + '_ {
        self.entries.iter().map(|entry| &entry.descriptor)
    }

    pub fn providers(&self) -> impl ExactSizeIterator<Item = &P> + '_ {
        self.entries.iter().map(|entry| &*entry.provider)
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &P> + '_ {
        self.providers()
    }

    pub fn host_api(&self) -> ApiVersion {
        self.host_api
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

fn stable_id_error(value: &str) -> Option<&'static str> {
    let bytes = value.as_bytes();
    if bytes.is_empty() {
        return Some("must not be empty");
    }
    if bytes.len() > 128 {
        return Some("must be at most 128 bytes");
    }
    if !bytes[0].is_ascii_lowercase() {
        return Some("must start with a lowercase ASCII letter");
    }
    if !bytes[bytes.len() - 1].is_ascii_alphanumeric() {
        return Some("must end with a lowercase ASCII letter or digit");
    }
    if bytes.iter().copied().any(|byte| {
        !(byte.is_ascii_lowercase()
            || byte.is_ascii_digit()
            || matches!(byte, b'.' | b'-' | b'_' | b':'))
    }) {
        return Some(
            "may contain only lowercase ASCII letters, digits, '.', '-', '_', and ':'",
        );
    }
    if bytes
        .windows(2)
        .any(|pair| is_separator(pair[0]) && is_separator(pair[1]))
    {
        return Some("must not contain consecutive separators");
    }
    None
}

fn is_separator(byte: u8) -> bool {
    matches!(byte, b'.' | b'-' | b'_' | b':')
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOST_API: ApiVersion = ApiVersion::new(1, 0);

    const CATALOG_PRIMARY: ProviderCapability = ProviderCapability {
        id: "mods.catalog",
        api_version: ApiVersion::new(1, 0),
        selection_keys: &["mod-source:alpha"],
    };
    const CATALOG_SECONDARY: ProviderCapability = ProviderCapability {
        id: "mods.catalog",
        api_version: ApiVersion::new(1, 0),
        selection_keys: &["mod-source:beta"],
    };
    const CATALOG_WITHOUT_KEYS: ProviderCapability = ProviderCapability {
        id: "mods.catalog",
        api_version: ApiVersion::new(1, 0),
        selection_keys: &[],
    };
    const OTHER_CAPABILITY: ProviderCapability = ProviderCapability {
        id: "mods.other",
        api_version: ApiVersion::new(1, 0),
        selection_keys: &["mod-source:alpha"],
    };
    const INVALID_CAPABILITY: ProviderCapability = ProviderCapability {
        id: "Bad.capability",
        api_version: ApiVersion::new(1, 0),
        selection_keys: &["mod-source:alpha"],
    };
    const INVALID_CAPABILITY_LIST: &[ProviderCapability] = &[INVALID_CAPABILITY];
    const INVALID_SELECTION_KEY_CAPABILITY: ProviderCapability = ProviderCapability {
        id: "mods.catalog",
        api_version: ApiVersion::new(1, 0),
        selection_keys: &["not a stable key"],
    };
    const INVALID_SELECTION_KEY_LIST: &[ProviderCapability] = &[INVALID_SELECTION_KEY_CAPABILITY];
    const DUP_CAPABILITY_LIST: &[ProviderCapability] = &[CATALOG_PRIMARY, CATALOG_PRIMARY];
    const DUP_SELECTION_KEY: ProviderCapability = ProviderCapability {
        id: "mods.catalog",
        api_version: ApiVersion::new(1, 0),
        selection_keys: &["mod-source:alpha", "mod-source:alpha"],
    };
    const DUP_SELECTION_KEY_LIST: &[ProviderCapability] = &[DUP_SELECTION_KEY];
    const PRIMARY_CAPABILITIES: &[ProviderCapability] = &[CATALOG_PRIMARY];
    const SECONDARY_CAPABILITIES: &[ProviderCapability] = &[CATALOG_SECONDARY];
    const UNKEYED_CAPABILITIES: &[ProviderCapability] = &[CATALOG_WITHOUT_KEYS];
    const OTHER_CAPABILITIES: &[ProviderCapability] = &[OTHER_CAPABILITY];

    #[derive(Debug)]
    struct MockProvider {
        descriptor: ProviderDescriptor,
    }

    impl Provider for MockProvider {
        fn descriptor(&self) -> ProviderDescriptor {
            self.descriptor
        }
    }

    fn provider(
        id: &'static str,
        provider_version: &'static str,
        provider_api: ApiVersion,
        capabilities: &'static [ProviderCapability],
    ) -> Box<MockProvider> {
        Box::new(MockProvider {
            descriptor: ProviderDescriptor {
                id,
                provider_version,
                provider_api,
                capabilities,
            },
        })
    }

    fn registry(
        providers: Vec<Box<MockProvider>>,
    ) -> Result<ProviderRegistry<MockProvider>, ProviderRegistryError> {
        ProviderRegistry::new(providers, HOST_API)
    }

    fn requirement(
        capability: &'static str,
        api_version: ApiVersion,
        provider_version_req: &'static str,
        selection_key: Option<&'static str>,
    ) -> ProviderRequirement {
        ProviderRequirement {
            capability,
            api_version,
            provider_version_req,
            selection_key,
        }
    }

    #[test]
    fn api_compatibility_requires_matching_major_and_older_or_equal_minor() {
        assert!(ApiVersion::new(1, 0).is_compatible_with(ApiVersion::new(1, 0)));
        assert!(ApiVersion::new(1, 1).is_compatible_with(ApiVersion::new(1, 4)));
        assert!(!ApiVersion::new(1, 5).is_compatible_with(ApiVersion::new(1, 4)));
        assert!(!ApiVersion::new(2, 0).is_compatible_with(ApiVersion::new(1, 9)));
        assert!(ApiVersion::new(1, 4).supports(ApiVersion::new(1, 2)));
    }

    #[test]
    fn registry_accepts_valid_descriptors_and_sorts_provider_ids() {
        let registry = registry(vec![
            provider("zeta", "1.0.0", HOST_API, UNKEYED_CAPABILITIES),
            provider("alpha", "1.0.0", HOST_API, UNKEYED_CAPABILITIES),
            provider("middle", "1.0.0", HOST_API, UNKEYED_CAPABILITIES),
        ])
        .expect("valid registry");

        assert_eq!(
            registry.provider_ids().collect::<Vec<_>>(),
            vec!["alpha", "middle", "zeta"]
        );
        assert_eq!(registry.len(), 3);
    }

    #[test]
    fn registry_rejects_invalid_provider_descriptors() {
        let empty_id = registry(vec![provider("", "1.0.0", HOST_API, PRIMARY_CAPABILITIES)]);
        assert!(matches!(
            empty_id,
            Err(ProviderRegistryError::InvalidProviderId { .. })
        ));

        let invalid_case = registry(vec![provider(
            "Alpha",
            "1.0.0",
            HOST_API,
            PRIMARY_CAPABILITIES,
        )]);
        assert!(matches!(
            invalid_case,
            Err(ProviderRegistryError::InvalidProviderId { .. })
        ));

        let invalid_version = registry(vec![provider(
            "alpha",
            "not-semver",
            HOST_API,
            PRIMARY_CAPABILITIES,
        )]);
        assert!(matches!(
            invalid_version,
            Err(ProviderRegistryError::InvalidProviderVersion { .. })
        ));

        let incompatible_api = registry(vec![provider(
            "alpha",
            "1.0.0",
            ApiVersion::new(1, 1),
            PRIMARY_CAPABILITIES,
        )]);
        assert!(matches!(
            incompatible_api,
            Err(ProviderRegistryError::IncompatibleProviderApi { .. })
        ));

        let invalid_capability = registry(vec![provider(
            "alpha",
            "1.0.0",
            HOST_API,
            INVALID_CAPABILITY_LIST,
        )]);
        assert!(matches!(
            invalid_capability,
            Err(ProviderRegistryError::InvalidCapabilityId { .. })
        ));

        let invalid_selection_key = registry(vec![provider(
            "alpha",
            "1.0.0",
            HOST_API,
            INVALID_SELECTION_KEY_LIST,
        )]);
        assert!(matches!(
            invalid_selection_key,
            Err(ProviderRegistryError::InvalidSelectionKey { .. })
        ));
    }

    #[test]
    fn registry_rejects_duplicate_provider_capability_and_selection_key_bindings() {
        let duplicate_provider = registry(vec![
            provider("alpha", "1.0.0", HOST_API, PRIMARY_CAPABILITIES),
            provider("alpha", "2.0.0", HOST_API, SECONDARY_CAPABILITIES),
        ]);
        assert!(matches!(
            duplicate_provider,
            Err(ProviderRegistryError::DuplicateProviderId { .. })
        ));

        let duplicate_capability =
            registry(vec![provider("alpha", "1.0.0", HOST_API, DUP_CAPABILITY_LIST)]);
        assert!(matches!(
            duplicate_capability,
            Err(ProviderRegistryError::DuplicateCapabilityId { .. })
        ));

        let duplicate_key_in_one_capability = registry(vec![provider(
            "alpha",
            "1.0.0",
            HOST_API,
            DUP_SELECTION_KEY_LIST,
        )]);
        assert!(matches!(
            duplicate_key_in_one_capability,
            Err(ProviderRegistryError::DuplicateSelectionKey { .. })
        ));

        let duplicate_key_across_providers = registry(vec![
            provider("alpha", "1.0.0", HOST_API, PRIMARY_CAPABILITIES),
            provider("beta", "1.0.0", HOST_API, PRIMARY_CAPABILITIES),
        ]);
        assert!(matches!(
            duplicate_key_across_providers,
            Err(ProviderRegistryError::DuplicateSelectionKey { .. })
        ));
    }

    #[test]
    fn selection_keys_are_qualified_by_capability_id() {
        let registry = registry(vec![
            provider("alpha", "1.0.0", HOST_API, PRIMARY_CAPABILITIES),
            provider("beta", "1.0.0", HOST_API, OTHER_CAPABILITIES),
        ])
        .expect("the same key may be reused under a different capability");

        let selected = registry
            .select(&requirement(
                "mods.other",
                ApiVersion::new(1, 0),
                "^1.0.0",
                Some("mod-source:alpha"),
            ))
            .expect("select other capability");
        assert_eq!(selected.descriptor().id, "beta");
    }

    #[test]
    fn selection_uses_semver_capability_api_and_exact_selection_key() {
        const CATALOG_API_1_2: ProviderCapability = ProviderCapability {
            id: "mods.catalog",
            api_version: ApiVersion::new(1, 2),
            selection_keys: &["mod-source:alpha"],
        };
        const CATALOG_API_1_2_LIST: &[ProviderCapability] = &[CATALOG_API_1_2];
        let registry = registry(vec![provider(
            "alpha",
            "1.4.2",
            HOST_API,
            CATALOG_API_1_2_LIST,
        )])
        .expect("valid registry");

        let selected = registry
            .select(&requirement(
                "mods.catalog",
                ApiVersion::new(1, 1),
                "^1.2.0",
                Some("mod-source:alpha"),
            ))
            .expect("compatible requirement");
        assert_eq!(selected.descriptor().id, "alpha");

        let newer_capability_api = registry
            .select(&requirement(
                "mods.catalog",
                ApiVersion::new(1, 3),
                "^1.2.0",
                Some("mod-source:alpha"),
            ))
            .expect_err("requirement API cannot be newer than capability API");
        assert!(matches!(
            newer_capability_api,
            ProviderSelectionError::Unavailable { .. }
        ));

        let newer_provider_version = registry
            .select(&requirement(
                "mods.catalog",
                ApiVersion::new(1, 0),
                "^2.0.0",
                Some("mod-source:alpha"),
            ))
            .expect_err("provider version requirement must match");
        assert!(matches!(
            newer_provider_version,
            ProviderSelectionError::Unavailable { .. }
        ));

        let wrong_key = registry
            .select(&requirement(
                "mods.catalog",
                ApiVersion::new(1, 0),
                "^1.0.0",
                Some("mod-source:beta"),
            ))
            .expect_err("selection key must match exactly");
        assert!(matches!(
            wrong_key,
            ProviderSelectionError::Unavailable { .. }
        ));

        let invalid_requirement = registry
            .select(&requirement(
                "mods.catalog",
                ApiVersion::new(1, 0),
                "not a version requirement",
                None,
            ))
            .expect_err("invalid semver requirement");
        assert!(matches!(
            invalid_requirement,
            ProviderSelectionError::InvalidProviderVersionRequirement { .. }
        ));
    }

    #[test]
    fn selection_fails_closed_for_zero_or_multiple_matches() {
        let no_match = registry(vec![provider(
            "alpha",
            "1.0.0",
            HOST_API,
            PRIMARY_CAPABILITIES,
        )])
        .expect("valid registry");
        let unavailable = no_match
            .select(&requirement(
                "mods.missing",
                ApiVersion::new(1, 0),
                "^1.0.0",
                None,
            ))
            .expect_err("zero matches are unavailable");
        assert!(matches!(
            unavailable,
            ProviderSelectionError::Unavailable { .. }
        ));

        let ambiguous = registry(vec![
            provider("alpha", "1.0.0", HOST_API, UNKEYED_CAPABILITIES),
            provider("beta", "1.0.0", HOST_API, UNKEYED_CAPABILITIES),
        ])
        .expect("valid registry");
        let ambiguous = ambiguous
            .select(&requirement(
                "mods.catalog",
                ApiVersion::new(1, 0),
                "^1.0.0",
                None,
            ))
            .expect_err("multiple matches are ambiguous");
        match ambiguous {
            ProviderSelectionError::Ambiguous { provider_ids, .. } => {
                assert_eq!(provider_ids, vec!["alpha".to_string(), "beta".to_string()]);
            }
            other => panic!("expected ambiguous selection, got {other:?}"),
        }
    }
}
