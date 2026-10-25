use crate::models::{
    InstallProgress, LoaderInstallProgress, LoaderInstallRequest, LoaderInstallResult, LoaderKind,
    LoaderVersionInfo,
};
use flint_feature_loaders::{LoaderInstallProgressSink, LoaderProgressSink};
use flint_provider_api::{LoaderProvider, ProviderRegistry};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};

const LOADER_INSTALL_PROGRESS_EVENT: &str = "flint://loader-install-progress";
const INSTALL_PROGRESS_EVENT: &str = "flint://install-progress";

struct TauriLoaderProgressSink<'a> {
    app: &'a AppHandle,
}

impl LoaderProgressSink for TauriLoaderProgressSink<'_> {
    fn report(&self, progress: LoaderInstallProgress) {
        let _ = self.app.emit(LOADER_INSTALL_PROGRESS_EVENT, progress);
    }
}

struct TauriVanillaProgressSink<'a> {
    app: &'a AppHandle,
}

impl LoaderInstallProgressSink for TauriVanillaProgressSink<'_> {
    fn report(&self, progress: InstallProgress) {
        let _ = self.app.emit(INSTALL_PROGRESS_EVENT, progress);
    }
}

pub fn list_loader_versions_blocking(
    registry: Arc<ProviderRegistry<dyn LoaderProvider>>,
    loader: LoaderKind,
    game_version: &str,
) -> Result<Vec<LoaderVersionInfo>, String> {
    flint_feature_loaders::list_loader_versions_blocking(registry, loader, game_version)
}

pub fn install_loader_blocking(
    app: &AppHandle,
    registry: Arc<ProviderRegistry<dyn LoaderProvider>>,
    request: &LoaderInstallRequest,
) -> Result<LoaderInstallResult, String> {
    let loader_sink = TauriLoaderProgressSink { app };
    let vanilla_sink = TauriVanillaProgressSink { app };
    flint_feature_loaders::install_loader_blocking(registry, request, &loader_sink, &vanilla_sink)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_loader_and_vanilla_progress_event_names() {
        assert_eq!(
            LOADER_INSTALL_PROGRESS_EVENT,
            "flint://loader-install-progress"
        );
        assert_eq!(INSTALL_PROGRESS_EVENT, "flint://install-progress");
    }
}