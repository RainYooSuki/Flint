use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
};

use crate::{
    FeatureAvailability, FeatureAvailabilitySnapshot, FeatureDescriptor, FeatureHealth, FeatureId,
    FeatureInitContext, FeatureRegistry, FeatureState, HostState, ProviderRequirement,
};

/// Provider selection state supplied by the host before or during feature startup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderAvailability {
    Available,
    Unavailable { reason: String },
    Ambiguous { provider_ids: Vec<String> },
}

/// Host-owned provider lifecycle/catalog abstraction. No dependency on a concrete provider crate.
///
/// IDs returned by `provider_ids` are independent leaves. The coordinator initializes them in
/// lexicographic order and never constructs a provider dependency graph.
pub trait ProviderCatalog: Send {
    /// All configured provider leaf IDs. Duplicate or malformed IDs fail startup before side effects.
    fn provider_ids(&self) -> Vec<String>;
    /// Apply the full host provider requirement predicate and report zero/one/multiple matches.
    fn availability(&self, requirement: &ProviderRequirement) -> ProviderAvailability;
    /// Initialize one provider. A failed initialization is marked unavailable and never retried;
    /// the failing provider must clean its own partial resources.
    fn initialize(&mut self, provider_id: &str) -> Result<(), String>;
    /// Stop one successfully initialized provider in reverse initialization order.
    fn shutdown(&mut self, provider_id: &str) -> Result<(), String>;
    /// Mark a provider unavailable so subsequent availability checks fail closed. This must be
    /// idempotent because the coordinator also marks providers after a failed initialization.
    fn mark_unavailable(&mut self, provider_id: &str, reason: &str);
}

/// Internal provider lifecycle state. Provider identities are deliberately excluded from snapshots.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProviderState {
    Initializing,
    Available,
    Failed,
    Stopped,
}

/// Callback used to release shared core/bootstrap resources last.
pub type CoreCleanup = Box<dyn FnMut() -> Result<(), String> + Send>;

/// Outcome of the one allowed startup attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartupOutcome {
    Ready,
    Degraded,
    StartupFailed,
}

/// Consequence of a runtime failure for the host lifecycle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FailureImpact {
    None,
    Degraded { affected: Vec<FeatureId> },
    Fatal { affected: Vec<FeatureId> },
}

impl FailureImpact {
    pub fn affected(&self) -> &[FeatureId] {
        match self {
            Self::None => &[],
            Self::Degraded { affected } | Self::Fatal { affected } => affected,
        }
    }
}

/// Stable, sanitized feature-unavailable response for command/API adapters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeatureUnavailable {
    feature_id: FeatureId,
    reason: String,
}

impl FeatureUnavailable {
    pub fn new(feature_id: FeatureId, reason: impl Into<String>) -> Self {
        Self {
            feature_id,
            reason: sanitize_reason(&reason.into()),
        }
    }

    pub fn feature_id(&self) -> &FeatureId {
        &self.feature_id
    }

    pub fn reason(&self) -> &str {
        &self.reason
    }
}

impl fmt::Display for FeatureUnavailable {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "feature '{}' is unavailable: {}",
            self.feature_id, self.reason
        )
    }
}

impl Error for FeatureUnavailable {}

/// Invalid lifecycle operation or feature/provider lookup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LifecycleError {
    AlreadyStarted,
    NotRunning { state: HostState },
    UnknownFeature { feature_id: FeatureId },
    FeatureNotReady {
        feature_id: FeatureId,
        state: FeatureState,
    },
    UnknownProvider { provider_id: String },
    ProviderNotAvailable { provider_id: String },
}

