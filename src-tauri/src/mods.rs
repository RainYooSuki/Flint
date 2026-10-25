use crate::models::{ModInstallProgress, ModInstallResult, ModSource};
use flint_feature_mods::ModInstallProgressSink;
use flint_provider_api::{ModProvider, ProviderRegistry};
use tauri::{AppHandle, Emitter};

const MOD_INSTALL_PROGRESS_EVENT: &str = "flint://mod-install-progress";

pub(crate) use flint_feature_mods::{
    delete_mod_blocking, list_installed_mods_blocking, list_mod_versions_blocking,
    resolve_mod_dependencies_blocking, search_mods_blocking, set_mod_enabled_blocking,
};

struct TauriModInstallProgressSink<'a> {
    app: &'a AppHandle,
}

impl ModInstallProgressSink for TauriModInstallProgressSink<'_> {
    fn report(&self, progress: ModInstallProgress) {
        let _ = self.app.emit(MOD_INSTALL_PROGRESS_EVENT, progress);
    }
}

pub fn install_mod_blocking(
    registry: &ProviderRegistry<dyn ModProvider>,
    app: &AppHandle,
    game_dir: &str,
    source: ModSource,
    project_id: &str,
    version_id: &str,
    game_version: &str,
    loader: &str,
    install_dependencies: bool,
) -> Result<ModInstallResult, String> {
    let sink = TauriModInstallProgressSink { app };
    flint_feature_mods::install_mod_blocking(
        registry,
        &sink,
        game_dir,
        source,
        project_id,
        version_id,
        game_version,
        loader,
        install_dependencies,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_mod_progress_event_name() {
        assert_eq!(MOD_INSTALL_PROGRESS_EVENT, "flint://mod-install-progress");
    }
}