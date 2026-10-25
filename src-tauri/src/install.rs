use crate::models::{InstallProgress, InstallResult};
use crate::vanilla::install::{self as feature_install, InstallProgressSink};
use tauri::{AppHandle, Emitter};

const INSTALL_PROGRESS_EVENT: &str = "flint://install-progress";

pub(crate) use crate::vanilla::install::fetch_remote_versions_blocking;

struct TauriInstallProgressSink<'a> {
    app: &'a AppHandle,
}

impl InstallProgressSink for TauriInstallProgressSink<'_> {
    fn report(&self, progress: InstallProgress) {
        let _ = self.app.emit(INSTALL_PROGRESS_EVENT, progress);
    }
}

pub fn install_blocking(
    app: &AppHandle,
    game_dir: &str,
    version_id: &str,
) -> Result<InstallResult, String> {
    let sink = TauriInstallProgressSink { app };
    feature_install::install_blocking(game_dir, version_id, &sink)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_install_progress_event_name() {
        assert_eq!(INSTALL_PROGRESS_EVENT, "flint://install-progress");
    }
}