impl fmt::Display for LifecycleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadyStarted => {
                write!(formatter, "lifecycle coordinator startup may be attempted only once")
            }
            Self::NotRunning { state } => {
                write!(formatter, "lifecycle operation requires a running host, not '{}'", state.as_str())
            }
            Self::UnknownFeature { feature_id } => {
                write!(formatter, "feature '{feature_id}' is not registered")
            }
            Self::FeatureNotReady { feature_id, state } => write!(
                formatter,
                "feature '{feature_id}' is not ready for this operation (state '{}')",
                state.as_str()
            ),
            Self::UnknownProvider { provider_id } => {
                write!(formatter, "provider '{provider_id}' is not known to the lifecycle coordinator")
            }
            Self::ProviderNotAvailable { provider_id } => {
                write!(formatter, "provider '{provider_id}' is not available")
            }
        }
    }
}

impl Error for LifecycleError {}

/// Owns deterministic startup, availability gating, failure propagation, and reverse shutdown.
pub struct LifecycleCoordinator {
    registry: FeatureRegistry,
    providers: Box<dyn ProviderCatalog>,
    core_cleanup: Option<CoreCleanup>,
    provider_order: Vec<String>,
    provider_states: BTreeMap<String, ProviderState>,
    initialized_providers: Vec<String>,
    initialized_features: Vec<FeatureId>,
    feature_states: BTreeMap<FeatureId, FeatureState>,
    feature_reasons: BTreeMap<FeatureId, String>,
    runtimes: BTreeMap<FeatureId, Box<dyn crate::FeatureRuntime>>,
    active_contributions: BTreeSet<String>,
    host_state: HostState,
    state_history: Vec<HostState>,
    revision: u64,
    last_failure: Option<String>,
    diagnostics: Vec<String>,
    startup_attempted: bool,
    core_cleanup_ran: bool,
}

impl LifecycleCoordinator {
    pub fn new(
        registry: FeatureRegistry,
        providers: Box<dyn ProviderCatalog>,
        core_cleanup: Option<CoreCleanup>,
    ) -> Self {
        let feature_states = registry
            .topological_order()
            .iter()
            .map(|feature_id| (feature_id.clone(), FeatureState::Starting))
            .collect();
        Self {
            registry,
            providers,
            core_cleanup,
            provider_order: Vec::new(),
            provider_states: BTreeMap::new(),
            initialized_providers: Vec::new(),
            initialized_features: Vec::new(),
            feature_states,
            feature_reasons: BTreeMap::new(),
            runtimes: BTreeMap::new(),
            active_contributions: BTreeSet::new(),
            host_state: HostState::Starting,
            state_history: vec![HostState::Starting],
            revision: 0,
            last_failure: None,
            diagnostics: Vec::new(),
            startup_attempted: false,
            core_cleanup_ran: false,
        }
    }

