use std::sync::{Mutex, MutexGuard};

use crate::provider_composition::ProviderRuntimeState;
use flint_feature_api::{
    ApiVersion, FeatureAvailability, FeatureAvailabilitySnapshot, FeatureDescriptor, FeatureHealth,
    FeatureId, FeatureInitContext, FeatureInitError, FeatureInitializer, FeatureRegistration,
    FeatureRegistry, FeatureRequirement, FeatureRuntime, FeatureState, HostState,
    LifecycleCoordinator, ProviderRequirement, StartupPolicy, Version, VersionReq,
};

const FEATURE_API_VERSION: ApiVersion = ApiVersion::new(1, 0);
const FEATURE_VERSION_REQUIREMENT: &str = "^1.0.0";

pub(crate) const FEATURE_SETTINGS: &str = "flint.settings";
pub(crate) const FEATURE_ACCOUNTS: &str = "flint.accounts";
pub(crate) const FEATURE_VANILLA: &str = "flint.vanilla";
pub(crate) const FEATURE_LAUNCH: &str = "flint.launch";
pub(crate) const FEATURE_LOADERS: &str = "flint.loaders";
pub(crate) const FEATURE_MODS: &str = "flint.mods";
pub(crate) const FEATURE_SAVES: &str = "flint.saves";
const FEATURE_IDS: [&str; 7] = [
    FEATURE_SETTINGS,
    FEATURE_ACCOUNTS,
    FEATURE_VANILLA,
    FEATURE_LAUNCH,
    FEATURE_LOADERS,
    FEATURE_MODS,
    FEATURE_SAVES,
];

fn feature_id(value: &str) -> Result<FeatureId, String> {
    FeatureId::new(value).map_err(|error| format!("invalid host feature ID '{value}': {error}"))
}

fn feature_requirement(value: &str) -> Result<FeatureRequirement, String> {
    let version_req = VersionReq::parse(FEATURE_VERSION_REQUIREMENT)
        .map_err(|error| format!("invalid host feature version requirement: {error}"))?;
    Ok(FeatureRequirement::new(feature_id(value)?, version_req))
}

fn feature_descriptor(
    value: &str,
    startup_policy: StartupPolicy,
    required_features: &[&str],
    provider_requirements: Vec<ProviderRequirement>,
) -> Result<FeatureDescriptor, String> {
    let requires = required_features
        .iter()
        .map(|required| feature_requirement(*required))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(FeatureDescriptor {
        id: feature_id(value)?,
        feature_version: Version::new(1, 0, 0),
        api_version: FEATURE_API_VERSION,
        startup_policy,
        requires,
        provider_requirements,
        contributions: Vec::new(),
    })
}

struct StatelessFeatureRuntime;

impl FeatureRuntime for StatelessFeatureRuntime {
    fn health(&self) -> FeatureHealth {
        FeatureHealth::Healthy
    }

    fn shutdown(&mut self) -> Result<(), String> {
        Ok(())
    }
}

struct StatelessFeatureInitializer;

impl FeatureInitializer for StatelessFeatureInitializer {
    fn initialize(
        &self,
        _context: &FeatureInitContext,
    ) -> Result<Box<dyn FeatureRuntime>, FeatureInitError> {
        Ok(Box::new(StatelessFeatureRuntime))
    }
}

fn registration(descriptor: FeatureDescriptor) -> FeatureRegistration {
    FeatureRegistration::new(descriptor, Box::new(StatelessFeatureInitializer))
}

