use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

use semver::{Version, VersionReq};

use crate::{
    ApiVersion, FeatureDescriptor, FeatureId, FeatureInitializer, ProviderRequirement,
    StartupPolicy,
};
use crate::id::validate_namespaced_id;

/// A validated feature descriptor paired with its single registration-time initializer.
pub struct FeatureRegistration {
    descriptor: FeatureDescriptor,
    initializer: Option<Box<dyn FeatureInitializer>>,
}

impl FeatureRegistration {
    pub fn new(
        descriptor: FeatureDescriptor,
        initializer: Box<dyn FeatureInitializer>,
    ) -> Self {
        Self {
            descriptor,
            initializer: Some(initializer),
        }
    }

    pub fn descriptor(&self) -> &FeatureDescriptor {
        &self.descriptor
    }

    pub(crate) fn take_initializer(&mut self) -> Option<Box<dyn FeatureInitializer>> {
        self.initializer.take()
    }
}

impl fmt::Debug for FeatureRegistration {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FeatureRegistration")
            .field("descriptor", &self.descriptor)
            .field("has_initializer", &self.initializer.is_some())
            .finish()
    }
}

/// Metadata/configuration validation failure. Construction never invokes an initializer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FeatureRegistryError {
    DuplicateFeature {
        feature_id: FeatureId,
    },
    SelfDependency {
        feature_id: FeatureId,
    },
    DuplicateDependency {
        feature_id: FeatureId,
        dependency_id: FeatureId,
    },
    UnknownDependency {
        feature_id: FeatureId,
        dependency_id: FeatureId,
    },
    UnmetDependencyVersion {
        feature_id: FeatureId,
        dependency_id: FeatureId,
        required: VersionReq,
        actual: Version,
    },
    IncompatibleFeatureApi {
        feature_id: FeatureId,
        feature_api: ApiVersion,
        host_api: ApiVersion,
    },
    InvalidProviderRequirement {
        feature_id: FeatureId,
        capability: String,
        reason: String,
    },
    DuplicateProviderRequirement {
        feature_id: FeatureId,
        capability: String,
        selection_key: Option<String>,
    },
    ContributionOwnerMismatch {
        feature_id: FeatureId,
        contribution_id: String,
    },
    DuplicateContribution {
        contribution_id: String,
    },
    Cycle {
        features: Vec<FeatureId>,
    },
}

impl fmt::Display for FeatureRegistryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateFeature { feature_id } => {
                write!(formatter, "feature '{feature_id}' is registered more than once")
            }
            Self::SelfDependency { feature_id } => {
                write!(formatter, "feature '{feature_id}' declares itself as a dependency")
            }
            Self::DuplicateDependency {
                feature_id,
                dependency_id,
            } => write!(
                formatter,
                "feature '{feature_id}' declares dependency '{dependency_id}' more than once"
            ),
            Self::UnknownDependency {
                feature_id,
                dependency_id,
            } => write!(
                formatter,
                "feature '{feature_id}' requires unknown feature '{dependency_id}'"
            ),
            Self::UnmetDependencyVersion {
                feature_id,
                dependency_id,
                required,
                actual,
            } => write!(
                formatter,
                "feature '{feature_id}' requires '{dependency_id}' version '{required}', but the registered feature version is '{actual}'"
            ),
            Self::IncompatibleFeatureApi {
                feature_id,
                feature_api,
                host_api,
            } => write!(
                formatter,
                "feature '{feature_id}' API {feature_api} is incompatible with host feature API {host_api}; the major versions must match and the feature minor must not be newer"
            ),
            Self::InvalidProviderRequirement {
                feature_id,
                capability,
                reason,
            } => write!(
                formatter,
                "feature '{feature_id}' declares invalid provider requirement '{capability}': {reason}"
            ),
            Self::DuplicateProviderRequirement {
                feature_id,
                capability,
                selection_key,
            } => {
                let selection = selection_key
                    .as_deref()
                    .map(|key| format!(" with selection key '{key}'"))
                    .unwrap_or_default();
                write!(
                    formatter,
                    "feature '{feature_id}' declares provider capability '{capability}'{selection} more than once"
                )
            }
            Self::ContributionOwnerMismatch {
                feature_id,
                contribution_id,
            } => write!(
                formatter,
                "feature '{feature_id}' declares contribution '{contribution_id}' with a different owning feature prefix"
            ),
            Self::DuplicateContribution { contribution_id } => write!(
                formatter,
                "backend contribution ID '{contribution_id}' is registered more than once"
            ),
            Self::Cycle { features } => {
                let path = features
                    .iter()
                    .map(FeatureId::as_str)
                    .collect::<Vec<_>>()
                    .join(" -> ");
                write!(formatter, "feature dependency cycle: {path}")
            }
        }
    }
}

