use crate::{
    install,
    java,
    models::{
        LoaderInstallProgress, LoaderInstallRequest, LoaderInstallResult, LoaderKind,
        LoaderVersionInfo,
    },
    paths::{component_is_safe, resolve_game_dir},
    version,
};
use flint_core::ports::{
    Cancellation, CommandProcessPort, Deadline, NoopLogger, NoopProgress, ProcessJavaRunner,
    ProgressSink, ProgressUpdate, ReqwestNetwork, SafeWorkspace,
};
use flint_provider_api::{
    loader_provider_requirement, LoaderInstallContext, LoaderInstallRequest as TypedLoaderInstallRequest,
    LoaderListContext,
    LoaderListRequest as TypedLoaderListRequest, LoaderProvider, LoaderTimeouts,
    LoaderVersionData, ProviderRegistry, ProviderRequirement,
    FABRIC_PROVIDER_ID, FABRIC_SELECTION_KEY, FORGE_PROVIDER_ID, FORGE_SELECTION_KEY,
    LITELOADER_PROVIDER_ID, LITELOADER_SELECTION_KEY, LOADER_INSTALLER_HTTP_TIMEOUT,
    LOADER_JAVA_PROCESS_TIMEOUT, LOADER_METADATA_TIMEOUT, NEOFORGE_PROVIDER_ID,
    NEOFORGE_SELECTION_KEY, OPTIFINE_PROVIDER_ID, OPTIFINE_SELECTION_KEY, QUILT_PROVIDER_ID,
    QUILT_SELECTION_KEY,
};
use std::{fs, path::Path, sync::Arc};
use tauri::{AppHandle, Emitter};

const LOADER_INSTALL_PROGRESS_EVENT: &str = "flint://loader-install-progress";

#[allow(dead_code)]
struct LoaderBinding {
    requirement: ProviderRequirement,
    provider_id: &'static str,
    display_name: &'static str,
}

/// The sole serialized `LoaderKind` to provider-binding mapping.
fn loader_binding(loader: LoaderKind) -> LoaderBinding {
    match loader {
        LoaderKind::Fabric => LoaderBinding {
            requirement: loader_provider_requirement(Some(FABRIC_SELECTION_KEY)),
            provider_id: FABRIC_PROVIDER_ID,
            display_name: "Fabric",
        },
        LoaderKind::Forge => LoaderBinding {
            requirement: loader_provider_requirement(Some(FORGE_SELECTION_KEY)),
            provider_id: FORGE_PROVIDER_ID,
            display_name: "Forge",
        },
        LoaderKind::Liteloader => LoaderBinding {
            requirement: loader_provider_requirement(Some(LITELOADER_SELECTION_KEY)),
            provider_id: LITELOADER_PROVIDER_ID,
            display_name: "LiteLoader",
        },
        LoaderKind::Neoforge => LoaderBinding {
            requirement: loader_provider_requirement(Some(NEOFORGE_SELECTION_KEY)),
            provider_id: NEOFORGE_PROVIDER_ID,
            display_name: "NeoForge",
        },
        LoaderKind::Optifine => LoaderBinding {
            requirement: loader_provider_requirement(Some(OPTIFINE_SELECTION_KEY)),
            provider_id: OPTIFINE_PROVIDER_ID,
            display_name: "OptiFine",
        },
        LoaderKind::Quilt => LoaderBinding {
            requirement: loader_provider_requirement(Some(QUILT_SELECTION_KEY)),
            provider_id: QUILT_PROVIDER_ID,
            display_name: "Quilt",
        },
    }
}

fn select_loader_provider<'a>(
    registry: &'a ProviderRegistry<dyn LoaderProvider>,
    loader: LoaderKind,
) -> Result<&'a dyn LoaderProvider, String> {
    let binding = loader_binding(loader);
    registry.select(&binding.requirement).map_err(|error| {
        format!(
            "Cannot select the {} loader provider: {error}",
            binding.provider_id
        )
    })
}

fn loader_timeouts() -> LoaderTimeouts {
    LoaderTimeouts::new(
        LOADER_METADATA_TIMEOUT,
        LOADER_INSTALLER_HTTP_TIMEOUT,
        LOADER_JAVA_PROCESS_TIMEOUT,
    )
}

fn loader_deadline() -> Deadline {
    Deadline::none()
}

fn map_loader_version(loader: LoaderKind, version: LoaderVersionData) -> LoaderVersionInfo {
    LoaderVersionInfo {
        loader,
        version: version.version,
        label: version.label,
        recommended: version.recommended,
        game_version: version.game_version,
        release_time: version.release_time,
    }
}