fn build_feature_registry() -> Result<FeatureRegistry, String> {
    let registrations = vec![
        registration(feature_descriptor(
            FEATURE_SETTINGS,
            StartupPolicy::Required,
            &[],
            Vec::new(),
        )?),
        registration(feature_descriptor(
            FEATURE_ACCOUNTS,
            StartupPolicy::Required,
            &[FEATURE_SETTINGS],
            Vec::new(),
        )?),
        registration(feature_descriptor(
            FEATURE_VANILLA,
            StartupPolicy::Required,
            &[FEATURE_SETTINGS],
            Vec::new(),
        )?),
        registration(feature_descriptor(
            FEATURE_LAUNCH,
            StartupPolicy::Required,
            &[FEATURE_ACCOUNTS, FEATURE_VANILLA, FEATURE_SETTINGS],
            Vec::new(),
        )?),
        registration(feature_descriptor(
            FEATURE_LOADERS,
            StartupPolicy::Optional,
            &[FEATURE_VANILLA],
            crate::provider_composition::loader_provider_requirements(),
        )?),
        registration(feature_descriptor(
            FEATURE_MODS,
            StartupPolicy::Optional,
            &[FEATURE_SETTINGS],
            crate::provider_composition::mod_provider_requirements(),
        )?),
        registration(feature_descriptor(
            FEATURE_SAVES,
            StartupPolicy::Optional,
            &[FEATURE_SETTINGS],
            Vec::new(),
        )?),
    ];

    FeatureRegistry::new(FEATURE_API_VERSION, registrations)
        .map_err(|error| format!("invalid host feature registry: {error}"))
}

fn startup_failed_snapshot() -> FeatureAvailabilitySnapshot {
    let features = FEATURE_IDS
        .iter()
        .map(|feature_id| {
            FeatureAvailability::new(
                *feature_id,
                FeatureState::Stopped,
                Some("host startup failed".to_string()),
            )
        })
        .collect();
    FeatureAvailabilitySnapshot::new(
        1,
        HostState::StartupFailed,
        Some("host feature lifecycle startup failed".to_string()),
        features,
    )
}

enum ShutdownStart {
    AlreadyComplete,
    Coordinator(LifecycleCoordinator),
    SnapshotOnly,
}

fn snapshot_with_host_state(
    snapshot: &FeatureAvailabilitySnapshot,
    host_state: HostState,
) -> FeatureAvailabilitySnapshot {
    let mut updated = snapshot.clone();
    if updated.host_state != host_state {
        updated.host_state = host_state;
        updated.revision = updated.revision.saturating_add(1);
    }
    updated
}

struct HostRuntimeState {
    coordinator: Option<LifecycleCoordinator>,
    snapshot: FeatureAvailabilitySnapshot,
    shutdown_in_progress: bool,
}

pub(crate) struct HostRuntime {
    state: Mutex<HostRuntimeState>,
}

impl HostRuntime {
    fn from_coordinator(coordinator: LifecycleCoordinator) -> Self {
        let snapshot = coordinator.snapshot();
        Self {
            state: Mutex::new(HostRuntimeState {
                coordinator: Some(coordinator),
                snapshot,
                shutdown_in_progress: false,
            }),
        }
    }

    fn startup_failed() -> Self {
        Self {
            state: Mutex::new(HostRuntimeState {
                coordinator: None,
                snapshot: startup_failed_snapshot(),
                shutdown_in_progress: false,
            }),
        }
    }