impl std::error::Error for FeatureRegistryError {}

/// Immutable, validated feature metadata and deterministic dependency ordering.
pub struct FeatureRegistry {
    host_api: ApiVersion,
    registrations: BTreeMap<FeatureId, FeatureRegistration>,
    descriptors: BTreeMap<FeatureId, FeatureDescriptor>,
    dependencies: BTreeMap<FeatureId, Vec<FeatureId>>,
    dependents: BTreeMap<FeatureId, Vec<FeatureId>>,
    topological_order: Vec<FeatureId>,
}

impl FeatureRegistry {
    pub fn new(
        host_api: ApiVersion,
        registrations: Vec<FeatureRegistration>,
    ) -> Result<Self, FeatureRegistryError> {
        let mut registration_map = BTreeMap::new();
        for registration in registrations {
            let feature_id = registration.descriptor().id.clone();
            if registration_map
                .insert(feature_id.clone(), registration)
                .is_some()
            {
                return Err(FeatureRegistryError::DuplicateFeature { feature_id });
            }
        }

        let descriptors = registration_map
            .iter()
            .map(|(feature_id, registration)| {
                (feature_id.clone(), registration.descriptor().clone())
            })
            .collect::<BTreeMap<_, _>>();

        for (feature_id, descriptor) in &descriptors {
            if !descriptor.is_api_compatible_with(host_api) {
                return Err(FeatureRegistryError::IncompatibleFeatureApi {
                    feature_id: feature_id.clone(),
                    feature_api: descriptor.api_version,
                    host_api,
                });
            }

            let mut dependency_ids = BTreeSet::new();
            for requirement in &descriptor.requires {
                if &requirement.id == feature_id {
                    return Err(FeatureRegistryError::SelfDependency {
                        feature_id: feature_id.clone(),
                    });
                }
                if !dependency_ids.insert(requirement.id.clone()) {
                    return Err(FeatureRegistryError::DuplicateDependency {
                        feature_id: feature_id.clone(),
                        dependency_id: requirement.id.clone(),
                    });
                }
                let dependency = descriptors.get(&requirement.id).ok_or_else(|| {
                    FeatureRegistryError::UnknownDependency {
                        feature_id: feature_id.clone(),
                        dependency_id: requirement.id.clone(),
                    }
                })?;
                if !requirement.version_req.matches(&dependency.feature_version) {
                    return Err(FeatureRegistryError::UnmetDependencyVersion {
                        feature_id: feature_id.clone(),
                        dependency_id: requirement.id.clone(),
                        required: requirement.version_req.clone(),
                        actual: dependency.feature_version.clone(),
                    });
                }
            }

            let mut provider_requirements = BTreeSet::new();
            for requirement in &descriptor.provider_requirements {
                validate_namespaced_id(requirement.capability, 2).map_err(|error| {
                    FeatureRegistryError::InvalidProviderRequirement {
                        feature_id: feature_id.clone(),
                        capability: requirement.capability.to_string(),
                        reason: error.to_string(),
                    }
                })?;
                VersionReq::parse(requirement.provider_version_req).map_err(|error| {
                    FeatureRegistryError::InvalidProviderRequirement {
                        feature_id: feature_id.clone(),
                        capability: requirement.capability.to_string(),
                        reason: format!("provider version requirement: {error}"),
                    }
                })?;
                if let Some(selection_key) = requirement.selection_key {
                    if !valid_selection_key(selection_key) {
                        return Err(FeatureRegistryError::InvalidProviderRequirement {
                            feature_id: feature_id.clone(),
                            capability: requirement.capability.to_string(),
                            reason: "selection key is not a stable lowercase identifier"
                                .to_string(),
                        });
                    }
                }
                let requirement_key = (
                    requirement.capability.to_string(),
                    requirement.selection_key.map(str::to_string),
                );
                if !provider_requirements.insert(requirement_key) {
                    return Err(FeatureRegistryError::DuplicateProviderRequirement {
                        feature_id: feature_id.clone(),
                        capability: requirement.capability.to_string(),
                        selection_key: requirement.selection_key.map(str::to_string),
                    });
                }
            }
        }

        let mut contribution_ids = BTreeSet::new();
        for descriptor in descriptors.values() {
            for contribution in &descriptor.contributions {
                if contribution.id.owner() != &descriptor.id {
                    return Err(FeatureRegistryError::ContributionOwnerMismatch {
                        feature_id: descriptor.id.clone(),
                        contribution_id: contribution.id.as_str().to_string(),
                    });
                }
                if !contribution_ids.insert(contribution.id.as_str().to_string()) {
                    return Err(FeatureRegistryError::DuplicateContribution {
                        contribution_id: contribution.id.as_str().to_string(),
                    });
                }
            }
        }

        let mut dependencies = BTreeMap::new();
        for (feature_id, descriptor) in &descriptors {
            let mut dependency_ids = descriptor
                .requires
                .iter()
                .map(|requirement| requirement.id.clone())
                .collect::<Vec<_>>();
            dependency_ids.sort();
            dependencies.insert(feature_id.clone(), dependency_ids);
        }

        let mut dependents = descriptors
            .keys()
            .map(|feature_id| (feature_id.clone(), Vec::new()))
            .collect::<BTreeMap<_, _>>();
        for (feature_id, dependency_ids) in &dependencies {
            for dependency_id in dependency_ids {
                if let Some(values) = dependents.get_mut(dependency_id) {
                    values.push(feature_id.clone());
                }
            }
        }
        for values in dependents.values_mut() {
            values.sort();
        }

        let topological_order = deterministic_topological_order(&dependencies, &dependents)
            .map_err(|features| FeatureRegistryError::Cycle { features })?;

        Ok(Self {
            host_api,
            registrations: registration_map,
            descriptors,
            dependencies,
            dependents,
            topological_order,
        })
    }

