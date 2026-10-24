use serde::Serialize;

/// Host lifecycle state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HostState {
    Starting,
    Ready,
    Degraded,
    StartupFailed,
    RuntimeFailed,
    Stopping,
    Stopped,
}

impl HostState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Starting => "starting",
            Self::Ready => "ready",
            Self::Degraded => "degraded",
            Self::StartupFailed => "startup_failed",
            Self::RuntimeFailed => "runtime_failed",
            Self::Stopping => "stopping",
            Self::Stopped => "stopped",
        }
    }
}

/// Feature lifecycle state. There is intentionally no feature-level `Degraded` state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FeatureState {
    Starting,
    Ready,
    Failed,
    BlockedDependency,
    Stopped,
}

impl FeatureState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Starting => "starting",
            Self::Ready => "ready",
            Self::Failed => "failed",
            Self::BlockedDependency => "blocked_dependency",
            Self::Stopped => "stopped",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_names_are_stable_snake_case_values() {
        assert_eq!(
            [
                HostState::Starting,
                HostState::Ready,
                HostState::Degraded,
                HostState::StartupFailed,
                HostState::RuntimeFailed,
                HostState::Stopping,
                HostState::Stopped,
            ]
            .map(HostState::as_str),
            [
                "starting",
                "ready",
                "degraded",
                "startup_failed",
                "runtime_failed",
                "stopping",
                "stopped",
            ]
        );
        assert_eq!(
            [
                FeatureState::Starting,
                FeatureState::Ready,
                FeatureState::Failed,
                FeatureState::BlockedDependency,
                FeatureState::Stopped,
            ]
            .map(FeatureState::as_str),
            ["starting", "ready", "failed", "blocked_dependency", "stopped"]
        );
    }
}