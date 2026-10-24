use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
};

use semver::{Version, VersionReq};

use crate::{
    ApiVersion, ContributionDescriptor, ContributionId, ContributionKind, CoreCleanup,
    FailureImpact, FeatureDescriptor, FeatureHealth, FeatureId, FeatureInitContext, FeatureInitError,
    FeatureInitializer, FeatureRegistration, FeatureRegistry, FeatureRuntime, FeatureState,
    HostState, LifecycleCoordinator, LifecycleError, ProviderAvailability, ProviderCatalog,
    ProviderRequirement, StartupOutcome, StartupPolicy,
};

use super::{
    sanitize_reason, stable_provider_id_error, ProviderState, MAX_STABLE_PROVIDER_ID_BYTES,
};

#[derive(Clone)]
struct RuntimeLogs {
    initialized_features: Arc<Mutex<Vec<String>>>,
    shut_down_features: Arc<Mutex<Vec<String>>>,
}

impl Default for RuntimeLogs {
    fn default() -> Self {
        Self {
            initialized_features: Arc::new(Mutex::new(Vec::new())),
            shut_down_features: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

struct RecordingRuntime {
    feature_id: String,
    logs: RuntimeLogs,
}

impl FeatureRuntime for RecordingRuntime {
    fn health(&self) -> FeatureHealth {
        FeatureHealth::Healthy
    }

    fn shutdown(&mut self) -> Result<(), String> {
        self.logs
            .shut_down_features
            .lock()
            .expect("runtime shutdown log lock")
            .push(self.feature_id.clone());
        Ok(())
    }
}

struct TestInitializer {
    logs: RuntimeLogs,
    failure: Option<String>,
}

impl FeatureInitializer for TestInitializer {
    fn initialize(
        &self,
        context: &FeatureInitContext,
    ) -> Result<Box<dyn FeatureRuntime>, FeatureInitError> {
        self.logs
            .initialized_features
            .lock()
            .expect("runtime init log lock")
            .push(context.feature_id().to_string());
        if let Some(reason) = &self.failure {
            return Err(FeatureInitError::new(reason.clone()));
        }
        Ok(Box::new(RecordingRuntime {
            feature_id: context.feature_id().to_string(),
            logs: self.logs.clone(),
        }))
    }
}

struct TestProviderCatalog {
    provider_ids: Vec<String>,
    capability_providers: BTreeMap<String, String>,
    failed_init: BTreeSet<String>,
    mark_failed_init_on_error: bool,
    failed_provider_ids: BTreeSet<String>,
    lost_provider_ids: BTreeSet<String>,
    marked_unavailable: Arc<Mutex<Vec<String>>>,
    ambiguous_capabilities: BTreeSet<String>,
    shutdown_failures: BTreeSet<String>,
    initialized_providers: Arc<Mutex<Vec<String>>>,
    shut_down_providers: Arc<Mutex<Vec<String>>>,
}

impl TestProviderCatalog {
    fn new(provider_ids: &[&str]) -> Self {
        Self {
            provider_ids: provider_ids
                .iter()
                .map(|provider_id| (*provider_id).to_string())
                .collect(),
            capability_providers: BTreeMap::new(),
            failed_init: BTreeSet::new(),
            mark_failed_init_on_error: true,
            failed_provider_ids: BTreeSet::new(),
            lost_provider_ids: BTreeSet::new(),
            marked_unavailable: Arc::new(Mutex::new(Vec::new())),
            ambiguous_capabilities: BTreeSet::new(),
            shutdown_failures: BTreeSet::new(),
            initialized_providers: Arc::new(Mutex::new(Vec::new())),
            shut_down_providers: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn with_capability(mut self, capability: &str, provider_id: &str) -> Self {
        self.capability_providers
            .insert(capability.to_string(), provider_id.to_string());
        self
    }

    fn with_failed_init(mut self, provider_id: &str) -> Self {
        self.failed_init.insert(provider_id.to_string());
        self
    }

    fn with_unmarked_failed_init(mut self, provider_id: &str) -> Self {
        self.failed_init.insert(provider_id.to_string());
        self.mark_failed_init_on_error = false;
        self
    }

    fn with_ambiguous_capability(mut self, capability: &str) -> Self {
        self.ambiguous_capabilities
            .insert(capability.to_string());
        self
    }

    fn with_shutdown_failure(mut self, provider_id: &str) -> Self {
        self.shutdown_failures.insert(provider_id.to_string());
        self
    }
}

impl ProviderCatalog for TestProviderCatalog {
    fn provider_ids(&self) -> Vec<String> {
        self.provider_ids.clone()
    }

    fn availability(&self, requirement: &ProviderRequirement) -> ProviderAvailability {
        if self
            .ambiguous_capabilities
            .contains(requirement.capability)
        {
            return ProviderAvailability::Ambiguous {
                provider_ids: vec!["provider.one".to_string(), "provider.two".to_string()],
            };
        }
        match self.capability_providers.get(requirement.capability) {
            Some(provider_id)
                if !self
                    .failed_provider_ids
                    .contains(provider_id.as_str())
                    && !self
                        .lost_provider_ids
                        .contains(provider_id.as_str()) =>
            {
                ProviderAvailability::Available
            }
            Some(provider_id) => ProviderAvailability::Unavailable {
                reason: format!("provider '{provider_id}' is unavailable"),
            },
            None => ProviderAvailability::Unavailable {
                reason: "no provider matches the requirement".to_string(),
            },
        }
    }

    fn initialize(&mut self, provider_id: &str) -> Result<(), String> {
        self.initialized_providers
            .lock()
            .expect("provider init log lock")
            .push(provider_id.to_string());
        if self.failed_init.contains(provider_id) {
            if self.mark_failed_init_on_error {
                self.failed_provider_ids.insert(provider_id.to_string());
            }
            return Err("provider initialization failed with token=hidden".to_string());
        }
        Ok(())
    }

    fn shutdown(&mut self, provider_id: &str) -> Result<(), String> {
        self.shut_down_providers
            .lock()
            .expect("provider shutdown log lock")
            .push(provider_id.to_string());
        if self.shutdown_failures.contains(provider_id) {
            Err("provider shutdown failed with token=hidden".to_string())
        } else {
            Ok(())
        }
    }

    fn mark_unavailable(&mut self, provider_id: &str, _reason: &str) {
        self.lost_provider_ids.insert(provider_id.to_string());
        self.marked_unavailable
            .lock()
            .expect("marked unavailable log lock")
            .push(provider_id.to_string());
    }
}

fn feature_id(value: &str) -> FeatureId {
    FeatureId::new(value).expect("valid test feature ID")
}

fn feature_requirement(value: &str, version_req: &str) -> crate::FeatureRequirement {
    crate::FeatureRequirement::new(
        feature_id(value),
        VersionReq::parse(version_req).expect("valid test version requirement"),
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

fn descriptor(feature_id: &str, startup_policy: StartupPolicy) -> FeatureDescriptor {
    FeatureDescriptor {
        id: self::feature_id(feature_id),
        feature_version: Version::new(1, 0, 0),
        api_version: ApiVersion::new(1, 0),
        startup_policy,
        requires: Vec::new(),
        provider_requirements: Vec::new(),
        contributions: Vec::new(),
    }
}

fn registration(
    descriptor: FeatureDescriptor,
    logs: &RuntimeLogs,
    failure: Option<&str>,
) -> FeatureRegistration {
    FeatureRegistration::new(
        descriptor,
        Box::new(TestInitializer {
            logs: logs.clone(),
            failure: failure.map(str::to_string),
        }),
    )
}

fn contribution(
    descriptor: &FeatureDescriptor,
    suffix: &str,
    kind: ContributionKind,
) -> ContributionDescriptor {
    let contribution_id = ContributionId::new(
        &descriptor.id,
        format!("{}.{}", descriptor.id, suffix),
    )
    .expect("valid contribution ID");
    ContributionDescriptor::new(contribution_id, kind)
}

fn cleanup_counter(calls: &Arc<AtomicUsize>) -> Option<CoreCleanup> {
    let calls = calls.clone();
    Some(Box::new(move || {
        calls.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }))
}

fn initialized_features(logs: &RuntimeLogs) -> Vec<String> {
    logs.initialized_features
        .lock()
        .expect("runtime init log lock")
        .clone()
}

fn shut_down_features(logs: &RuntimeLogs) -> Vec<String> {
    logs.shut_down_features
        .lock()
        .expect("runtime shutdown log lock")
        .clone()
}

#[test]
fn public_failure_reasons_are_control_safe_bounded_and_redacted() {
    assert_eq!(
        sanitize_reason(" line one\nline two\tline three "),
        "line one line two line three"
    );
    assert_eq!(sanitize_reason("token=hidden"), "operation failed");
    assert_eq!(sanitize_reason(""), "unspecified failure");

    let bounded = sanitize_reason(&"x".repeat(400));
    assert!(bounded.chars().count() <= 243);
    assert!(bounded.ends_with("..."));
}

#[test]
fn startup_shutdown_is_deterministic_reverse_ordered_and_idempotent() {
    let logs = RuntimeLogs::default();
    let core_calls = Arc::new(AtomicUsize::new(0));
    let catalog = TestProviderCatalog::new(&[
        "provider.zeta",
        "provider.alpha",
        "provider.middle",
    ])
    .with_shutdown_failure("provider.middle");
    let provider_init_log = catalog.initialized_providers.clone();
    let provider_shutdown_log = catalog.shut_down_providers.clone();

    let mut alpha = descriptor("flint.alpha", StartupPolicy::Required);
    alpha.requires = vec![feature_requirement("flint.zeta", "^1.0.0")];

    let registry = FeatureRegistry::new(
        ApiVersion::new(1, 0),
        vec![
            registration(descriptor("flint.gamma", StartupPolicy::Required), &logs, None),
            registration(descriptor("flint.beta", StartupPolicy::Optional), &logs, None),
            registration(descriptor("flint.zeta", StartupPolicy::Optional), &logs, None),
            registration(alpha, &logs, None),
        ],
    )
    .expect("valid feature registry");

    let mut lifecycle =
        LifecycleCoordinator::new(registry, Box::new(catalog), cleanup_counter(&core_calls));

    assert_eq!(lifecycle.start(), Ok(StartupOutcome::Ready));
    assert_eq!(
        initialized_features(&logs),
        vec!["flint.beta", "flint.gamma", "flint.zeta", "flint.alpha"]
    );
    assert_eq!(
        *provider_init_log.lock().expect("provider init log lock"),
        vec!["provider.alpha", "provider.middle", "provider.zeta"]
    );
    assert!(lifecycle
        .provider_states
        .values()
        .all(|state| *state == ProviderState::Available));
    assert_eq!(
        lifecycle
            .require_feature(&feature_id("flint.alpha"))
            .expect("alpha is ready")
            .health(),
        FeatureHealth::Healthy
    );
    assert_eq!(lifecycle.start(), Err(LifecycleError::AlreadyStarted));
    assert_eq!(initialized_features(&logs).len(), 4);
    assert_eq!(core_calls.load(Ordering::SeqCst), 0);

    let revision_before_shutdown = lifecycle.revision();
    lifecycle.shutdown().expect("normal shutdown");
    assert_eq!(
        shut_down_features(&logs),
        vec!["flint.alpha", "flint.zeta", "flint.gamma", "flint.beta"]
    );
    assert_eq!(
        *provider_shutdown_log
            .lock()
            .expect("provider shutdown log lock"),
        vec!["provider.zeta", "provider.middle", "provider.alpha"]
    );
    assert_eq!(core_calls.load(Ordering::SeqCst), 1);
    assert_eq!(lifecycle.host_state(), HostState::Stopped);
    assert_eq!(
        lifecycle.host_state_history(),
        &[
            HostState::Starting,
            HostState::Ready,
            HostState::Stopping,
            HostState::Stopped,
        ]
    );
    assert!(lifecycle.revision() > revision_before_shutdown);
    assert!(lifecycle
        .snapshot()
        .features()
        .iter()
        .all(|feature| feature.state == FeatureState::Stopped));
    assert!(lifecycle
        .diagnostics()
        .iter()
        .any(|diagnostic| diagnostic.contains("provider shutdown failed")));
    assert!(lifecycle
        .provider_states
        .values()
        .all(|state| *state == ProviderState::Stopped));

    lifecycle.shutdown().expect("idempotent shutdown");
    assert_eq!(core_calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        shut_down_features(&logs),
        vec!["flint.alpha", "flint.zeta", "flint.gamma", "flint.beta"]
    );
}

#[test]
fn duplicate_provider_ids_fail_before_any_initialization() {
    let logs = RuntimeLogs::default();
    let core_calls = Arc::new(AtomicUsize::new(0));
    let catalog = TestProviderCatalog::new(&["provider.same", "provider.same"]);
    let provider_init_log = catalog.initialized_providers.clone();

    let registry = FeatureRegistry::new(
        ApiVersion::new(1, 0),
        vec![registration(
            descriptor("flint.required", StartupPolicy::Required),
            &logs,
            None,
        )],
    )
    .expect("valid feature registry");

    let mut lifecycle =
        LifecycleCoordinator::new(registry, Box::new(catalog), cleanup_counter(&core_calls));

    assert_eq!(lifecycle.start(), Ok(StartupOutcome::StartupFailed));
    assert_eq!(lifecycle.host_state(), HostState::StartupFailed);
    assert!(initialized_features(&logs).is_empty());
    assert!(provider_init_log
        .lock()
        .expect("provider init log lock")
        .is_empty());
    assert_eq!(
        lifecycle.feature_state(&feature_id("flint.required")),
        Some(FeatureState::Stopped)
    );
    assert_eq!(core_calls.load(Ordering::SeqCst), 1);
}

#[test]
fn required_startup_failure_cleans_up_without_masking_the_original_failure() {
    let logs = RuntimeLogs::default();
    let core_calls = Arc::new(AtomicUsize::new(0));
    let cleanup_calls = core_calls.clone();
    let cleanup: Option<CoreCleanup> = Some(Box::new(move || {
        cleanup_calls.fetch_add(1, Ordering::SeqCst);
        Err("core cleanup failed with token=hidden".to_string())
    }));
    let mut catalog = TestProviderCatalog::new(&["provider.zeta", "provider.alpha"]);
    let provider_shutdown_log = catalog.shut_down_providers.clone();

    let mut dependent = descriptor("flint.dependent", StartupPolicy::Optional);
    dependent.requires = vec![feature_requirement("flint.required", "^1.0.0")];

    let registry = FeatureRegistry::new(
        ApiVersion::new(1, 0),
        vec![
            registration(
                descriptor("flint.required", StartupPolicy::Required),
                &logs,
                Some("initialization failed\nwith secret=hidden"),
            ),
            registration(dependent, &logs, None),
            registration(
                descriptor("flint.independent", StartupPolicy::Required),
                &logs,
                None,
            ),
        ],
    )
    .expect("valid feature registry");

    let mut lifecycle =
        LifecycleCoordinator::new(registry, Box::new(catalog), cleanup);

    assert_eq!(
        lifecycle.start(),
        Ok(StartupOutcome::StartupFailed)
    );
    assert_eq!(lifecycle.host_state(), HostState::StartupFailed);
    assert_eq!(
        lifecycle.host_state_history(),
        &[HostState::Starting, HostState::StartupFailed]
    );
    assert_eq!(
        initialized_features(&logs),
        vec!["flint.independent", "flint.required"]
    );
    assert_eq!(
        lifecycle.feature_state(&feature_id("flint.required")),
        Some(FeatureState::Failed)
    );
    assert_eq!(
        lifecycle.feature_state(&feature_id("flint.dependent")),
        Some(FeatureState::BlockedDependency)
    );
    assert_eq!(
        lifecycle.feature_state(&feature_id("flint.independent")),
        Some(FeatureState::Stopped)
    );
    assert_eq!(
        shut_down_features(&logs),
        vec!["flint.independent"]
    );
    assert_eq!(
        *provider_shutdown_log
            .lock()
            .expect("provider shutdown log lock"),
        vec!["provider.zeta", "provider.alpha"]
    );
    assert_eq!(core_calls.load(Ordering::SeqCst), 1);
    assert!(lifecycle
        .diagnostics()
        .iter()
        .any(|diagnostic| diagnostic.contains("core cleanup failed")));

    let last_failure = lifecycle.last_failure().expect("failure recorded");
    assert!(last_failure.contains("flint.required"));
    let snapshot_debug = format!("{:?}", lifecycle.snapshot()).to_ascii_lowercase();
    for leaked in ["secret", "token", "hidden"] {
        assert!(!last_failure.contains(leaked), "leaked '{leaked}'");
        assert!(!snapshot_debug.contains(leaked), "snapshot leaked '{leaked}'");
    }

    lifecycle.shutdown().expect("startup-failed shutdown is idempotent");
    assert_eq!(lifecycle.host_state(), HostState::StartupFailed);
    assert_eq!(core_calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        lifecycle.host_state_history(),
        &[HostState::Starting, HostState::StartupFailed]
    );
}

#[test]
fn optional_startup_failure_degrades_and_keeps_required_features_ready() {
    let logs = RuntimeLogs::default();
    let core_calls = Arc::new(AtomicUsize::new(0));
    let catalog = TestProviderCatalog::new(&[]);

    let mut required = descriptor("flint.required", StartupPolicy::Required);
    required
        .contributions
        .push(contribution(&required, "service", ContributionKind::Service));

    let mut optional = descriptor("flint.optional", StartupPolicy::Optional);
    optional
        .contributions
        .push(contribution(&optional, "service", ContributionKind::Service));

    let mut dependent = descriptor("flint.dependent", StartupPolicy::Optional);
    dependent.requires = vec![feature_requirement("flint.optional", "^1.0.0")];
    dependent
        .contributions
        .push(contribution(&dependent, "event", ContributionKind::Event));

    let mut side = descriptor("flint.side", StartupPolicy::Optional);
    side.contributions
        .push(contribution(&side, "capability", ContributionKind::Capability));

    let registry = FeatureRegistry::new(
        ApiVersion::new(1, 0),
        vec![
            registration(required, &logs, None),
            registration(optional, &logs, Some("optional init failed")),
            registration(dependent, &logs, None),
            registration(side, &logs, None),
        ],
    )
    .expect("valid feature registry");

    let mut lifecycle =
        LifecycleCoordinator::new(registry, Box::new(catalog), cleanup_counter(&core_calls));

    assert_eq!(lifecycle.start(), Ok(StartupOutcome::Degraded));
    assert_eq!(lifecycle.host_state(), HostState::Degraded);
    assert_eq!(
        initialized_features(&logs),
        vec!["flint.optional", "flint.required", "flint.side"]
    );
    assert_eq!(
        lifecycle.feature_state(&feature_id("flint.required")),
        Some(FeatureState::Ready)
    );
    assert_eq!(
        lifecycle.feature_state(&feature_id("flint.optional")),
        Some(FeatureState::Failed)
    );
    assert_eq!(
        lifecycle.feature_state(&feature_id("flint.dependent")),
        Some(FeatureState::BlockedDependency)
    );
    assert_eq!(
        lifecycle.feature_state(&feature_id("flint.side")),
        Some(FeatureState::Ready)
    );
    assert!(lifecycle
        .require_feature(&feature_id("flint.required"))
        .is_ok());
    assert!(lifecycle
        .require_feature(&feature_id("flint.side"))
        .is_ok());
    let unavailable = match lifecycle.require_feature(&feature_id("flint.optional")) {
        Ok(_) => panic!("failed feature must be gated"),
        Err(error) => error,
    };
    assert_eq!(unavailable.feature_id(), &feature_id("flint.optional"));

    let active_contributions = lifecycle.active_contributions().collect::<Vec<_>>();
    assert_eq!(
        active_contributions,
        vec![
            "flint.required.service",
            "flint.side.capability",
        ]
    );

    let snapshot = lifecycle.snapshot();
    assert_eq!(snapshot.host_state, HostState::Degraded);
    assert_eq!(
        snapshot
            .features()
            .iter()
            .map(|feature| feature.feature_id.as_str())
            .collect::<Vec<_>>(),
        vec![
            "flint.dependent",
            "flint.optional",
            "flint.required",
            "flint.side",
        ]
    );
    assert!(snapshot
        .last_failure
        .as_deref()
        .expect("degradation failure")
        .contains("flint.optional"));
    assert_eq!(core_calls.load(Ordering::SeqCst), 0);

    lifecycle.shutdown().expect("normal degraded shutdown");
    assert_eq!(lifecycle.host_state(), HostState::Stopped);
    assert_eq!(core_calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        lifecycle.host_state_history(),
        &[
            HostState::Starting,
            HostState::Degraded,
            HostState::Stopping,
            HostState::Stopped,
        ]
    );
    assert_eq!(
        lifecycle.feature_state(&feature_id("flint.required")),
        Some(FeatureState::Stopped)
    );
    assert_eq!(
        lifecycle.feature_state(&feature_id("flint.side")),
        Some(FeatureState::Stopped)
    );
    assert_eq!(
        lifecycle.feature_state(&feature_id("flint.optional")),
        Some(FeatureState::Failed)
    );
    assert_eq!(
        lifecycle.feature_state(&feature_id("flint.dependent")),
        Some(FeatureState::BlockedDependency)
    );
}

#[test]
fn required_dependent_makes_an_optional_root_failure_fatal() {
    let logs = RuntimeLogs::default();
    let core_calls = Arc::new(AtomicUsize::new(0));
    let catalog = TestProviderCatalog::new(&[]);

    let mut required_dependent =
        descriptor("flint.required-dependent", StartupPolicy::Required);
    required_dependent.requires = vec![feature_requirement("flint.optional", "^1.0.0")];

    let registry = FeatureRegistry::new(
        ApiVersion::new(1, 0),
        vec![
            registration(
                descriptor("flint.optional", StartupPolicy::Optional),
                &logs,
                Some("optional root failed"),
            ),
            registration(required_dependent, &logs, None),
            registration(
                descriptor("flint.side", StartupPolicy::Optional),
                &logs,
                None,
            ),
        ],
    )
    .expect("valid feature registry");

    let mut lifecycle =
        LifecycleCoordinator::new(registry, Box::new(catalog), cleanup_counter(&core_calls));

    assert_eq!(
        lifecycle.start(),
        Ok(StartupOutcome::StartupFailed)
    );
    assert_eq!(lifecycle.host_state(), HostState::StartupFailed);
    assert_eq!(
        lifecycle.feature_state(&feature_id("flint.optional")),
        Some(FeatureState::Failed)
    );
    assert_eq!(
        lifecycle.feature_state(&feature_id("flint.required-dependent")),
        Some(FeatureState::BlockedDependency)
    );
    assert_eq!(
        lifecycle.feature_state(&feature_id("flint.side")),
        Some(FeatureState::Stopped)
    );
    assert_eq!(initialized_features(&logs), vec!["flint.optional"]);
    assert_eq!(core_calls.load(Ordering::SeqCst), 1);
}

#[test]
fn provider_id_grammar_matches_provider_api_stable_identifiers() {
    for valid in [
        "provider",
        "provider.alpha",
        "provider-alpha",
        "provider_alpha",
        "provider:alpha",
        "a0",
    ] {
        assert_eq!(
            stable_provider_id_error(valid),
            None,
            "rejected valid provider ID {valid:?}"
        );
    }
    for invalid in [
        "",
        "Provider.alpha",
        "0provider",
        "provider alpha",
        "provider/path",
        "provider..alpha",
        "provider.",
        "provider\npath",
        "provider.Good",
    ] {
        assert!(
            stable_provider_id_error(invalid).is_some(),
            "accepted invalid provider ID {invalid:?}"
        );
    }

    let max_length = "a".repeat(MAX_STABLE_PROVIDER_ID_BYTES);
    assert_eq!(stable_provider_id_error(&max_length), None);
    assert!(stable_provider_id_error(&format!("{max_length}a")).is_some());
}

#[test]
fn invalid_provider_ids_fail_before_any_provider_or_feature_initialization() {
    for invalid_id in [
        "Provider.good",
        "provider/path",
        "provider id",
        "provider..bad",
    ] {
        let logs = RuntimeLogs::default();
        let core_calls = Arc::new(AtomicUsize::new(0));
        let catalog = TestProviderCatalog::new(&[invalid_id]);
        let provider_init_log = catalog.initialized_providers.clone();
        let marked_unavailable_log = catalog.marked_unavailable.clone();

        let registry = FeatureRegistry::new(
            ApiVersion::new(1, 0),
            vec![registration(
                descriptor("flint.required", StartupPolicy::Required),
                &logs,
                None,
            )],
        )
        .expect("valid feature registry");
        let mut lifecycle =
            LifecycleCoordinator::new(registry, Box::new(catalog), cleanup_counter(&core_calls));

        assert_eq!(
            lifecycle.start(),
            Ok(StartupOutcome::StartupFailed),
            "provider ID {invalid_id:?}"
        );
        assert_eq!(lifecycle.host_state(), HostState::StartupFailed);
        assert!(
            provider_init_log
                .lock()
                .expect("provider init log lock")
                .is_empty(),
            "provider initialized for invalid ID {invalid_id:?}"
        );
        assert!(
            initialized_features(&logs).is_empty(),
            "feature initialized for invalid ID {invalid_id:?}"
        );
        assert!(marked_unavailable_log
            .lock()
            .expect("marked unavailable log lock")
            .is_empty());
        assert!(lifecycle.provider_states.is_empty());
        assert_eq!(
            lifecycle.feature_state(&feature_id("flint.required")),
            Some(FeatureState::Stopped)
        );
        assert_eq!(core_calls.load(Ordering::SeqCst), 1);
    }
}

#[test]
fn provider_init_failure_forces_catalog_unavailability_before_feature_checks() {
    let logs = RuntimeLogs::default();
    let core_calls = Arc::new(AtomicUsize::new(0));
    let catalog = TestProviderCatalog::new(&["provider.failed"])
        .with_capability("mods.catalog", "provider.failed")
        .with_unmarked_failed_init("provider.failed");
    let provider_init_log = catalog.initialized_providers.clone();
    let marked_unavailable_log = catalog.marked_unavailable.clone();

    let mut required = descriptor("flint.required", StartupPolicy::Required);
    required.provider_requirements = vec![provider_requirement("mods.catalog")];

    let registry = FeatureRegistry::new(
        ApiVersion::new(1, 0),
        vec![registration(required, &logs, None)],
    )
    .expect("valid feature registry");

    let mut lifecycle =
        LifecycleCoordinator::new(registry, Box::new(catalog), cleanup_counter(&core_calls));

    assert_eq!(lifecycle.start(), Ok(StartupOutcome::StartupFailed));
    assert_eq!(lifecycle.host_state(), HostState::StartupFailed);
    assert_eq!(
        lifecycle.provider_states.get("provider.failed"),
        Some(&ProviderState::Failed)
    );
    assert_eq!(
        *provider_init_log.lock().expect("provider init log lock"),
        vec!["provider.failed"]
    );
    assert_eq!(
        *marked_unavailable_log
            .lock()
            .expect("marked unavailable log lock"),
        vec!["provider.failed"]
    );
    assert!(initialized_features(&logs).is_empty());
    assert_eq!(
        lifecycle.feature_state(&feature_id("flint.required")),
        Some(FeatureState::Failed)
    );
    assert_eq!(core_calls.load(Ordering::SeqCst), 1);
}

#[test]
fn provider_absence_failure_and_ambiguity_block_features_without_leaking_ids() {
    let logs = RuntimeLogs::default();
    let core_calls = Arc::new(AtomicUsize::new(0));
    let mut catalog = TestProviderCatalog::new(&["provider.bad", "provider.good"])
        .with_capability("mods.failed", "provider.bad")
        .with_failed_init("provider.bad")
        .with_ambiguous_capability("mods.ambiguous");
    let provider_init_log = catalog.initialized_providers.clone();
    let provider_shutdown_log = catalog.shut_down_providers.clone();

    let mut failed = descriptor("flint.failed-provider", StartupPolicy::Optional);
    failed.provider_requirements = vec![provider_requirement("mods.failed")];
    let mut ambiguous = descriptor("flint.ambiguous", StartupPolicy::Optional);
    ambiguous.provider_requirements = vec![provider_requirement("mods.ambiguous")];
    let mut absent = descriptor("flint.absent", StartupPolicy::Optional);
    absent.provider_requirements = vec![provider_requirement("mods.absent")];

    let registry = FeatureRegistry::new(
        ApiVersion::new(1, 0),
        vec![
            registration(
                descriptor("flint.required", StartupPolicy::Required),
                &logs,
                None,
            ),
            registration(failed, &logs, None),
            registration(ambiguous, &logs, None),
            registration(absent, &logs, None),
        ],
    )
    .expect("valid feature registry");

    let mut lifecycle =
        LifecycleCoordinator::new(registry, Box::new(catalog), cleanup_counter(&core_calls));

    assert_eq!(lifecycle.start(), Ok(StartupOutcome::Degraded));
    assert_eq!(
        *provider_init_log.lock().expect("provider init log lock"),
        vec!["provider.bad", "provider.good"]
    );
    assert_eq!(
        lifecycle.provider_states.get("provider.bad"),
        Some(&ProviderState::Failed)
    );
    for feature in ["flint.failed-provider", "flint.ambiguous", "flint.absent"] {
        assert_eq!(
            lifecycle.feature_state(&feature_id(feature)),
            Some(FeatureState::BlockedDependency)
        );
    }
    assert_eq!(
        lifecycle.feature_state(&feature_id("flint.required")),
        Some(FeatureState::Ready)
    );
    assert_eq!(initialized_features(&logs), vec!["flint.required"]);

    let snapshot = lifecycle.snapshot();
    let serialized_shape = format!("{snapshot:?}").to_ascii_lowercase();
    for leaked in [
        "provider.bad",
        "provider.good",
        "provider.one",
        "provider.two",
        "token",
        "secret",
        "hidden",
    ] {
        assert!(!serialized_shape.contains(leaked), "leaked '{leaked}'");
    }

    lifecycle.shutdown().expect("shutdown after degraded startup");
    assert_eq!(
        *provider_shutdown_log
            .lock()
            .expect("provider shutdown log lock"),
        vec!["provider.good"]
    );
    assert_eq!(core_calls.load(Ordering::SeqCst), 1);
    assert!(lifecycle
        .diagnostics()
        .iter()
        .all(|diagnostic| !diagnostic.contains("token")));
}

#[test]
fn optional_runtime_failure_degrades_and_hides_affected_contributions() {
    let logs = RuntimeLogs::default();
    let core_calls = Arc::new(AtomicUsize::new(0));
    let catalog = TestProviderCatalog::new(&[]);

    let mut required = descriptor("flint.base", StartupPolicy::Required);
    required
        .contributions
        .push(contribution(&required, "service", ContributionKind::Service));

    let mut root = descriptor("flint.root", StartupPolicy::Optional);
    root.contributions
        .push(contribution(&root, "service", ContributionKind::Service));

    let mut dependent = descriptor("flint.dependent", StartupPolicy::Optional);
    dependent.requires = vec![feature_requirement("flint.root", "^1.0.0")];
    dependent
        .contributions
        .push(contribution(&dependent, "event", ContributionKind::Event));

    let mut side = descriptor("flint.side", StartupPolicy::Optional);
    side.contributions
        .push(contribution(&side, "capability", ContributionKind::Capability));

    let registry = FeatureRegistry::new(
        ApiVersion::new(1, 0),
        vec![
            registration(required, &logs, None),
            registration(root, &logs, None),
            registration(dependent, &logs, None),
            registration(side, &logs, None),
        ],
    )
    .expect("valid feature registry");

    let mut lifecycle =
        LifecycleCoordinator::new(registry, Box::new(catalog), cleanup_counter(&core_calls));

    assert_eq!(lifecycle.start(), Ok(StartupOutcome::Ready));
    let impact = lifecycle
        .fail_feature(
            &feature_id("flint.root"),
            "runtime failure\nwith token=hidden",
        )
        .expect("runtime failure accepted");
    assert!(matches!(impact, FailureImpact::Degraded { .. }));
    assert_eq!(
        impact
            .affected()
            .iter()
            .map(FeatureId::as_str)
            .collect::<Vec<_>>(),
        vec!["flint.dependent", "flint.root"]
    );
    assert_eq!(lifecycle.host_state(), HostState::Degraded);
    assert_eq!(
        lifecycle.feature_state(&feature_id("flint.base")),
        Some(FeatureState::Ready)
    );
    assert_eq!(
        lifecycle.feature_state(&feature_id("flint.side")),
        Some(FeatureState::Ready)
    );
    assert_eq!(
        lifecycle.feature_state(&feature_id("flint.root")),
        Some(FeatureState::Failed)
    );
    assert_eq!(
        lifecycle.feature_state(&feature_id("flint.dependent")),
        Some(FeatureState::BlockedDependency)
    );
    assert_eq!(
        shut_down_features(&logs),
        vec!["flint.dependent", "flint.root"]
    );
    assert!(lifecycle
        .require_feature(&feature_id("flint.base"))
        .is_ok());
    assert!(lifecycle
        .require_feature(&feature_id("flint.side"))
        .is_ok());
    assert!(lifecycle
        .require_feature(&feature_id("flint.root"))
        .is_err());
    assert!(lifecycle
        .require_feature(&feature_id("flint.dependent"))
        .is_err());
    assert_eq!(
        lifecycle.active_contributions().collect::<Vec<_>>(),
        vec!["flint.base.service", "flint.side.capability"]
    );
    assert!(!lifecycle
        .last_failure()
        .expect("runtime failure recorded")
        .contains("token"));
    assert_eq!(core_calls.load(Ordering::SeqCst), 0);

    lifecycle.shutdown().expect("degraded shutdown");
    assert_eq!(lifecycle.host_state(), HostState::Stopped);
    assert_eq!(
        shut_down_features(&logs),
        vec!["flint.dependent", "flint.root", "flint.side", "flint.base"]
    );
    assert_eq!(
        lifecycle.feature_state(&feature_id("flint.base")),
        Some(FeatureState::Stopped)
    );
    assert_eq!(
        lifecycle.feature_state(&feature_id("flint.side")),
        Some(FeatureState::Stopped)
    );
    assert_eq!(core_calls.load(Ordering::SeqCst), 1);
}

#[test]
fn required_runtime_failure_enters_runtime_failed_and_then_stops() {
    let logs = RuntimeLogs::default();
    let core_calls = Arc::new(AtomicUsize::new(0));
    let catalog = TestProviderCatalog::new(&[]);

    let mut dependent = descriptor("flint.dependent", StartupPolicy::Optional);
    dependent.requires = vec![feature_requirement("flint.required", "^1.0.0")];

    let registry = FeatureRegistry::new(
        ApiVersion::new(1, 0),
        vec![
            registration(
                descriptor("flint.required", StartupPolicy::Required),
                &logs,
                None,
            ),
            registration(dependent, &logs, None),
            registration(descriptor("flint.side", StartupPolicy::Optional), &logs, None),
        ],
    )
    .expect("valid feature registry");

    let mut lifecycle =
        LifecycleCoordinator::new(registry, Box::new(catalog), cleanup_counter(&core_calls));

    assert_eq!(lifecycle.start(), Ok(StartupOutcome::Ready));
    let impact = lifecycle
        .fail_feature(&feature_id("flint.required"), "required runtime failure")
        .expect("runtime failure accepted");
    assert!(matches!(impact, FailureImpact::Fatal { .. }));
    assert_eq!(lifecycle.host_state(), HostState::RuntimeFailed);
    assert_eq!(
        lifecycle.feature_state(&feature_id("flint.required")),
        Some(FeatureState::Failed)
    );
    assert_eq!(
        lifecycle.feature_state(&feature_id("flint.dependent")),
        Some(FeatureState::BlockedDependency)
    );
    assert_eq!(
        lifecycle.feature_state(&feature_id("flint.side")),
        Some(FeatureState::Ready)
    );
    assert_eq!(
        shut_down_features(&logs),
        vec!["flint.dependent", "flint.required"]
    );
    assert!(lifecycle
        .require_feature(&feature_id("flint.side"))
        .is_err());
    assert!(lifecycle
        .host_state_history()
        .contains(&HostState::RuntimeFailed));

    lifecycle.shutdown().expect("runtime-fatal shutdown");
    assert_eq!(lifecycle.host_state(), HostState::Stopped);
    assert_eq!(
        lifecycle.host_state_history(),
        &[
            HostState::Starting,
            HostState::Ready,
            HostState::RuntimeFailed,
            HostState::Stopping,
            HostState::Stopped,
        ]
    );
    assert_eq!(
        lifecycle.feature_state(&feature_id("flint.side")),
        Some(FeatureState::Stopped)
    );
    assert_eq!(
        shut_down_features(&logs),
        vec!["flint.dependent", "flint.required", "flint.side"]
    );
    assert_eq!(core_calls.load(Ordering::SeqCst), 1);
}

#[test]
fn optional_only_provider_loss_degrades_and_preserves_required_features() {
    let logs = RuntimeLogs::default();
    let core_calls = Arc::new(AtomicUsize::new(0));
    let mut catalog = TestProviderCatalog::new(&["provider.live"])
        .with_capability("mods.catalog", "provider.live");
    let provider_shutdown_log = catalog.shut_down_providers.clone();

    let mut provided = descriptor("flint.provided", StartupPolicy::Optional);
    provided.provider_requirements = vec![provider_requirement("mods.catalog")];
    let mut dependent = descriptor("flint.dependent", StartupPolicy::Optional);
    dependent.requires = vec![feature_requirement("flint.provided", "^1.0.0")];

    let registry = FeatureRegistry::new(
        ApiVersion::new(1, 0),
        vec![
            registration(
                descriptor("flint.required", StartupPolicy::Required),
                &logs,
                None,
            ),
            registration(provided, &logs, None),
            registration(dependent, &logs, None),
            registration(descriptor("flint.side", StartupPolicy::Optional), &logs, None),
        ],
    )
    .expect("valid feature registry");

    let mut lifecycle =
        LifecycleCoordinator::new(registry, Box::new(catalog), cleanup_counter(&core_calls));

    assert_eq!(lifecycle.start(), Ok(StartupOutcome::Ready));
    let impact = lifecycle
        .fail_provider("provider.live", "provider became unavailable")
        .expect("provider loss accepted");
    assert!(matches!(impact, FailureImpact::Degraded { .. }));
    assert_eq!(
        impact
            .affected()
            .iter()
            .map(FeatureId::as_str)
            .collect::<Vec<_>>(),
        vec!["flint.dependent", "flint.provided"]
    );
    assert_eq!(lifecycle.host_state(), HostState::Degraded);
    assert_eq!(
        lifecycle.feature_state(&feature_id("flint.required")),
        Some(FeatureState::Ready)
    );
    assert_eq!(
        lifecycle.feature_state(&feature_id("flint.side")),
        Some(FeatureState::Ready)
    );
    assert_eq!(
        lifecycle.feature_state(&feature_id("flint.provided")),
        Some(FeatureState::Failed)
    );
    assert_eq!(
        lifecycle.feature_state(&feature_id("flint.dependent")),
        Some(FeatureState::BlockedDependency)
    );
    assert!(lifecycle
        .require_feature(&feature_id("flint.required"))
        .is_ok());
    assert!(lifecycle
        .feature_health(&feature_id("flint.side"))
        .is_ok());
    assert!(lifecycle
        .require_feature(&feature_id("flint.provided"))
        .is_err());
    assert!(lifecycle
        .require_feature(&feature_id("flint.dependent"))
        .is_err());

    lifecycle.shutdown().expect("degraded provider-loss shutdown");
    assert_eq!(lifecycle.host_state(), HostState::Stopped);
    assert_eq!(
        *provider_shutdown_log
            .lock()
            .expect("provider shutdown log lock"),
        vec!["provider.live"]
    );
    assert_eq!(core_calls.load(Ordering::SeqCst), 1);
}

#[test]
fn provider_runtime_loss_fails_required_dependents_without_leaking_provider_ids() {
    let logs = RuntimeLogs::default();
    let core_calls = Arc::new(AtomicUsize::new(0));
    let mut catalog = TestProviderCatalog::new(&["provider.live"])
        .with_capability("mods.catalog", "provider.live");
    let provider_shutdown_log = catalog.shut_down_providers.clone();

    let mut required = descriptor("flint.required", StartupPolicy::Required);
    required.provider_requirements = vec![provider_requirement("mods.catalog")];
    let mut dependent = descriptor("flint.dependent", StartupPolicy::Optional);
    dependent.requires = vec![feature_requirement("flint.required", "^1.0.0")];

    let registry = FeatureRegistry::new(
        ApiVersion::new(1, 0),
        vec![
            registration(required, &logs, None),
            registration(dependent, &logs, None),
            registration(descriptor("flint.side", StartupPolicy::Optional), &logs, None),
        ],
    )
    .expect("valid feature registry");

    let mut lifecycle =
        LifecycleCoordinator::new(registry, Box::new(catalog), cleanup_counter(&core_calls));

    assert_eq!(lifecycle.start(), Ok(StartupOutcome::Ready));
    let impact = lifecycle
        .fail_provider("provider.live", "provider loss token=hidden")
        .expect("provider loss accepted");
    assert!(matches!(impact, FailureImpact::Fatal { .. }));
    assert_eq!(lifecycle.host_state(), HostState::RuntimeFailed);
    assert_eq!(
        lifecycle.provider_states.get("provider.live"),
        Some(&ProviderState::Failed)
    );
    assert_eq!(
        lifecycle.feature_state(&feature_id("flint.required")),
        Some(FeatureState::Failed)
    );
    assert_eq!(
        lifecycle.feature_state(&feature_id("flint.dependent")),
        Some(FeatureState::BlockedDependency)
    );
    assert_eq!(
        lifecycle.feature_state(&feature_id("flint.side")),
        Some(FeatureState::Ready)
    );

    let snapshot = lifecycle.snapshot();
    let last_failure = snapshot
        .last_failure
        .clone()
        .expect("provider failure recorded");
    let debug_snapshot = format!("{snapshot:?}").to_ascii_lowercase();
    for leaked in ["provider.live", "token", "secret", "hidden"] {
        assert!(!last_failure.to_ascii_lowercase().contains(leaked));
        assert!(!debug_snapshot.contains(leaked));
    }

    lifecycle.shutdown().expect("provider-loss shutdown");
    assert_eq!(lifecycle.host_state(), HostState::Stopped);
    assert_eq!(
        *provider_shutdown_log
            .lock()
            .expect("provider shutdown log lock"),
        vec!["provider.live"]
    );
    assert_eq!(core_calls.load(Ordering::SeqCst), 1);
}