    fn lock_state(&self) -> MutexGuard<'_, HostRuntimeState> {
        match self.state.lock() {
            Ok(state) => state,
            Err(poisoned) => poisoned.into_inner(),
        }
    }

    pub(crate) fn snapshot(&self) -> FeatureAvailabilitySnapshot {
        let state = self.lock_state();
        if state.shutdown_in_progress {
            snapshot_with_host_state(&state.snapshot, HostState::Stopping)
        } else {
            state.snapshot.clone()
        }
    }

    pub(crate) fn ensure_feature_ready(&self, feature_id: &str) -> Result<(), String> {
        let stable_id = FeatureId::new(feature_id)
            .map_err(|_| format!("Feature '{feature_id}' is unavailable: feature ID is invalid"))?;
        let state = self.lock_state();
        if state.shutdown_in_progress {
            return Err(format!(
                "Feature '{feature_id}' is unavailable: host lifecycle state is 'stopping'"
            ));
        }
        if let Some(coordinator) = state.coordinator.as_ref() {
            return match coordinator.require_feature(&stable_id) {
                Ok(_) => Ok(()),
                Err(unavailable) => Err(format!(
                    "Feature '{}' is unavailable: {}",
                    unavailable.feature_id(),
                    unavailable.reason()
                )),
            };
        }

        let snapshot = &state.snapshot;
        if !matches!(snapshot.host_state, HostState::Ready | HostState::Degraded) {
            return Err(format!(
                "Feature '{feature_id}' is unavailable: host lifecycle state is '{}'",
                snapshot.host_state.as_str()
            ));
        }
        let Some(feature) = snapshot
            .features
            .iter()
            .find(|feature| feature.feature_id == feature_id)
        else {
            return Err(format!(
                "Feature '{feature_id}' is unavailable: feature is not registered"
            ));
        };
        if feature.state == FeatureState::Ready {
            return Ok(());
        }
        let reason = feature
            .reason
            .as_deref()
            .filter(|reason| !reason.trim().is_empty())
            .unwrap_or_else(|| feature.state.as_str());
        Err(format!(
            "Feature '{feature_id}' is unavailable: {reason}"
        ))
    }

    fn begin_shutdown(&self) -> ShutdownStart {
        // Publish Stopping and remove the coordinator under one lock so fallback
        // gating never observes stale Ready.
        let mut state = self.lock_state();
        if state.shutdown_in_progress {
            return ShutdownStart::AlreadyComplete;
        }
        if state.coordinator.is_none() && state.snapshot.host_state == HostState::Stopped {
            return ShutdownStart::AlreadyComplete;
        }

        let coordinator = state.coordinator.take();
        state.shutdown_in_progress = true;
        state.snapshot = snapshot_with_host_state(&state.snapshot, HostState::Stopping);
        match coordinator {
            Some(coordinator) => ShutdownStart::Coordinator(coordinator),
            None => ShutdownStart::SnapshotOnly,
        }
    }

    fn finish_shutdown(&self, snapshot: FeatureAvailabilitySnapshot) {
        let snapshot = snapshot_with_host_state(&snapshot, HostState::Stopped);
        let mut state = self.lock_state();
        state.snapshot = snapshot;
        state.shutdown_in_progress = false;
    }

    pub(crate) fn shutdown(&self) {
        let snapshot = match self.begin_shutdown() {
            ShutdownStart::AlreadyComplete => return,
            ShutdownStart::Coordinator(mut coordinator) => {
                if let Err(error) = coordinator.shutdown() {
                    eprintln!("Host feature lifecycle shutdown failed: {error}");
                }
                coordinator.snapshot()
            }
            ShutdownStart::SnapshotOnly => {
                let state = self.lock_state();
                state.snapshot.clone()
            }
        };
        self.finish_shutdown(snapshot);
    }
}

/// Starts feature lifecycle against the same shared provider runtime held by Tauri state.
pub(crate) fn initialize_host_runtime(provider_runtime: ProviderRuntimeState) -> HostRuntime {
    let registry = match build_feature_registry() {
        Ok(registry) => registry,
        Err(error) => {
            eprintln!("Host feature registry composition failed: {error}");
            return HostRuntime::startup_failed();
        }
    };

    let mut coordinator = LifecycleCoordinator::new(registry, Box::new(provider_runtime), None);
    if let Err(error) = coordinator.start() {
        eprintln!("Host feature lifecycle startup failed: {error}");
        return HostRuntime::startup_failed();
    }
    HostRuntime::from_coordinator(coordinator)
}

#[cfg(test)]
mod tests {
    use super::*;
    use flint_feature_api::{ProviderAvailability, ProviderCatalog as _, StartupOutcome};