    pub fn host_api(&self) -> ApiVersion {
        self.host_api
    }

    pub fn topological_order(&self) -> &[FeatureId] {
        &self.topological_order
    }

    pub fn descriptor(&self, feature_id: &FeatureId) -> Option<&FeatureDescriptor> {
        self.descriptors.get(feature_id)
    }

    pub fn descriptors(&self) -> impl ExactSizeIterator<Item = &FeatureDescriptor> + '_ {
        self.topological_order.iter().map(move |feature_id| {
            self.descriptors
                .get(feature_id)
                .expect("topological feature IDs always have descriptors")
        })
    }

    pub fn descriptors_in_id_order(&self) -> impl Iterator<Item = &FeatureDescriptor> + '_ {
        self.descriptors.values()
    }

    pub fn dependencies_of(&self, feature_id: &FeatureId) -> Option<&[FeatureId]> {
        self.dependencies.get(feature_id).map(Vec::as_slice)
    }

    pub fn dependents_of(&self, feature_id: &FeatureId) -> Option<&[FeatureId]> {
        self.dependents.get(feature_id).map(Vec::as_slice)
    }

    pub fn len(&self) -> usize {
        self.descriptors.len()
    }

    pub fn is_empty(&self) -> bool {
        self.descriptors.is_empty()
    }

    pub(crate) fn take_initializer(
        &mut self,
        feature_id: &FeatureId,
    ) -> Option<Box<dyn FeatureInitializer>> {
        self.registrations
            .get_mut(feature_id)
            .and_then(FeatureRegistration::take_initializer)
    }
}

impl fmt::Debug for FeatureRegistry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FeatureRegistry")
            .field("host_api", &self.host_api)
            .field("topological_order", &self.topological_order)
            .field("descriptor_count", &self.descriptors.len())
            .finish()
    }
}