fn loader_install_result(
    request: &LoaderInstallRequest,
    root: &Path,
    installed_version_id: String,
    log_path: Option<String>,
) -> LoaderInstallResult {
    let install_path = version::version_dir(root, &installed_version_id);
    LoaderInstallResult {
        game_dir: root.display().to_string(),
        game_version: request.game_version.clone(),
        loader: request.loader,
        loader_version: request.loader_version.clone(),
        installed_version_id,
        install_path: install_path.display().to_string(),
        log_path,
    }
}

fn loader_progress_stage(message: &str) -> &'static str {
    if message == "Writing loader version profile" || message == "Loader version profile written" {
        return "finalize";
    }
    if message == "Loader installation complete" || message.ends_with(" installation complete") {
        return "complete";
    }
    if (message.starts_with("Fetching ") && message.ends_with(" profile"))
        || message.ends_with(" profile downloaded")
    {
        return "profile";
    }
    if (message.starts_with("Downloading ") && message.ends_with(" libraries"))
        || message.starts_with("Downloaded library ")
        || message == "Downloading LiteLoader"
        || message == "LiteLoader downloaded"
        || message.starts_with("Downloaded LiteLoader dependency ")
    {
        return "libraries";
    }
    if (message.starts_with("Downloading ") && message.ends_with(" installer"))
        || (message.starts_with("Running ") && message.ends_with(" installer"))
        || message.ends_with(" installer finished")
    {
        return "installer";
    }
    "provider"
}

fn loader_progress_event(
    request: &LoaderInstallRequest,
    stage: &str,
    completed: u64,
    total: u64,
    message: &str,
) -> LoaderInstallProgress {
    LoaderInstallProgress {
        loader: request.loader,
        game_version: request.game_version.clone(),
        loader_version: request.loader_version.clone(),
        stage: stage.to_string(),
        completed,
        total,
        message: message.to_string(),
    }
}

fn emit_loader_progress_event(
    app: &AppHandle,
    request: &LoaderInstallRequest,
    stage: &str,
    completed: u64,
    total: u64,
    message: &str,
) {
    let _ = app.emit(
        LOADER_INSTALL_PROGRESS_EVENT,
        loader_progress_event(request, stage, completed, total, message),
    );
}

struct TauriLoaderProgressSink<'a> {
    app: &'a AppHandle,
    request: &'a LoaderInstallRequest,
}

impl<'a> TauriLoaderProgressSink<'a> {
    fn new(app: &'a AppHandle, request: &'a LoaderInstallRequest) -> Self {
        Self { app, request }
    }

    fn emit(&self, stage: &str, completed: u64, total: u64, message: &str) {
        emit_loader_progress_event(self.app, self.request, stage, completed, total, message);
    }
}

impl ProgressSink for TauriLoaderProgressSink<'_> {
    fn report(&self, update: ProgressUpdate) {
        let message = update.message.as_deref().unwrap_or_default();
        let stage = loader_progress_stage(message);
        let total = update.total.unwrap_or(update.completed.max(1));
        self.emit(stage, update.completed, total, message);
    }
}

pub fn list_loader_versions_blocking(
    registry: Arc<ProviderRegistry<dyn LoaderProvider>>,
    loader: LoaderKind,
    game_version: &str,
) -> Result<Vec<LoaderVersionInfo>, String> {
    validate_game_version(game_version)?;
    let metadata_network = ReqwestNetwork::new(LOADER_METADATA_TIMEOUT)?;
    let provider = select_loader_provider(&registry, loader)?;
    let workspace = SafeWorkspace;
    let progress = NoopProgress;
    let logger = NoopLogger;
    let context = LoaderListContext::new(
        &metadata_network,
        &workspace,
        &progress,
        &logger,
        loader_timeouts(),
        loader_deadline(),
        Cancellation::new(),
    );
    let request = TypedLoaderListRequest::new(game_version);
    let versions = provider.list_versions(&context, &request)?;
    Ok(versions
        .into_iter()
        .map(|version| map_loader_version(loader, version))
        .collect())
}

