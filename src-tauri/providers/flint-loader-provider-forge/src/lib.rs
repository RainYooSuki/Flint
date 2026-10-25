//! Forge implementation of the provider-neutral loader contract.

use flint_core::{
    minecraft::{
        compare_loader_versions_desc, fetch_json, install_java_installer, validate_game_version,
        InstallerContext, InstallerRequest, MAX_INSTALLER_BYTES, MAX_JSON_BYTES,
    },
    paths::component_is_safe,
};
use flint_provider_api::{
    loader_descriptor_for, LoaderInstallContext, LoaderInstallOutputData, LoaderInstallRequest,
    LoaderListContext, LoaderListRequest, LoaderProvider, LoaderVersionData, Provider,
    ProviderDescriptor, FORGE_CAPABILITIES, FORGE_PROVIDER_ID,
};
use serde_json::{json, Value};
use std::path::Path;

const FORGE_MAVEN_METADATA: &str =
    "https://files.minecraftforge.net/net/minecraftforge/forge/maven-metadata.json";
const FORGE_PROMOTIONS: &str =
    "https://files.minecraftforge.net/net/minecraftforge/forge/promotions_slim.json";
const SERVICE_NAME: &str = "Forge";

pub const DESCRIPTOR: ProviderDescriptor =
    loader_descriptor_for(FORGE_PROVIDER_ID, FORGE_CAPABILITIES);

#[derive(Debug, Clone, Copy, Default)]
pub struct ForgeLoaderProvider;

impl ForgeLoaderProvider {
    pub const fn new() -> Self {
        Self
    }
}

impl Provider for ForgeLoaderProvider {
    fn descriptor(&self) -> ProviderDescriptor {
        DESCRIPTOR
    }
}

impl LoaderProvider for ForgeLoaderProvider {
    fn list_versions(
        &self,
        context: &LoaderListContext<'_>,
        request: &LoaderListRequest,
    ) -> Result<Vec<LoaderVersionData>, String> {
        validate_game_version(&request.game_version)?;
        context.check_control()?;
        let metadata = fetch_json(
            context.network(),
            FORGE_MAVEN_METADATA,
            "Minecraft Forge Maven",
            MAX_JSON_BYTES,
            context.timeouts().metadata,
            context.deadline(),
            context.cancellation(),
        )?;
        let promotions = fetch_json(
            context.network(),
            FORGE_PROMOTIONS,
            "Minecraft Forge promotions",
            MAX_JSON_BYTES,
            context.timeouts().metadata,
            context.deadline(),
            context.cancellation(),
        )
        .unwrap_or_else(|_| json!({}));
        context.check_control()?;
        let recommended_version = promotions
            .get("promos")
            .and_then(|promos| promos.get(format!("{}-recommended", request.game_version)))
            .and_then(Value::as_str)
            .map(|value| value.to_string());
        let Some(raw_versions) = metadata
            .get(request.game_version.as_str())
            .and_then(Value::as_array)
        else {
            return Ok(Vec::new());
        };
        let prefix = format!("{}-", request.game_version);
        let mut versions = Vec::new();
        for raw in raw_versions {
            let Some(raw) = raw.as_str() else {
                continue;
            };
            let loader_version = raw.strip_prefix(&prefix).unwrap_or(raw);
            if !component_is_safe(loader_version) {
                continue;
            }
            versions.push(LoaderVersionData {
                version: loader_version.to_string(),
                label: format!("Forge {loader_version}"),
                recommended: recommended_version.as_deref() == Some(loader_version),
                game_version: request.game_version.clone(),
                release_time: None,
            });
        }
        context.check_control()?;
        versions.sort_by(|left, right| {
            compare_loader_versions_desc(&left.version, &right.version)
        });
        Ok(versions)
    }