    /// Initialize providers and features exactly once in deterministic order.
    pub fn start(&mut self) -> Result<StartupOutcome, LifecycleError> {
        if self.startup_attempted {
            return Err(LifecycleError::AlreadyStarted);
        }
        self.startup_attempted = true;

        let mut provider_ids = BTreeSet::new();
        for provider_id in self.providers.provider_ids() {
            if stable_provider_id_error(&provider_id).is_some() {
                self.finish_startup_failure(
                    None,
                    "provider catalog contains an invalid provider ID",
                );
                return Ok(StartupOutcome::StartupFailed);
            }
            if !provider_ids.insert(provider_id) {
                self.finish_startup_failure(
                    None,
                    "provider catalog contains a duplicate provider ID",
                );
                return Ok(StartupOutcome::StartupFailed);
            }
        }
        self.provider_order = provider_ids.into_iter().collect();

        for provider_id in self.provider_order.clone() {
            self.provider_states
                .insert(provider_id.clone(), ProviderState::Initializing);
            match self.providers.initialize(&provider_id) {
                Ok(()) => {
                    self.provider_states
                        .insert(provider_id.clone(), ProviderState::Available);
                    self.initialized_providers.push(provider_id);
                }
                Err(_) => {
                    self.providers.mark_unavailable(
                        &provider_id,
                        "provider initialization failed",
                    );
                    self.provider_states
                        .insert(provider_id, ProviderState::Failed);
                }
            }
        }

        let feature_order = self.registry.topological_order().to_vec();
        for feature_id in feature_order {
            if self.feature_states.get(&feature_id).copied() != Some(FeatureState::Starting) {
                continue;
            }

            let descriptor = match self.registry.descriptor(&feature_id).cloned() {
                Some(descriptor) => descriptor,
                None => {
                    self.finish_startup_failure(
                        Some(&feature_id),
                        "feature metadata is missing during startup",
                    );
                    return Ok(StartupOutcome::StartupFailed);
                }
            };

            let dependency_not_ready = descriptor.requires.iter().any(|requirement| {
                self.feature_states.get(&requirement.id).copied()
                    != Some(FeatureState::Ready)
            });
            if dependency_not_ready {
                let reason = "a required feature dependency is not ready";
                let affected = self.apply_failure_roots(
                    std::slice::from_ref(&feature_id),
                    FeatureState::BlockedDependency,
                    reason,
                );
                self.record_failure(Some(&feature_id), reason);
                if self.affected_contains_required(&affected) {
                    self.finish_startup_failure(Some(&feature_id), reason);
                    return Ok(StartupOutcome::StartupFailed);
                }
                continue;
            }

            if !self.provider_requirements_available(&descriptor) {
                let reason = "a required provider is unavailable";
                let affected = self.apply_failure_roots(
                    std::slice::from_ref(&feature_id),
                    FeatureState::BlockedDependency,
                    reason,
                );
                self.record_failure(Some(&feature_id), reason);
                if self.affected_contains_required(&affected) {
                    self.finish_startup_failure(Some(&feature_id), reason);
                    return Ok(StartupOutcome::StartupFailed);
                }
                continue;
            }

            let initializer = match self.registry.take_initializer(&feature_id) {
                Some(initializer) => initializer,
                None => {
                    let reason = "feature initializer is missing";
                    let affected = self.apply_failure_roots(
                        std::slice::from_ref(&feature_id),
                        FeatureState::Failed,
                        reason,
                    );
                    self.record_failure(Some(&feature_id), reason);
                    if self.affected_contains_required(&affected) {
                        self.finish_startup_failure(Some(&feature_id), reason);
                        return Ok(StartupOutcome::StartupFailed);
                    }
                    continue;
                }
            };

            let context =
                FeatureInitContext::new(feature_id.clone(), self.registry.host_api());
            match initializer.initialize(&context) {
                Ok(runtime) => {
                    self.runtimes.insert(feature_id.clone(), runtime);
                    self.initialized_features.push(feature_id.clone());
                    self.activate_contributions(&descriptor);
                    self.set_feature_state(&feature_id, FeatureState::Ready, None);
                }
                Err(error) => {
                    let reason = sanitize_reason(error.reason());
                    let affected = self.apply_failure_roots(
                        std::slice::from_ref(&feature_id),
                        FeatureState::Failed,
                        &reason,
                    );
                    self.record_failure(Some(&feature_id), &reason);
                    if self.affected_contains_required(&affected) {
                        self.finish_startup_failure(Some(&feature_id), &reason);
                        return Ok(StartupOutcome::StartupFailed);
                    }
                }
            }
        }

        if let Some(violation) = self.required_ready_violation() {
            let reason = "a required feature dependency or provider is unavailable";
            self.apply_failure_roots(
                std::slice::from_ref(&violation),
                FeatureState::Failed,
                reason,
            );
            self.finish_startup_failure(Some(&violation), reason);
            return Ok(StartupOutcome::StartupFailed);
        }

        let required_unready = self
            .registry
            .descriptors()
            .find(|descriptor| {
                descriptor.startup_policy == crate::StartupPolicy::Required
                    && self.feature_states.get(&descriptor.id).copied()
                        != Some(FeatureState::Ready)
            })
            .map(|descriptor| descriptor.id.clone());

        if let Some(required_unready) = required_unready {
            let reason = "a required feature is not ready";
            self.apply_failure_roots(
                std::slice::from_ref(&required_unready),
                FeatureState::Failed,
                reason,
            );
            self.finish_startup_failure(Some(&required_unready), reason);
            return Ok(StartupOutcome::StartupFailed);
        }

        if self.all_features_ready() {
            self.set_host_state(HostState::Ready);
            Ok(StartupOutcome::Ready)
        } else {
            self.set_host_state(HostState::Degraded);
            Ok(StartupOutcome::Degraded)
        }
    }