    fn provider_requirement(selection_key: &str) -> ProviderRequirement {
        crate::provider_composition::loader_provider_requirements()
            .into_iter()
            .chain(crate::provider_composition::mod_provider_requirements())
            .find(|requirement| requirement.selection_key == Some(selection_key))
            .expect("reviewed provider requirement")
    }

    fn descriptor(id: &str) -> FeatureDescriptor {
        let registry = build_feature_registry().expect("host feature registry");
        registry
            .descriptor(&FeatureId::new(id).expect("test feature ID"))
            .expect("registered feature")
            .clone()
    }

    fn feature_state(snapshot: &FeatureAvailabilitySnapshot, feature_id: &str) -> FeatureState {
        snapshot
            .features
            .iter()
            .find(|feature| feature.feature_id == feature_id)
            .map(|feature| feature.state)
            .expect("registered feature in snapshot")
    }

    fn required_provider_feature_registry() -> FeatureRegistry {
        const PROVIDER_FEATURE: &str = "flint.phase5-provider";
        const DEPENDENT_FEATURE: &str = "flint.phase5-dependent";
        FeatureRegistry::new(
            FEATURE_API_VERSION,
            vec![
                registration(
                    feature_descriptor(
                        PROVIDER_FEATURE,
                        StartupPolicy::Required,
                        &[],
                        crate::provider_composition::mod_provider_requirements(),
                    )
                    .expect("provider-backed feature descriptor"),
                ),
                registration(
                    feature_descriptor(
                        DEPENDENT_FEATURE,
                        StartupPolicy::Required,
                        &[PROVIDER_FEATURE],
                        Vec::new(),
                    )
                    .expect("dependent feature descriptor"),
                ),
            ],
        )
        .expect("required provider feature registry")
    }