    fn install(
        &self,
        context: &LoaderInstallContext<'_>,
        request: &LoaderInstallRequest,
    ) -> Result<LoaderInstallOutputData, String> {
        validate_game_version(&request.game_version)?;
        context.check_control()?;
        if !component_is_safe(&request.loader_version) {
            return Err("The selected loader version is invalid.".to_string());
        }
        let loader_version = request
            .loader_version
            .strip_prefix(&format!("{}-", request.game_version))
            .unwrap_or(&request.loader_version);
        let maven_version = format!("{}-{loader_version}", request.game_version);
        let installer_url = format!(
            "https://maven.minecraftforge.net/net/minecraftforge/forge/{maven_version}/forge-{maven_version}-installer.jar"
        );
        let file_name = format!("forge-{maven_version}-installer.jar");
        let expected_id = format!("{}-forge-{loader_version}", request.game_version);
        let log_label = format!("forge-{}-{loader_version}", request.game_version);
        let root_arg = context.root().display().to_string();
        let mut installer_request = InstallerRequest::new(
            &installer_url,
            &file_name,
            vec!["--installClient".to_string(), root_arg],
            SERVICE_NAME,
            &log_label,
            &request.game_version,
            Path::new(&request.java_path),
        );
        installer_request.expected_profile_id = Some(&expected_id);
        installer_request.installer_max_bytes = MAX_INSTALLER_BYTES;
        installer_request.installer_timeout = context.timeouts().installer_http;
        installer_request.java_timeout = context.timeouts().java_process;
        let installer_context = InstallerContext::new(
            context.root(),
            context.installer_network(),
            context.workspace(),
            context.java(),
            context.progress(),
            context.logger(),
            *context.deadline(),
            context.cancellation().clone(),
        );
        let output = install_java_installer(&installer_context, &installer_request)?;
        Ok(LoaderInstallOutputData {
            installed_version_id: output.installed_version_id,
            log_path: Some(output.log_path),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flint_core::ports::{
        Cancellation, Deadline, DownloadRequest, NetworkPort, NetworkRequest, NoopLogger,
        NoopProgress, ProgressSink, ProgressUpdate,
    };
    use flint_provider_api::test_support::{FakeJavaRunner, FakeNetworkPort, FakePathPort};
    use flint_provider_api::LoaderTimeouts;
    use serde_json::json;
    use std::{
        path::{Path, PathBuf},
        sync::Mutex,
        time::Duration,
    };

    #[derive(Default)]
    struct RecordingProgress {
        updates: Mutex<Vec<ProgressUpdate>>,
    }

    impl RecordingProgress {
        fn messages(&self) -> Vec<String> {
            self.updates
                .lock()
                .expect("progress lock")
                .iter()
                .filter_map(|update| update.message.clone())
                .collect()
        }
    }

    impl ProgressSink for RecordingProgress {
        fn report(&self, update: ProgressUpdate) {
            self.updates.lock().expect("progress lock").push(update);
        }
    }

    fn list_context<'a>(
        network: &'a FakeNetworkPort,
        workspace: &'a FakePathPort,
        progress: &'a NoopProgress,
        logger: &'a NoopLogger,
        deadline: Deadline,
    ) -> LoaderListContext<'a> {
        LoaderListContext::new(
            network,
            workspace,
            progress,
            logger,
            LoaderTimeouts::default(),
            deadline,
            Cancellation::new(),
        )
    }

    #[test]
    fn list_parses_forge_versions_and_recommendation() {
        let network = FakeNetworkPort::new();
        network.push_response(
            serde_json::to_vec(&json!({
                "1.20.1": ["1.20.1-47.2.0", "47.1.0", "1.20.1-47.3.0"]
            }))
            .expect("metadata JSON"),
        );
        network.push_response(
            serde_json::to_vec(&json!({
                "promos": {"1.20.1-recommended": "47.2.0"}
            }))
            .expect("promotions JSON"),
        );
        let workspace = FakePathPort::new();
        let progress = NoopProgress;
        let logger = NoopLogger;
        let context = list_context(
            &network,
            &workspace,
            &progress,
            &logger,
            Deadline::after(Duration::from_secs(30)),
        );
        let versions = ForgeLoaderProvider::new()
            .list_versions(&context, &LoaderListRequest::new("1.20.1"))
            .expect("forge versions");
        assert_eq!(
            versions.iter().map(|item| item.version.as_str()).collect::<Vec<_>>(),
            vec!["47.3.0", "47.2.0", "47.1.0"]
        );
        assert!(!versions[0].recommended);
        assert!(versions[1].recommended);
        assert!(!versions[2].recommended);
        let requests = network.requests.lock().expect("network requests");
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[0].url, FORGE_MAVEN_METADATA);
        assert_eq!(requests[1].url, FORGE_PROMOTIONS);
        assert_eq!(requests[0].timeout, LoaderTimeouts::default().metadata);
        assert_eq!(requests[1].timeout, LoaderTimeouts::default().metadata);
    }