    /// Report a runtime feature failure and apply reverse-dependency propagation.
    pub fn fail_feature(
        &mut self,
        feature_id: &FeatureId,
        reason: impl Into<String>,
    ) -> Result<FailureImpact, LifecycleError> {
        self.require_running()?;
        let state = match self.feature_states.get(feature_id).copied() {
            Some(state) => state,
            None => {
                return Err(LifecycleError::UnknownFeature {
                    feature_id: feature_id.clone(),
                })
            }
        };
        if state != FeatureState::Ready {
            return Err(LifecycleError::FeatureNotReady {
                feature_id: feature_id.clone(),
                state,
            });
        }

        let raw_reason: String = reason.into();
        let reason = sanitize_reason(&raw_reason);
        let affected = self.apply_failure_roots(
            std::slice::from_ref(feature_id),
            FeatureState::Failed,
            &reason,
        );
        Ok(self.finalize_runtime_impact(Some(feature_id), &reason, affected))
    }

    /// Report runtime loss of a provider and fail every feature whose requirement is no longer available.
    pub fn fail_provider(
        &mut self,
        provider_id: &str,
        reason: impl Into<String>,
    ) -> Result<FailureImpact, LifecycleError> {
        self.require_running()?;
        if !self
            .provider_order
            .iter()
            .any(|known| known.as_str() == provider_id)
        {
            return Err(LifecycleError::UnknownProvider {
                provider_id: provider_id.to_string(),
            });
        }
        if self.provider_states.get(provider_id).copied() != Some(ProviderState::Available) {
            return Err(LifecycleError::ProviderNotAvailable {
                provider_id: provider_id.to_string(),
            });
        }

        let raw_reason: String = reason.into();
        let sanitized_reason = sanitize_reason(&raw_reason);
        self.provider_states
            .insert(provider_id.to_string(), ProviderState::Failed);
        self.providers
            .mark_unavailable(provider_id, &sanitized_reason);

        let mut roots = Vec::new();
        for feature_id in self.registry.topological_order() {
            if self.feature_states.get(feature_id).copied() != Some(FeatureState::Ready) {
                continue;
            }
            if let Some(descriptor) = self.registry.descriptor(feature_id) {
                if !self.provider_requirements_available(descriptor) {
                    roots.push(feature_id.clone());
                }
            }
        }

        if roots.is_empty() {
            self.record_failure(None, "a provider became unavailable");
            return Ok(FailureImpact::None);
        }

        let reason = "a required provider is unavailable";
        let affected = self.apply_failure_roots(&roots, FeatureState::Failed, reason);
        Ok(self.finalize_runtime_impact(Some(&roots[0]), reason, affected))
    }

