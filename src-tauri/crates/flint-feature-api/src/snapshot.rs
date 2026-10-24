use serde::Serialize;

use crate::{FeatureState, HostState};

/// One lexicographically sortable feature availability entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FeatureAvailability {
    pub feature_id: String,
    pub state: FeatureState,
    pub reason: Option<String>,
}

impl FeatureAvailability {
    pub fn new(
        feature_id: impl Into<String>,
        state: FeatureState,
        reason: Option<String>,
    ) -> Self {
        Self {
            feature_id: feature_id.into(),
            state,
            reason,
        }
    }
}

/// Read-only host availability snapshot suitable for serialization to the shell.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FeatureAvailabilitySnapshot {
    pub revision: u64,
    pub host_state: HostState,
    pub last_failure: Option<String>,
    pub features: Vec<FeatureAvailability>,
}

impl FeatureAvailabilitySnapshot {
    pub fn new(
        revision: u64,
        host_state: HostState,
        last_failure: Option<String>,
        mut features: Vec<FeatureAvailability>,
    ) -> Self {
        features.sort_by(|left, right| left.feature_id.cmp(&right.feature_id));
        Self {
            revision,
            host_state,
            last_failure,
            features,
        }
    }

    pub fn features(&self) -> &[FeatureAvailability] {
        &self.features
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use serde_json::json;

    #[test]
    fn snapshot_sorts_features_and_retains_optional_reasons() {
        let snapshot = FeatureAvailabilitySnapshot::new(
            7,
            HostState::Degraded,
            Some("feature 'flint.optional': unavailable".to_string()),
            vec![
                FeatureAvailability::new(
                    "flint.zeta",
                    FeatureState::Ready,
                    None,
                ),
                FeatureAvailability::new(
                    "flint.alpha",
                    FeatureState::BlockedDependency,
                    Some("a required dependency is unavailable".to_string()),
                ),
            ],
        );

        assert_eq!(snapshot.revision, 7);
        assert_eq!(snapshot.host_state, HostState::Degraded);
        assert_eq!(
            snapshot
                .features()
                .iter()
                .map(|feature| feature.feature_id.as_str())
                .collect::<Vec<_>>(),
            vec!["flint.alpha", "flint.zeta"]
        );
        assert_eq!(
            snapshot.features()[0].reason.as_deref(),
            Some("a required dependency is unavailable")
        );
        assert_eq!(snapshot.features()[1].reason.as_deref(), None);
    }

    #[test]
    fn snapshot_json_has_exact_contract_field_and_state_names() {
        let snapshot = FeatureAvailabilitySnapshot::new(
            7,
            HostState::Degraded,
            Some("feature 'flint.optional': unavailable".to_string()),
            vec![
                FeatureAvailability::new("flint.zeta", FeatureState::Ready, None),
                FeatureAvailability::new(
                    "flint.alpha",
                    FeatureState::BlockedDependency,
                    Some("a required dependency is unavailable".to_string()),
                ),
            ],
        );

        assert_eq!(
            serde_json::to_value(&snapshot).expect("serialize availability snapshot"),
            json!({
                "revision": 7u64,
                "hostState": "degraded",
                "lastFailure": "feature 'flint.optional': unavailable",
                "features": [
                    {
                        "featureId": "flint.alpha",
                        "state": "blocked_dependency",
                        "reason": "a required dependency is unavailable"
                    },
                    {
                        "featureId": "flint.zeta",
                        "state": "ready",
                        "reason": null
                    }
                ]
            })
        );

        let null_fields = FeatureAvailabilitySnapshot::new(
            0,
            HostState::Ready,
            None,
            vec![FeatureAvailability::new(
                "flint.ready",
                FeatureState::Ready,
                None,
            )],
        );
        assert_eq!(
            serde_json::to_value(&null_fields).expect("serialize nullable fields"),
            json!({
                "revision": 0u64,
                "hostState": "ready",
                "lastFailure": null,
                "features": [
                    {
                        "featureId": "flint.ready",
                        "state": "ready",
                        "reason": null
                    }
                ]
            })
        );
    }

    #[test]
    fn snapshot_types_are_serializable_and_state_names_are_snake_case() {
        fn assert_serialize<T: Serialize>() {}

        assert_serialize::<FeatureAvailabilitySnapshot>();
        assert_serialize::<FeatureAvailability>();
        assert_eq!(HostState::Ready.as_str(), "ready");
        assert_eq!(FeatureState::Failed.as_str(), "failed");
    }
}