    #[test]
    fn list_preserves_invalid_json_error_wording() {
        let network = FakeNetworkPort::new();
        network.push_response(b"not json".to_vec());
        let workspace = FakePathPort::new();
        let progress = NoopProgress;
        let logger = NoopLogger;
        let context = list_context(
            &network,
            &workspace,
            &progress,
            &logger,
            Deadline::after(Duration::from_secs(30)),
        );
        let error = ForgeLoaderProvider::new()
            .list_versions(&context, &LoaderListRequest::new("1.20.1"))
            .expect_err("invalid metadata JSON must fail");
        assert!(error.starts_with("Minecraft Forge Maven returned invalid JSON:"));
    }

    #[test]
    fn install_builds_forge_installer_arguments_and_expected_profile() {
        let installer_id = "1.20.1-forge-47.2.0";
        let installer_file = "forge-1.20.1-47.2.0-installer.jar";
        let metadata_network = FakeNetworkPort::new();
        let installer_network = FakeNetworkPort::new();
        let workspace = FakePathPort::new();
        workspace.insert_file(
            Path::new("game").join(format!("loader-installers/{installer_file}")),
            b"PK\x03\x04".to_vec(),
        );
        workspace.insert_file(
            Path::new("game").join(format!("versions/{installer_id}/{installer_id}.json")),
            br#"{"id":"1.20.1-forge-47.2.0","inheritsFrom":"1.20.1"}"#.to_vec(),
        );
        let java = FakeJavaRunner::new();
        let progress = RecordingProgress::default();
        let logger = NoopLogger;
        let timeouts = LoaderTimeouts::new(
            Duration::from_secs(11),
            Duration::from_secs(22),
            Duration::from_secs(33),
        );
        let context = LoaderInstallContext::new(
            &metadata_network,
            &installer_network,
            &workspace,
            &java,
            Path::new("game"),
            &progress,
            &logger,
            timeouts,
            Deadline::after(Duration::from_secs(30)),
            Cancellation::new(),
        );
        let output = ForgeLoaderProvider::new()
            .install(
                &context,
                &LoaderInstallRequest::new("1.20.1", "47.2.0", "java"),
            )
            .expect("forge install");
        assert_eq!(output.installed_version_id, installer_id);
        let log_path = output.log_path.expect("forge installer log");
        assert!(workspace.file_bytes(Path::new(log_path.as_str())).is_some());

        let downloads = installer_network
            .downloads
            .lock()
            .expect("installer downloads");
        assert_eq!(downloads.len(), 1);
        assert_eq!(
            downloads[0].url,
            "https://maven.minecraftforge.net/net/minecraftforge/forge/1.20.1-47.2.0/forge-1.20.1-47.2.0-installer.jar"
        );
        assert_eq!(downloads[0].max_bytes, MAX_INSTALLER_BYTES);
        assert_eq!(downloads[0].timeout, timeouts.installer_http);

        let java_requests = java.requests.lock().expect("java requests");
        assert_eq!(java_requests.len(), 1);
        assert_eq!(java_requests[0].program_args.len(), 3);
        assert_eq!(java_requests[0].main_class, "-jar");
        assert_eq!(java_requests[0].timeout, timeouts.java_process);
        assert_eq!(java_requests[0].current_dir, Some(PathBuf::from("game")));
        assert!(java_requests[0].program_args[0].ends_with(installer_file));
        assert_eq!(java_requests[0].program_args[1], "--installClient");
        assert_eq!(java_requests[0].program_args[2], "game");
        assert!(progress
            .messages()
            .iter()
            .any(|message| message.contains("Forge installer finished")));
    }

    struct FailingInstallerNetwork;