    /// Gate a feature command. The host must be ready/degraded and the feature fully ready.
    pub fn require_feature(
        &self,
        feature_id: &FeatureId,
    ) -> Result<&dyn crate::FeatureRuntime, FeatureUnavailable> {
        if !matches!(self.host_state, HostState::Ready | HostState::Degraded) {
            return Err(FeatureUnavailable::new(
                feature_id.clone(),
                format!("host lifecycle state is '{}'", self.host_state.as_str()),
            ));
        }
        let state = match self.feature_states.get(feature_id).copied() {
            Some(state) => state,
            None => {
                return Err(FeatureUnavailable::new(
                    feature_id.clone(),
                    "feature is not registered",
                ))
            }
        };
        if state != FeatureState::Ready {
            let reason = self
                .feature_reasons
                .get(feature_id)
                .cloned()
                .unwrap_or_else(|| format!("feature state is '{}'", state.as_str()));
            return Err(FeatureUnavailable::new(feature_id.clone(), reason));
        }
        let descriptor = self.registry.descriptor(feature_id).ok_or_else(|| {
            FeatureUnavailable::new(feature_id.clone(), "feature metadata is unavailable")
        })?;
        if descriptor.requires.iter().any(|requirement| {
            self.feature_states.get(&requirement.id).copied() != Some(FeatureState::Ready)
        }) {
            return Err(FeatureUnavailable::new(
                feature_id.clone(),
                "a required feature dependency is not ready",
            ));
        }
        if !self.provider_requirements_available(descriptor) {
            return Err(FeatureUnavailable::new(
                feature_id.clone(),
                "a required provider is unavailable",
            ));
        }
        self.runtimes
            .get(feature_id)
            .map(|runtime| runtime.as_ref())
            .ok_or_else(|| {
                FeatureUnavailable::new(feature_id.clone(), "feature runtime is unavailable")
            })
    }

    pub fn feature_health(
        &self,
        feature_id: &FeatureId,
    ) -> Result<FeatureHealth, FeatureUnavailable> {
        Ok(self.require_feature(feature_id)?.health())
    }

    /// Read-only availability snapshot with lexicographically sorted feature entries.
    pub fn snapshot(&self) -> FeatureAvailabilitySnapshot {
        let features = self
            .registry
            .topological_order()
            .iter()
            .map(|feature_id| {
                let state = self
                    .feature_states
                    .get(feature_id)
                    .copied()
                    .unwrap_or(FeatureState::Starting);
                let reason = self.feature_reasons.get(feature_id).cloned();
                FeatureAvailability::new(feature_id.as_str().to_string(), state, reason)
            })
            .collect::<Vec<_>>();
        FeatureAvailabilitySnapshot::new(
            self.revision,
            self.host_state,
            self.last_failure.clone(),
            features,
        )
    }

    pub fn host_state(&self) -> HostState {
        self.host_state
    }

    pub fn host_state_history(&self) -> &[HostState] {
        &self.state_history
    }

    pub fn feature_state(&self, feature_id: &FeatureId) -> Option<FeatureState> {
        self.feature_states.get(feature_id).copied()
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn last_failure(&self) -> Option<&str> {
        self.last_failure.as_deref()
    }

    pub fn diagnostics(&self) -> &[String] {
        &self.diagnostics
    }

    pub fn active_contributions(&self) -> impl Iterator<Item = &str> + '_ {
        self.active_contributions.iter().map(|contribution_id| contribution_id.as_str())
    }

    pub fn is_contribution_active(&self, contribution_id: &str) -> bool {
        self.active_contributions.contains(contribution_id)
    }

    /// Normal, runtime-fatal, or degraded shutdown. Repeated calls are idempotent.
    pub fn shutdown(&mut self) -> Result<(), LifecycleError> {
        match self.host_state {
            HostState::Stopped | HostState::StartupFailed | HostState::Stopping => {
                return Ok(())
            }
            HostState::Starting => {
                return Err(LifecycleError::NotRunning {
                    state: self.host_state,
                })
            }
            HostState::Ready | HostState::Degraded | HostState::RuntimeFailed => {}
        }

        self.set_host_state(HostState::Stopping);
        self.cleanup_resources();
        self.set_host_state(HostState::Stopped);
        Ok(())
    }

    fn require_running(&self) -> Result<(), LifecycleError> {
        if matches!(self.host_state, HostState::Ready | HostState::Degraded) {
            Ok(())
        } else {
            Err(LifecycleError::NotRunning {
                state: self.host_state,
            })
        }
    }