fn valid_selection_key(value: &str) -> bool {
    let bytes = value.as_bytes();
    let Some(first) = bytes.first().copied() else {
        return false;
    };
    let Some(last) = bytes.last().copied() else {
        return false;
    };
    if !first.is_ascii_lowercase() || !last.is_ascii_alphanumeric() {
        return false;
    }
    if bytes.iter().copied().any(|byte| {
        !(byte.is_ascii_lowercase()
            || byte.is_ascii_digit()
            || matches!(byte, b'.' | b'-' | b'_' | b':'))
    }) {
        return false;
    }
    !bytes
        .windows(2)
        .any(|pair| is_separator(pair[0]) && is_separator(pair[1]))
}

fn is_separator(byte: u8) -> bool {
    matches!(byte, b'.' | b'-' | b'_' | b':')
}

fn deterministic_topological_order(
    dependencies: &BTreeMap<FeatureId, Vec<FeatureId>>,
    dependents: &BTreeMap<FeatureId, Vec<FeatureId>>,
) -> Result<Vec<FeatureId>, Vec<FeatureId>> {
    let mut indegree = dependencies
        .iter()
        .map(|(feature_id, requirements)| (feature_id.clone(), requirements.len()))
        .collect::<BTreeMap<_, _>>();
    let mut ready = indegree
        .iter()
        .filter(|(_, count)| **count == 0)
        .map(|(feature_id, _)| feature_id.clone())
        .collect::<BTreeSet<_>>();
    let mut order = Vec::with_capacity(dependencies.len());
    let mut visited = BTreeSet::new();

    while let Some(feature_id) = ready.iter().next().cloned() {
        ready.remove(&feature_id);
        visited.insert(feature_id.clone());
        order.push(feature_id.clone());

        if let Some(dependent_ids) = dependents.get(&feature_id) {
            for dependent_id in dependent_ids {
                if let Some(count) = indegree.get_mut(dependent_id) {
                    *count -= 1;
                    if *count == 0 {
                        ready.insert(dependent_id.clone());
                    }
                }
            }
        }
    }

    if order.len() == dependencies.len() {
        return Ok(order);
    }

    let unresolved = dependencies
        .keys()
        .filter(|feature_id| !visited.contains(*feature_id))
        .cloned()
        .collect::<BTreeSet<_>>();
    let cycle = find_cycle(dependencies, &unresolved)
        .unwrap_or_else(|| unresolved.into_iter().collect::<Vec<_>>());
    Err(cycle)
}

fn find_cycle(
    dependencies: &BTreeMap<FeatureId, Vec<FeatureId>>,
    unresolved: &BTreeSet<FeatureId>,
) -> Option<Vec<FeatureId>> {
    let mut states = BTreeMap::<FeatureId, u8>::new();
    let mut stack = Vec::new();

    for feature_id in unresolved {
        if states.get(feature_id).copied().unwrap_or(0) == 0 {
            if let Some(cycle) = visit_cycle(
                feature_id,
                dependencies,
                unresolved,
                &mut states,
                &mut stack,
            ) {
                return Some(cycle);
            }
        }
    }
    None
}

fn visit_cycle(
    feature_id: &FeatureId,
    dependencies: &BTreeMap<FeatureId, Vec<FeatureId>>,
    unresolved: &BTreeSet<FeatureId>,
    states: &mut BTreeMap<FeatureId, u8>,
    stack: &mut Vec<FeatureId>,
) -> Option<Vec<FeatureId>> {
    states.insert(feature_id.clone(), 1);
    stack.push(feature_id.clone());

    if let Some(dependency_ids) = dependencies.get(feature_id) {
        for dependency_id in dependency_ids {
            if !unresolved.contains(dependency_id) {
                continue;
            }
            match states.get(dependency_id).copied().unwrap_or(0) {
                0 => {
                    if let Some(cycle) = visit_cycle(
                        dependency_id,
                        dependencies,
                        unresolved,
                        states,
                        stack,
                    ) {
                        return Some(cycle);
                    }
                }
                1 => {
                    let start = stack
                        .iter()
                        .position(|candidate| candidate == dependency_id)
                        .unwrap_or(0);
                    let mut cycle = stack[start..].to_vec();
                    cycle.push(dependency_id.clone());
                    return Some(cycle);
                }
                _ => {}
            }
        }
    }

    stack.pop();
    states.insert(feature_id.clone(), 2);
    None
}