pub fn install_loader_blocking(
    app: &AppHandle,
    registry: Arc<ProviderRegistry<dyn LoaderProvider>>,
    request: &LoaderInstallRequest,
) -> Result<LoaderInstallResult, String> {
    validate_game_version(&request.game_version)?;
    if !component_is_safe(request.loader_version.trim()) {
        return Err("The selected loader version is invalid.".to_string());
    }
    let root = resolve_game_dir(&request.game_dir);
    fs::create_dir_all(&root)
        .map_err(|error| format!("Cannot create game directory ({}): {error}", root.display()))?;

    let progress = TauriLoaderProgressSink::new(app, request);
    progress.emit(
        "prepare",
        0,
        1,
        "Preparing loader installation",
    );
    ensure_vanilla_base(app, request, &root, &progress)?;
    progress.emit("prepare", 1, 1, "Vanilla base version is ready");

    let resolved_java_path = resolve_loader_java_path(&request.java_path)?;
    let metadata_network = ReqwestNetwork::new(LOADER_METADATA_TIMEOUT)?;
    let installer_network = ReqwestNetwork::new(LOADER_INSTALLER_HTTP_TIMEOUT)?;
    let provider = select_loader_provider(&registry, request.loader)?;
    let workspace = SafeWorkspace;
    let process = CommandProcessPort;
    let java = ProcessJavaRunner::new(&process);
    let logger = NoopLogger;
    let typed_request = TypedLoaderInstallRequest::new(
        request.game_version.clone(),
        request.loader_version.clone(),
        resolved_java_path,
    );
    let context = LoaderInstallContext::new(
        &metadata_network,
        &installer_network,
        &workspace,
        &java,
        &root,
        &progress,
        &logger,
        loader_timeouts(),
        loader_deadline(),
        Cancellation::new(),
    );
    let output = provider.install(&context, &typed_request)?;
    let installed_version_id = output.installed_version_id;
    let log_path = output.log_path;

    if !version::version_json_path(&root, &installed_version_id).is_file() {
        return Err(format!(
            "Loader installation did not produce version profile {installed_version_id}."
        ));
    }
    progress.emit("complete", 1, 1, "Loader installation complete");

    Ok(loader_install_result(
        request,
        &root,
        installed_version_id,
        log_path,
    ))
}

fn resolve_loader_java_path(value: &str) -> Result<String, String> {
    java::resolve_java_path(value).map(|path| path.display().to_string())
}

fn validate_game_version(game_version: &str) -> Result<(), String> {
    if component_is_safe(game_version) {
        Ok(())
    } else {
        Err("The Minecraft version id is invalid.".to_string())
    }
}