    fn provider_requirements_available(&self, descriptor: &FeatureDescriptor) -> bool {
        descriptor.provider_requirements.iter().all(|requirement| {
            matches!(
                self.providers.availability(requirement),
                ProviderAvailability::Available
            )
        })
    }

    fn required_ready_violation(&self) -> Option<FeatureId> {
        for descriptor in self.registry.descriptors() {
            if descriptor.startup_policy != crate::StartupPolicy::Required {
                continue;
            }
            if self.feature_states.get(&descriptor.id).copied() != Some(FeatureState::Ready) {
                continue;
            }
            if descriptor.requires.iter().any(|requirement| {
                self.feature_states.get(&requirement.id).copied()
                    != Some(FeatureState::Ready)
            }) {
                return Some(descriptor.id.clone());
            }
            if !self.provider_requirements_available(descriptor) {
                return Some(descriptor.id.clone());
            }
        }
        None
    }

    fn all_required_features_ready(&self) -> bool {
        self.registry.descriptors().all(|descriptor| {
            descriptor.startup_policy != crate::StartupPolicy::Required
                || self.feature_states.get(&descriptor.id).copied()
                    == Some(FeatureState::Ready)
        })
    }

    fn all_features_ready(&self) -> bool {
        self.registry.descriptors().all(|descriptor| {
            self.feature_states.get(&descriptor.id).copied() == Some(FeatureState::Ready)
        })
    }

    fn affected_contains_required(&self, affected: &BTreeSet<FeatureId>) -> bool {
        affected.iter().any(|feature_id| {
            self.registry
                .descriptor(feature_id)
                .map(|descriptor| descriptor.startup_policy == crate::StartupPolicy::Required)
                .unwrap_or(false)
        })
    }

    fn affected_contains_required_in(&self, affected: &[FeatureId]) -> bool {
        affected.iter().any(|feature_id| {
            self.registry
                .descriptor(feature_id)
                .map(|descriptor| descriptor.startup_policy == crate::StartupPolicy::Required)
                .unwrap_or(false)
        })
    }

    fn collect_affected(&self, root: &FeatureId) -> BTreeSet<FeatureId> {
        let mut affected = BTreeSet::new();
        let mut stack = vec![root.clone()];
        while let Some(feature_id) = stack.pop() {
            if !affected.insert(feature_id.clone()) {
                continue;
            }
            if let Some(dependent_ids) = self.registry.dependents_of(&feature_id) {
                for dependent_id in dependent_ids {
                    stack.push(dependent_id.clone());
                }
            }
        }
        affected
    }

    fn apply_failure_roots(
        &mut self,
        roots: &[FeatureId],
        root_state: FeatureState,
        root_reason: &str,
    ) -> BTreeSet<FeatureId> {
        let root_set = roots.iter().cloned().collect::<BTreeSet<_>>();
        let mut affected = BTreeSet::new();
        for root in &root_set {
            affected.extend(self.collect_affected(root));
        }

        for root in &root_set {
            self.set_feature_state(root, root_state, Some(root_reason.to_string()));
        }
        for feature_id in &affected {
            if root_set.contains(feature_id) {
                continue;
            }
            let current = self
                .feature_states
                .get(feature_id)
                .copied()
                .unwrap_or(FeatureState::Starting);
            if matches!(current, FeatureState::Starting | FeatureState::Ready) {
                self.set_feature_state(
                    feature_id,
                    FeatureState::BlockedDependency,
                    Some("a required feature dependency is unavailable".to_string()),
                );
            }
        }

        self.stop_affected_runtimes(&affected);
        for feature_id in &affected {
            self.deactivate_contributions(feature_id);
        }
        affected
    }