#[cfg(test)]
mod tests {
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };

    use semver::{Version, VersionReq};

    use super::*;
    use crate::{
        ContributionDescriptor, ContributionId, ContributionKind, FeatureHealth,
        FeatureInitError, FeatureRequirement, FeatureRuntime,
    };

    struct NoopRuntime;

    impl FeatureRuntime for NoopRuntime {
        fn health(&self) -> FeatureHealth {
            FeatureHealth::Healthy
        }

        fn shutdown(&mut self) -> Result<(), String> {
            Ok(())
        }
    }

    struct NoopInitializer;

    impl FeatureInitializer for NoopInitializer {
        fn initialize(
            &self,
            _context: &crate::FeatureInitContext,
        ) -> Result<Box<dyn FeatureRuntime>, FeatureInitError> {
            Ok(Box::new(NoopRuntime))
        }
    }

    struct CountingInitializer(Arc<AtomicUsize>);

    impl FeatureInitializer for CountingInitializer {
        fn initialize(
            &self,
            _context: &crate::FeatureInitContext,
        ) -> Result<Box<dyn FeatureRuntime>, FeatureInitError> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(Box::new(NoopRuntime))
        }
    }

    fn feature_id(value: &str) -> FeatureId {
        FeatureId::new(value).unwrap()
    }

    fn requirement(feature_id: &str, version_req: &str) -> FeatureRequirement {
        FeatureRequirement::new(
            self::feature_id(feature_id),
            VersionReq::parse(version_req).unwrap(),
        )
    }

    fn provider_requirement(capability: &'static str) -> ProviderRequirement {
        ProviderRequirement {
            capability,
            api_version: ApiVersion::new(1, 0),
            provider_version_req: "^1.0.0",
            selection_key: Some("mod-source:default"),
        }
    }

    fn descriptor(feature_id: &str) -> FeatureDescriptor {
        FeatureDescriptor {
            id: self::feature_id(feature_id),
            feature_version: Version::new(1, 0, 0),
            api_version: ApiVersion::new(1, 0),
            startup_policy: StartupPolicy::Optional,
            requires: Vec::new(),
            provider_requirements: Vec::new(),
            contributions: Vec::new(),
        }
    }

    fn registration(descriptor: FeatureDescriptor) -> FeatureRegistration {
        FeatureRegistration::new(descriptor, Box::new(NoopInitializer))
    }

    #[test]
    fn topological_order_is_deterministic_with_lexicographic_ready_ties() {
        let mut alpha = descriptor("flint.alpha");
        alpha.requires = vec![requirement("flint.zeta", "^1.0.0")];

        let registry = FeatureRegistry::new(
            ApiVersion::new(1, 0),
            vec![
                registration(descriptor("flint.zeta")),
                registration(alpha),
                registration(descriptor("flint.gamma")),
                registration(descriptor("flint.beta")),
            ],
        )
        .expect("valid feature graph");

        assert_eq!(
            registry
                .topological_order()
                .iter()
                .map(FeatureId::as_str)
                .collect::<Vec<_>>(),
            vec!["flint.beta", "flint.gamma", "flint.zeta", "flint.alpha"]
        );
        assert_eq!(
            registry
                .dependencies_of(&feature_id("flint.alpha"))
                .unwrap()
                .iter()
                .map(FeatureId::as_str)
                .collect::<Vec<_>>(),
            vec!["flint.zeta"]
        );
        assert_eq!(
            registry
                .dependents_of(&feature_id("flint.zeta"))
                .unwrap()
                .iter()
                .map(FeatureId::as_str)
                .collect::<Vec<_>>(),
            vec!["flint.alpha"]
        );
    }

    #[test]
    fn cycle_errors_are_deterministic_and_include_the_edge_path() {
        let mut alpha = descriptor("flint.alpha");
        alpha.requires = vec![requirement("flint.beta", "^1.0.0")];
        let mut beta = descriptor("flint.beta");
        beta.requires = vec![requirement("flint.gamma", "^1.0.0")];
        let mut gamma = descriptor("flint.gamma");
        gamma.requires = vec![requirement("flint.alpha", "^1.0.0")];

        let error = FeatureRegistry::new(
            ApiVersion::new(1, 0),
            vec![registration(gamma), registration(alpha), registration(beta)],
        )
        .expect_err("cycle must fail");

        match error {
            FeatureRegistryError::Cycle { features } => assert_eq!(
                features.iter().map(FeatureId::as_str).collect::<Vec<_>>(),
                vec!["flint.alpha", "flint.beta", "flint.gamma", "flint.alpha"]
            ),
            other => panic!("expected cycle error, got {other:?}"),
        }
    }

    #[test]
    fn duplicate_self_unknown_and_unmet_feature_requirements_are_rejected() {
        let duplicate_feature = FeatureRegistry::new(
            ApiVersion::new(1, 0),
            vec![
                registration(descriptor("flint.alpha")),
                registration(descriptor("flint.alpha")),
            ],
        );
        assert!(matches!(
            duplicate_feature,
            Err(FeatureRegistryError::DuplicateFeature { .. })
        ));

        let mut self_dependency = descriptor("flint.alpha");
        self_dependency.requires = vec![requirement("flint.alpha", "^1.0.0")];
        let self_error = FeatureRegistry::new(
            ApiVersion::new(1, 0),
            vec![registration(self_dependency)],
        );
        assert!(matches!(
            self_error,
            Err(FeatureRegistryError::SelfDependency { .. })
        ));

        let mut duplicate_dependency = descriptor("flint.alpha");
        duplicate_dependency.requires = vec![
            requirement("flint.beta", "^1.0.0"),
            requirement("flint.beta", "^1.0.0"),
        ];
        let duplicate_error = FeatureRegistry::new(
            ApiVersion::new(1, 0),
            vec![
                registration(descriptor("flint.beta")),
                registration(duplicate_dependency),
            ],
        );
        assert!(matches!(
            duplicate_error,
            Err(FeatureRegistryError::DuplicateDependency { .. })
        ));

        let mut unknown_dependency = descriptor("flint.alpha");
        unknown_dependency.requires = vec![requirement("flint.missing", "^1.0.0")];
        let unknown_error = FeatureRegistry::new(
            ApiVersion::new(1, 0),
            vec![registration(unknown_dependency)],
        );
        assert!(matches!(
            unknown_error,
            Err(FeatureRegistryError::UnknownDependency { .. })
        ));

        let mut unmet_dependency = descriptor("flint.alpha");
        unmet_dependency.requires = vec![requirement("flint.beta", "^2.0.0")];
        let unmet_error = FeatureRegistry::new(
            ApiVersion::new(1, 0),
            vec![
                registration(descriptor("flint.beta")),
                registration(unmet_dependency),
            ],
        );
        assert!(matches!(
            unmet_error,
            Err(FeatureRegistryError::UnmetDependencyVersion { .. })
        ));
    }

    #[test]
    fn incompatible_api_and_malformed_provider_requirements_are_rejected() {
        let mut incompatible = descriptor("flint.alpha");
        incompatible.api_version = ApiVersion::new(1, 1);
        let api_error = FeatureRegistry::new(ApiVersion::new(1, 0), vec![registration(incompatible)]);
        assert!(matches!(
            api_error,
            Err(FeatureRegistryError::IncompatibleFeatureApi { .. })
        ));

        let mut malformed = descriptor("flint.alpha");
        malformed.provider_requirements = vec![ProviderRequirement {
            capability: "mods.catalog",
            api_version: ApiVersion::new(1, 0),
            provider_version_req: "not a version requirement",
            selection_key: None,
        }];
        let malformed_error =
            FeatureRegistry::new(ApiVersion::new(1, 0), vec![registration(malformed)]);
        assert!(matches!(
            malformed_error,
            Err(FeatureRegistryError::InvalidProviderRequirement { .. })
        ));

        let mut malformed_capability = descriptor("flint.alpha");
        malformed_capability.provider_requirements = vec![ProviderRequirement {
            capability: "mods",
            api_version: ApiVersion::new(1, 0),
            provider_version_req: "^1.0.0",
            selection_key: None,
        }];
        let capability_error = FeatureRegistry::new(
            ApiVersion::new(1, 0),
            vec![registration(malformed_capability)],
        );
        assert!(matches!(
            capability_error,
            Err(FeatureRegistryError::InvalidProviderRequirement { .. })
        ));

        let mut malformed_selection_key = descriptor("flint.alpha");
        malformed_selection_key.provider_requirements = vec![ProviderRequirement {
            capability: "mods.catalog",
            api_version: ApiVersion::new(1, 0),
            provider_version_req: "^1.0.0",
            selection_key: Some("Bad selection key"),
        }];
        let selection_error = FeatureRegistry::new(
            ApiVersion::new(1, 0),
            vec![registration(malformed_selection_key)],
        );
        assert!(matches!(
            selection_error,
            Err(FeatureRegistryError::InvalidProviderRequirement { .. })
        ));

        let mut duplicate_provider = descriptor("flint.alpha");
        duplicate_provider.provider_requirements = vec![
            provider_requirement("mods.catalog"),
            provider_requirement("mods.catalog"),
        ];
        let duplicate_provider_error =
            FeatureRegistry::new(ApiVersion::new(1, 0), vec![registration(duplicate_provider)]);
        assert!(matches!(
            duplicate_provider_error,
            Err(FeatureRegistryError::DuplicateProviderRequirement { .. })
        ));
    }

    #[test]
    fn duplicate_and_misowned_contributions_are_rejected() {
        let alpha = feature_id("flint.alpha");
        let beta = feature_id("flint.beta");
        let contribution = ContributionId::new(&alpha, "flint.alpha.runtime").unwrap();

        let mut duplicate = descriptor("flint.alpha");
        duplicate.contributions = vec![
            ContributionDescriptor::new(contribution.clone(), ContributionKind::Service),
            ContributionDescriptor::new(contribution, ContributionKind::Event),
        ];
        let duplicate_error =
            FeatureRegistry::new(ApiVersion::new(1, 0), vec![registration(duplicate)]);
        assert!(matches!(
            duplicate_error,
            Err(FeatureRegistryError::DuplicateContribution { .. })
        ));

        let mut misowned = descriptor("flint.alpha");
        misowned.contributions = vec![ContributionDescriptor::new(
            ContributionId::new(&beta, "flint.beta.runtime").unwrap(),
            ContributionKind::Capability,
        )];
        let owner_error =
            FeatureRegistry::new(ApiVersion::new(1, 0), vec![registration(misowned)]);
        assert!(matches!(
            owner_error,
            Err(FeatureRegistryError::ContributionOwnerMismatch { .. })
        ));
    }

    #[test]
    fn registry_validation_never_invokes_initializers() {
        let calls = Arc::new(AtomicUsize::new(0));
        let mutate_descriptor = descriptor("flint.alpha");
        let registry = FeatureRegistry::new(
            ApiVersion::new(1, 0),
            vec![FeatureRegistration::new(
                mutate_descriptor,
                Box::new(CountingInitializer(calls.clone())),
            )],
        )
        .expect("valid metadata");
        assert_eq!(registry.len(), 1);
        assert_eq!(calls.load(Ordering::SeqCst), 0);

        let mut cyclic_alpha = descriptor("flint.alpha");
        cyclic_alpha.requires = vec![requirement("flint.beta", "^1.0.0")];
        let mut cyclic_beta = descriptor("flint.beta");
        cyclic_beta.requires = vec![requirement("flint.alpha", "^1.0.0")];
        let cyclic = FeatureRegistry::new(
            ApiVersion::new(1, 0),
            vec![
                FeatureRegistration::new(
                    cyclic_alpha,
                    Box::new(CountingInitializer(calls.clone())),
                ),
                FeatureRegistration::new(
                    cyclic_beta,
                    Box::new(CountingInitializer(calls.clone())),
                ),
            ],
        );
        assert!(cyclic.is_err());
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }
}