fn ensure_vanilla_base(
    app: &AppHandle,
    request: &LoaderInstallRequest,
    root: &Path,
    progress: &TauriLoaderProgressSink<'_>,
) -> Result<(), String> {
    let metadata = version::version_json_path(root, &request.game_version);
    let client_jar = version::client_jar_path(root, &request.game_version);
    if metadata.is_file() && client_jar.is_file() {
        return Ok(());
    }
    progress.emit(
        "vanilla",
        0,
        1,
        "Installing the required vanilla version",
    );
    let game_dir = root.display().to_string();
    install::install_blocking(app, &game_dir, &request.game_version)?;
    progress.emit("vanilla", 1, 1, "Vanilla version installed");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider_composition::LoaderProviderState;
    use flint_provider_api::{LoaderInstallOutputData, Provider as _};
    use serde_json::json;
    use std::time::Duration;

    fn install_request() -> LoaderInstallRequest {
        LoaderInstallRequest {
            game_dir: "game".to_string(),
            game_version: "1.20.1".to_string(),
            loader: LoaderKind::Fabric,
            loader_version: "0.15.7".to_string(),
            java_path: "java".to_string(),
        }
    }

    #[test]
    fn one_loader_binding_selects_each_exact_composed_provider() {
        let state = LoaderProviderState::new();
        let registry = state.registry().expect("loader provider registry");
        let expected = [
            (LoaderKind::Fabric, "fabric", "loader-kind:fabric", "Fabric"),
            (LoaderKind::Forge, "forge", "loader-kind:forge", "Forge"),
            (
                LoaderKind::Liteloader,
                "liteloader",
                "loader-kind:liteloader",
                "LiteLoader",
            ),
            (
                LoaderKind::Neoforge,
                "neoforge",
                "loader-kind:neoforge",
                "NeoForge",
            ),
            (
                LoaderKind::Optifine,
                "optifine",
                "loader-kind:optifine",
                "OptiFine",
            ),
            (LoaderKind::Quilt, "quilt", "loader-kind:quilt", "Quilt"),
        ];
        for (loader, provider_id, selection_key, display_name) in expected {
            let binding = loader_binding(loader);
            assert_eq!(binding.provider_id, provider_id);
            assert_eq!(binding.display_name, display_name);
            assert_eq!(
                binding.requirement.capability,
                flint_provider_api::LOADER_INSTALLATION_CAPABILITY_ID
            );
            assert_eq!(binding.requirement.selection_key, Some(selection_key));
            let provider = select_loader_provider(&registry, loader).expect("select provider");
            assert_eq!(provider.descriptor().id, provider_id);
        }
    }

    #[test]
    fn injects_and_reuses_the_composed_registry() {
        let state = LoaderProviderState::new();
        let first = state.registry().expect("loader provider registry");
        let second = state.registry().expect("loader provider registry");
        assert!(Arc::ptr_eq(&first, &second));
        assert_eq!(
            first.provider_ids().collect::<Vec<_>>(),
            vec!["fabric", "forge", "liteloader", "neoforge", "optifine", "quilt"]
        );
    }

    #[test]
    fn maps_neutral_loader_version_to_unchanged_host_dto() {
        let data = LoaderVersionData {
            version: "0.15.7".to_string(),
            label: "Fabric Loader 0.15.7".to_string(),
            recommended: true,
            game_version: "1.20.1".to_string(),
            release_time: Some("2024-01-01T00:00:00Z".to_string()),
        };
        let mapped = map_loader_version(LoaderKind::Fabric, data);
        assert_eq!(
            mapped,
            LoaderVersionInfo {
                loader: LoaderKind::Fabric,
                version: "0.15.7".to_string(),
                label: "Fabric Loader 0.15.7".to_string(),
                recommended: true,
                game_version: "1.20.1".to_string(),
                release_time: Some("2024-01-01T00:00:00Z".to_string()),
            }
        );
    }

    #[test]
    fn maps_neutral_install_output_to_unchanged_host_result() {
        let request = install_request();
        let output = LoaderInstallOutputData {
            installed_version_id: "fabric-loader-0.15.7-1.20.1".to_string(),
            log_path: Some("game/logs/installer.log".to_string()),
        };
        let result = loader_install_result(
            &request,
            Path::new("game"),
            output.installed_version_id,
            output.log_path,
        );
        assert_eq!(
            result,
            LoaderInstallResult {
                game_dir: "game".to_string(),
                game_version: "1.20.1".to_string(),
                loader: LoaderKind::Fabric,
                loader_version: "0.15.7".to_string(),
                installed_version_id: "fabric-loader-0.15.7-1.20.1".to_string(),
                install_path: Path::new("game")
                    .join("versions")
                    .join("fabric-loader-0.15.7-1.20.1")
                    .display()
                    .to_string(),
                log_path: Some("game/logs/installer.log".to_string()),
            }
        );
    }

    #[test]
    fn maps_provider_progress_messages_to_legacy_event_stages() {
        let cases = [
            ("Fetching Fabric profile", "profile"),
            ("Fabric profile downloaded", "profile"),
            ("Downloading Fabric libraries", "libraries"),
            (
                "Downloaded library example/library/1.0/library-1.0.jar",
                "libraries",
            ),
            ("Writing loader version profile", "finalize"),
            ("Loader version profile written", "finalize"),
            ("Fabric installation complete", "complete"),
            ("Downloading Forge installer", "installer"),
            ("Running Forge installer", "installer"),
            ("Forge installer finished", "installer"),
            ("Downloading LiteLoader", "libraries"),
            ("LiteLoader downloaded", "libraries"),
            ("Downloaded LiteLoader dependency 1", "libraries"),
            ("LiteLoader installation complete", "complete"),
            ("unmapped provider message", "provider"),
        ];
        for (message, expected_stage) in cases {
            assert_eq!(loader_progress_stage(message), expected_stage);
        }
    }

    #[test]
    fn preserves_progress_event_shape_and_serde() {
        let request = install_request();
        let event = loader_progress_event(
            &request,
            "libraries",
            1,
            2,
            "Downloaded library example/library/1.0/library-1.0.jar",
        );
        assert_eq!(
            serde_json::to_value(&event).expect("event serialization"),
            json!({
                "loader": "fabric",
                "gameVersion": "1.20.1",
                "loaderVersion": "0.15.7",
                "stage": "libraries",
                "completed": 1,
                "total": 2,
                "message": "Downloaded library example/library/1.0/library-1.0.jar"
            })
        );
    }

    #[test]
    fn preserves_loader_timeout_split_and_event_name() {
        let timeouts = loader_timeouts();
        assert_eq!(timeouts.metadata, Duration::from_secs(75));
        assert_eq!(timeouts.installer_http, Duration::from_secs(600));
        assert_eq!(timeouts.java_process, Duration::from_secs(600));
        assert_eq!(
            LOADER_INSTALL_PROGRESS_EVENT,
            "flint://loader-install-progress"
        );
    }

    #[test]
    fn resolves_loader_java_path_before_creating_the_typed_request() {
        let default = resolve_loader_java_path("   ").expect("platform default java");
        assert_eq!(
            Path::new(&default)
                .file_name()
                .and_then(|value| value.to_str()),
            Some(java::java_executable())
        );
        let missing = std::env::temp_dir().join(format!(
            "flint-loader-java-missing-{}",
            std::process::id()
        ));
        let input = missing.to_string_lossy().to_string();
        assert_eq!(
            resolve_loader_java_path(&input).expect_err("missing java executable"),
            format!("Java executable not found: {input}")
        );
    }
    #[test]
    fn loader_contexts_use_a_non_expiring_aggregate_deadline() {
        let deadline = loader_deadline();
        assert!(deadline.is_none());
        assert!(!deadline.is_expired());
        assert!(deadline.check().is_ok());
        assert_eq!(deadline.remaining(), Duration::MAX);
    }
}