    fn finalize_runtime_impact(
        &mut self,
        feature_id: Option<&FeatureId>,
        reason: &str,
        mut affected: BTreeSet<FeatureId>,
    ) -> FailureImpact {
        self.record_failure(feature_id, reason);

        while let Some(violation) = self.required_ready_violation() {
            let violation_reason = "a required feature dependency or provider is unavailable";
            let extra = self.apply_failure_roots(
                std::slice::from_ref(&violation),
                FeatureState::Failed,
                violation_reason,
            );
            affected.extend(extra);
            self.record_failure(Some(&violation), violation_reason);
        }

        let affected = affected.into_iter().collect::<Vec<_>>();
        let fatal = self.affected_contains_required_in(&affected)
            || !self.all_required_features_ready();
        if fatal {
            self.set_host_state(HostState::RuntimeFailed);
            FailureImpact::Fatal { affected }
        } else {
            self.set_host_state(HostState::Degraded);
            FailureImpact::Degraded { affected }
        }
    }

    fn stop_affected_runtimes(&mut self, affected: &BTreeSet<FeatureId>) {
        for feature_id in self.initialized_features.clone().into_iter().rev() {
            if affected.contains(&feature_id) {
                self.shutdown_runtime(&feature_id);
            }
        }
    }

    fn shutdown_runtime(&mut self, feature_id: &FeatureId) {
        if let Some(mut runtime) = self.runtimes.remove(feature_id) {
            if let Err(error) = runtime.shutdown() {
                self.diagnostics.push(format!(
                    "feature '{feature_id}' shutdown failed: {}",
                    sanitize_reason(&error)
                ));
            }
        }
    }

    fn activate_contributions(&mut self, descriptor: &FeatureDescriptor) {
        for contribution in &descriptor.contributions {
            self.active_contributions
                .insert(contribution.id.as_str().to_string());
        }
    }