    impl NetworkPort for FailingInstallerNetwork {
        fn read_json(&self, _request: &NetworkRequest) -> Result<serde_json::Value, String> {
            Err("unexpected metadata read".to_string())
        }

        fn read_text(&self, _request: &NetworkRequest) -> Result<String, String> {
            Err("unexpected metadata read".to_string())
        }

        fn read_bytes(&self, _request: &NetworkRequest) -> Result<Vec<u8>, String> {
            Err("unexpected metadata read".to_string())
        }

        fn download_atomic(&self, _request: &DownloadRequest) -> Result<u64, String> {
            Err(
                "Forge installer: Download failed for Forge installer after 3 attempts: connection reset"
                    .to_string(),
            )
        }
    }

    #[test]
    fn installer_download_failure_preserves_shared_download_error() {
        let metadata_network = FakeNetworkPort::new();
        let installer_network = FailingInstallerNetwork;
        let workspace = FakePathPort::new();
        let java = FakeJavaRunner::new();
        let progress = NoopProgress;
        let logger = NoopLogger;
        let context = LoaderInstallContext::new(
            &metadata_network,
            &installer_network,
            &workspace,
            &java,
            Path::new("game"),
            &progress,
            &logger,
            LoaderTimeouts::default(),
            Deadline::after(Duration::from_secs(30)),
            Cancellation::new(),
        );
        let error = ForgeLoaderProvider::new()
            .install(
                &context,
                &LoaderInstallRequest::new("1.20.1", "47.2.0", "java"),
            )
            .expect_err("installer download failure must fail");
        assert_eq!(
            error,
            "Download failed for Forge installer after 3 attempts: connection reset"
        );
    }

    #[test]
    fn expired_deadline_and_pre_cancelled_cancellation_fail_before_network_io() {
        let network = FakeNetworkPort::new();
        let workspace = FakePathPort::new();
        let progress = NoopProgress;
        let logger = NoopLogger;
        let context = list_context(
            &network,
            &workspace,
            &progress,
            &logger,
            Deadline::after(Duration::ZERO),
        );
        let error = ForgeLoaderProvider::new()
            .list_versions(&context, &LoaderListRequest::new("1.20.1"))
            .expect_err("deadline must fail");
        assert_eq!(error, "Operation deadline exceeded.");
        assert_eq!(network.requests_len(), 0);

        let cancellation = Cancellation::new();
        cancellation.cancel();
        let cancelled_context = LoaderListContext::new(
            &network,
            &workspace,
            &progress,
            &logger,
            LoaderTimeouts::default(),
            Deadline::after(Duration::from_secs(30)),
            cancellation,
        );
        let error = ForgeLoaderProvider::new()
            .list_versions(&cancelled_context, &LoaderListRequest::new("1.20.1"))
            .expect_err("cancellation must fail");
        assert_eq!(error, "Operation was cancelled.");
        assert_eq!(network.requests_len(), 0);
    }

    #[test]
    fn provider_is_object_safe_and_descriptor_is_stable() {
        assert_eq!(DESCRIPTOR.id, FORGE_PROVIDER_ID);
        assert_eq!(DESCRIPTOR.capabilities[0].selection_keys[0], "loader-kind:forge");
        let _provider: Box<dyn flint_provider_api::LoaderProvider> = Box::new(ForgeLoaderProvider::new());
    }

    #[test]
    fn provider_source_uses_no_raw_network_filesystem_process_or_tauri_apis() {
        let source = include_str!("lib.rs");
        for forbidden in [
            concat!("req", "west::"),
            concat!("std", "::fs"),
            concat!("std", "::process"),
            concat!("std", "::net"),
            concat!("Command", "::new"),
            concat!("tauri", "::"),
            concat!("tokio", "::process"),
            concat!("zip", "::"),
            concat!("Loader", "Kind"),
            concat!("File", "::"),
            concat!("Tcp", "Stream"),
            concat!("Udp", "Socket"),
            concat!("flint_", "loader_provider_"),
            concat!("flint_", "mod_provider_"),
        ] {
            assert!(!source.contains(forbidden), "forbidden API token present: {forbidden}");
        }
    }
}