    #[test]
    fn managed_host_runtime_is_send_and_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<HostRuntime>();
    }

    #[test]
    fn composition_registers_all_seven_features_with_expected_policies_and_edges() {
        let registry = build_feature_registry().expect("host feature registry");
        assert_eq!(registry.len(), 7);
        for feature_id in FEATURE_IDS {
            let declared = registry
                .descriptor(&FeatureId::new(feature_id).expect("feature ID"))
                .expect("registered feature");
            assert_eq!(declared.feature_version, Version::new(1, 0, 0));
            assert_eq!(declared.api_version, FEATURE_API_VERSION);
        }

        let settings = descriptor(FEATURE_SETTINGS);
        assert_eq!(settings.startup_policy, StartupPolicy::Required);
        assert!(settings.requires.is_empty());

        let accounts = descriptor(FEATURE_ACCOUNTS);
        assert_eq!(accounts.startup_policy, StartupPolicy::Required);
        assert_eq!(
            accounts
                .requires
                .iter()
                .map(|requirement| requirement.id.as_str())
                .collect::<Vec<_>>(),
            vec![FEATURE_SETTINGS]
        );

        let vanilla = descriptor(FEATURE_VANILLA);
        assert_eq!(vanilla.startup_policy, StartupPolicy::Required);
        assert_eq!(
            vanilla
                .requires
                .iter()
                .map(|requirement| requirement.id.as_str())
                .collect::<Vec<_>>(),
            vec![FEATURE_SETTINGS]
        );

        let launch = descriptor(FEATURE_LAUNCH);
        assert_eq!(launch.startup_policy, StartupPolicy::Required);
        assert_eq!(
            launch
                .requires
                .iter()
                .map(|requirement| requirement.id.as_str())
                .collect::<Vec<_>>(),
            vec![FEATURE_ACCOUNTS, FEATURE_VANILLA, FEATURE_SETTINGS]
        );

        let loaders = descriptor(FEATURE_LOADERS);
        assert_eq!(loaders.startup_policy, StartupPolicy::Optional);
        assert_eq!(
            loaders
                .requires
                .iter()
                .map(|requirement| requirement.id.as_str())
                .collect::<Vec<_>>(),
            vec![FEATURE_VANILLA]
        );
        assert_eq!(loaders.provider_requirements.len(), 6);

        let mods = descriptor(FEATURE_MODS);
        assert_eq!(mods.startup_policy, StartupPolicy::Optional);
        assert_eq!(
            mods.requires
                .iter()
                .map(|requirement| requirement.id.as_str())
                .collect::<Vec<_>>(),
            vec![FEATURE_SETTINGS]
        );
        assert_eq!(mods.provider_requirements.len(), 2);

        let saves = descriptor(FEATURE_SAVES);
        assert_eq!(saves.startup_policy, StartupPolicy::Optional);
        assert_eq!(
            saves
                .requires
                .iter()
                .map(|requirement| requirement.id.as_str())
                .collect::<Vec<_>>(),
            vec![FEATURE_SETTINGS]
        );
    }

    #[test]
    fn shared_provider_runtime_selects_every_requirement_from_actual_registries() {
        let mut catalog = ProviderRuntimeState::new();
        let provider_ids = catalog.provider_ids();
        assert_eq!(
            provider_ids
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>(),
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
        for requirement in crate::provider_composition::loader_provider_requirements() {
            assert!(matches!(
                catalog.availability(&requirement),
                ProviderAvailability::Available
            ));
        }
        for requirement in crate::provider_composition::mod_provider_requirements() {
            assert!(matches!(
                catalog.availability(&requirement),
                ProviderAvailability::Available
            ));
        }

        catalog.mark_unavailable("fabric", "test provider loss");
        assert!(matches!(
            catalog.availability(&provider_requirement("loader-kind:fabric")),
            ProviderAvailability::Unavailable { .. }
        ));
        assert!(matches!(
            catalog.availability(&provider_requirement("loader-kind:forge")),
            ProviderAvailability::Available
        ));
    }

    #[test]
    fn registry_failure_keeps_optional_provider_features_unready_and_gated() {
        let runtime = initialize_host_runtime(crate::provider_composition::test_support::construction_failure_for_test());
        let snapshot = runtime.snapshot();
        assert_eq!(snapshot.host_state, HostState::Degraded);
        assert_eq!(
            feature_state(&snapshot, FEATURE_MODS),
            FeatureState::BlockedDependency
        );
        assert_eq!(
            feature_state(&snapshot, FEATURE_LOADERS),
            FeatureState::BlockedDependency
        );
        assert!(runtime.ensure_feature_ready(FEATURE_MODS).is_err());
        assert!(runtime.ensure_feature_ready(FEATURE_LOADERS).is_err());
        assert!(runtime.ensure_feature_ready(FEATURE_SETTINGS).is_ok());
    }

    #[test]
    fn actual_registry_failure_fails_required_provider_feature_startup() {
        let mut coordinator = LifecycleCoordinator::new(
            required_provider_feature_registry(),
            Box::new(crate::provider_composition::test_support::construction_failure_for_test()),
            None,
        );
        assert_eq!(
            coordinator.start().expect("startup result"),
            StartupOutcome::StartupFailed
        );
        let snapshot = coordinator.snapshot();
        assert_eq!(snapshot.host_state, HostState::StartupFailed);
        assert_ne!(
            feature_state(&snapshot, "flint.phase5-provider"),
            FeatureState::Ready
        );
        assert_eq!(
            feature_state(&snapshot, "flint.phase5-dependent"),
            FeatureState::BlockedDependency
        );
    }
    #[test]
    fn shutdown_publishes_stopping_and_gates_before_teardown_completes() {
        let runtime = initialize_host_runtime(ProviderRuntimeState::new());
        assert!(runtime.ensure_feature_ready(FEATURE_SETTINGS).is_ok());

        let mut coordinator = match runtime.begin_shutdown() {
            ShutdownStart::Coordinator(coordinator) => coordinator,
            ShutdownStart::AlreadyComplete | ShutdownStart::SnapshotOnly => {
                panic!("expected coordinator-backed shutdown")
            }
        };

        assert_eq!(runtime.snapshot().host_state, HostState::Stopping);
        let error = runtime
            .ensure_feature_ready(FEATURE_SETTINGS)
            .expect_err("commands must fail closed during shutdown");
        assert_eq!(
            error.as_str(),
            "Feature 'flint.settings' is unavailable: host lifecycle state is 'stopping'"
        );

        coordinator.shutdown().expect("coordinator shutdown");
        runtime.finish_shutdown(coordinator.snapshot());
        assert_eq!(runtime.snapshot().host_state, HostState::Stopped);
        let state = runtime.lock_state();
        assert!(!state.shutdown_in_progress);
        drop(state);

        runtime.shutdown();
        assert_eq!(runtime.snapshot().host_state, HostState::Stopped);
    }

    #[test]
    fn host_runtime_starts_ready_gates_features_and_shuts_down_idempotently() {
        let runtime = initialize_host_runtime(ProviderRuntimeState::new());
        let snapshot = runtime.snapshot();
        assert_eq!(snapshot.host_state, HostState::Ready);
        assert_eq!(snapshot.features.len(), 7);
        let mut expected_ids = FEATURE_IDS.to_vec();
        expected_ids.sort_unstable();
        assert_eq!(
            snapshot
                .features
                .iter()
                .map(|feature| feature.feature_id.as_str())
                .collect::<Vec<_>>(),
            expected_ids
        );
        assert!(snapshot
            .features
            .iter()
            .all(|feature| feature.state == FeatureState::Ready));
        assert!(runtime.ensure_feature_ready(FEATURE_SETTINGS).is_ok());
        assert!(runtime.ensure_feature_ready(FEATURE_LAUNCH).is_ok());

        runtime.shutdown();
        runtime.shutdown();
        let stopped = runtime.snapshot();
        assert_eq!(stopped.host_state, HostState::Stopped);
        let error = runtime
            .ensure_feature_ready(FEATURE_SETTINGS)
            .expect_err("stopped host must gate commands");
        assert_eq!(
            error.as_str(),
            "Feature 'flint.settings' is unavailable: host lifecycle state is 'stopped'"
        );
    }

    #[test]
    fn poisoned_host_runtime_lock_is_recovered() {
        let runtime = HostRuntime::startup_failed();
        let poisoned = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = runtime.state.lock().expect("fresh host runtime lock");
            panic!("poison host runtime lock");
        }));
        assert!(poisoned.is_err());
        assert_eq!(runtime.snapshot().host_state, HostState::StartupFailed);
        assert!(runtime.ensure_feature_ready(FEATURE_SETTINGS).is_err());
    }

    #[test]
    fn fallback_runtime_returns_a_valid_startup_failed_snapshot_and_gates_commands() {
        let runtime = HostRuntime::startup_failed();
        let snapshot = runtime.snapshot();
        assert_eq!(snapshot.host_state, HostState::StartupFailed);
        assert_eq!(snapshot.features.len(), 7);
        let mut expected_ids = FEATURE_IDS.to_vec();
        expected_ids.sort_unstable();
        assert_eq!(
            snapshot
                .features
                .iter()
                .map(|feature| feature.feature_id.as_str())
                .collect::<Vec<_>>(),
            expected_ids
        );
        assert!(snapshot.last_failure.is_some());
        assert!(snapshot
            .features
            .iter()
            .all(|feature| feature.state == FeatureState::Stopped));

        let error = runtime
            .ensure_feature_ready(FEATURE_SETTINGS)
            .expect_err("startup failure must gate features");
        assert!(error.starts_with("Feature 'flint.settings' is unavailable: "));
        assert!(!error.to_ascii_lowercase().contains("token"));

        runtime.shutdown();
        assert_eq!(runtime.snapshot().host_state, HostState::Stopped);
        runtime.shutdown();
        assert_eq!(runtime.snapshot().host_state, HostState::Stopped);
    }
}