    fn deactivate_contributions(&mut self, feature_id: &FeatureId) {
        let contribution_ids = self
            .registry
            .descriptor(feature_id)
            .map(|descriptor| {
                descriptor
                    .contributions
                    .iter()
                    .map(|contribution| contribution.id.as_str().to_string())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        for contribution_id in contribution_ids {
            self.active_contributions.remove(&contribution_id);
        }
    }

    fn finish_startup_failure(&mut self, feature_id: Option<&FeatureId>, reason: &str) {
        self.record_failure(feature_id, reason);
        self.cleanup_resources();
        self.set_host_state(HostState::StartupFailed);
    }

    fn cleanup_resources(&mut self) {
        self.shutdown_all_features();
        self.active_contributions.clear();
        self.shutdown_all_providers();
        self.run_core_cleanup();
    }

    fn shutdown_all_features(&mut self) {
        let feature_ids = std::mem::take(&mut self.initialized_features);
        for feature_id in feature_ids.into_iter().rev() {
            self.shutdown_runtime(&feature_id);
            if self.feature_states.get(&feature_id).copied() == Some(FeatureState::Ready) {
                self.set_feature_state(&feature_id, FeatureState::Stopped, None);
            }
        }
        let remaining = self.feature_states.keys().cloned().collect::<Vec<_>>();
        for feature_id in remaining {
            if self.feature_states.get(&feature_id).copied() == Some(FeatureState::Starting) {
                self.set_feature_state(&feature_id, FeatureState::Stopped, None);
            }
        }
    }

    fn shutdown_all_providers(&mut self) {
        let provider_ids = std::mem::take(&mut self.initialized_providers);
        for provider_id in provider_ids.into_iter().rev() {
            if let Err(error) = self.providers.shutdown(&provider_id) {
                self.diagnostics.push(format!(
                    "provider shutdown failed: {}",
                    sanitize_reason(&error)
                ));
            }
            self.provider_states
                .insert(provider_id, ProviderState::Stopped);
        }
    }

    fn run_core_cleanup(&mut self) {
        if self.core_cleanup_ran {
            return;
        }
        self.core_cleanup_ran = true;
        if let Some(mut cleanup) = self.core_cleanup.take() {
            if let Err(error) = cleanup() {
                self.diagnostics.push(format!(
                    "core cleanup failed: {}",
                    sanitize_reason(&error)
                ));
            }
        }
    }

    fn set_host_state(&mut self, state: HostState) {
        if self.host_state == state {
            return;
        }
        self.host_state = state;
        self.state_history.push(state);
        self.revision = self.revision.saturating_add(1);
    }

    fn set_feature_state(
        &mut self,
        feature_id: &FeatureId,
        state: FeatureState,
        reason: Option<String>,
    ) {
        let reason = reason.map(|value| self.public_reason(&value));
        let changed = self.feature_states.get(feature_id).copied() != Some(state)
            || self.feature_reasons.get(feature_id).cloned() != reason;
        if !changed {
            return;
        }
        self.feature_states.insert(feature_id.clone(), state);
        match reason {
            Some(reason) => {
                self.feature_reasons.insert(feature_id.clone(), reason);
            }
            None => {
                self.feature_reasons.remove(feature_id);
            }
        }
        self.revision = self.revision.saturating_add(1);
    }

    fn record_failure(&mut self, feature_id: Option<&FeatureId>, reason: &str) {
        let reason = self.public_reason(reason);
        let failure = match feature_id {
            Some(feature_id) => format!("feature '{feature_id}': {reason}"),
            None => format!("host: {reason}"),
        };
        if self.last_failure.as_deref() != Some(failure.as_str()) {
            self.last_failure = Some(failure);
            self.revision = self.revision.saturating_add(1);
        }
    }

    fn public_reason(&self, value: &str) -> String {
        let mut reason = sanitize_reason(value);
        for provider_id in &self.provider_order {
            if !provider_id.is_empty() {
                reason = reason.replace(provider_id, "<provider>");
            }
        }
        sanitize_reason(&reason)
    }
}

const MAX_STABLE_PROVIDER_ID_BYTES: usize = 128;

/// Mirrors the provider API stable-ID grammar without adding a crate dependency.
fn stable_provider_id_error(value: &str) -> Option<&'static str> {
    let bytes = value.as_bytes();
    if bytes.is_empty() {
        return Some("must not be empty");
    }
    if bytes.len() > MAX_STABLE_PROVIDER_ID_BYTES {
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
            || is_stable_provider_id_separator(byte))
    }) {
        return Some(
            "may contain only lowercase ASCII letters, digits, '.', '-', '_', and ':'",
        );
    }
    if bytes.windows(2).any(|pair| {
        is_stable_provider_id_separator(pair[0]) && is_stable_provider_id_separator(pair[1])
    }) {
        return Some("must not contain consecutive separators");
    }
    None
}

fn is_stable_provider_id_separator(byte: u8) -> bool {
    matches!(byte, b'.' | b'-' | b'_' | b':')
}

fn sanitize_reason(value: &str) -> String {
    let mut cleaned = String::new();
    let mut previous_was_space = false;
    for character in value.chars() {
        if character.is_control() || character.is_whitespace() {
            if !previous_was_space && !cleaned.is_empty() {
                cleaned.push(' ');
                previous_was_space = true;
            }
        } else {
            cleaned.push(character);
            previous_was_space = false;
        }
    }
    let cleaned = cleaned.trim();
    if cleaned.is_empty() {
        return "unspecified failure".to_string();
    }

    let lowercase = cleaned.to_ascii_lowercase();
    if [
        "token",
        "secret",
        "password",
        "authorization",
        "api_key",
        "apikey",
    ]
    .iter()
    .any(|marker| lowercase.contains(marker))
    {
        return "operation failed".to_string();
    }

    if cleaned.chars().count() > 240 {
        let mut truncated = cleaned.chars().take(240).collect::<String>();
        truncated.push_str("...");
        truncated
    } else {
        cleaned.to_string()
    }
}

#[cfg(test)]
mod tests;