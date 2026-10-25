//! Fabric implementation of the provider-neutral loader contract.

use flint_core::{
    minecraft::{
        compare_loader_versions_desc, fetch_json, install_meta_profile, validate_game_version,
        MinecraftPlatform, MetaProfileContext, MetaProfileRequest, MAX_JSON_BYTES,
        MAX_LIBRARY_DOWNLOAD_BYTES,
    },
    paths::component_is_safe,
};
use flint_provider_api::{
    loader_descriptor_for, LoaderInstallContext, LoaderInstallOutputData, LoaderInstallRequest,
    LoaderListContext, LoaderListRequest, LoaderProvider, LoaderVersionData, Provider,
    ProviderDescriptor, FABRIC_CAPABILITIES, FABRIC_PROVIDER_ID,
};
use serde_json::Value;

const FABRIC_LOADER_LIST: &str = "https://meta.fabricmc.net/v2/versions/loader";
const SERVICE_NAME: &str = "Fabric";

pub const DESCRIPTOR: ProviderDescriptor =
    loader_descriptor_for(FABRIC_PROVIDER_ID, FABRIC_CAPABILITIES);

#[derive(Debug, Clone, Copy, Default)]
pub struct FabricLoaderProvider;

impl FabricLoaderProvider {
    pub const fn new() -> Self {
        Self
    }
}

impl Provider for FabricLoaderProvider {
    fn descriptor(&self) -> ProviderDescriptor {
        DESCRIPTOR
    }
}

impl LoaderProvider for FabricLoaderProvider {
    fn list_versions(
        &self,
        context: &LoaderListContext<'_>,
        request: &LoaderListRequest,
    ) -> Result<Vec<LoaderVersionData>, String> {
        validate_game_version(&request.game_version)?;
        context.check_control()?;
        let url = format!("{FABRIC_LOADER_LIST}/{}", request.game_version);
        let value = fetch_json(
            context.network(),
            &url,
            "Fabric Meta",
            MAX_JSON_BYTES,
            context.timeouts().metadata,
            context.deadline(),
            context.cancellation(),
        )?;
        context.check_control()?;
        let mut versions = Vec::new();
        if let Some(entries) = value.as_array() {
            for entry in entries {
                let Some(loader) = entry.get("loader") else {
                    continue;
                };
                let Some(loader_version) = loader.get("version").and_then(Value::as_str) else {
                    continue;
                };
                if !component_is_safe(loader_version) {
                    continue;
                }
                let stable = loader
                    .get("stable")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                versions.push(LoaderVersionData {
                    version: loader_version.to_string(),
                    label: format!("Fabric Loader {loader_version}"),
                    recommended: stable,
                    game_version: request.game_version.clone(),
                    release_time: None,
                });
            }
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
        let profile_url = format!(
            "{FABRIC_LOADER_LIST}/{}/{}/profile/json",
            request.game_version, request.loader_version
        );
        let platform = MinecraftPlatform::current();
        let mut meta_request =
            MetaProfileRequest::new(&profile_url, SERVICE_NAME, &request.game_version);
        meta_request.metadata_timeout = context.timeouts().metadata;
        meta_request.profile_max_bytes = MAX_JSON_BYTES;
        meta_request.library_max_bytes = MAX_LIBRARY_DOWNLOAD_BYTES;
        let meta_context = MetaProfileContext::new(
            context.root(),
            context.metadata_network(),
            context.workspace(),
            context.progress(),
            context.logger(),
            &platform,
            *context.deadline(),
            context.cancellation().clone(),
        );
        // The shared helper validates the returned profile id and requires
        // inheritsFrom to match request.game_version before writing anything.
        let output = install_meta_profile(&meta_context, &meta_request)?;
        Ok(LoaderInstallOutputData {
            installed_version_id: output.installed_version_id,
            log_path: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flint_core::ports::{
        Cancellation, Deadline, NoopLogger, NoopProgress, ProgressSink, ProgressUpdate,
    };
    use flint_provider_api::test_support::{FakeJavaRunner, FakeNetworkPort, FakePathPort};
    use flint_provider_api::{LoaderProvider as _, LoaderTimeouts};
    use serde_json::json;
    use std::{
        path::Path,
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
    fn list_parses_sorts_and_recommends_fabric_versions() {
        let network = FakeNetworkPort::new();
        network.push_response(
            serde_json::to_vec(&json!([
                {"loader": {"version": "0.15.6", "stable": true}},
                {"loader": {"version": "0.15.10", "stable": false}},
                {"loader": {"version": "0.15.9", "stable": true}}
            ]))
            .expect("fixture JSON"),
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
        let versions = FabricLoaderProvider::new()
            .list_versions(&context, &LoaderListRequest::new("1.20.1"))
            .expect("fabric versions");
        assert_eq!(
            versions.iter().map(|item| item.version.as_str()).collect::<Vec<_>>(),
            vec!["0.15.10", "0.15.9", "0.15.6"]
        );
        assert!(!versions[0].recommended);
        assert!(versions[1].recommended);
        assert!(versions[2].recommended);
        assert_eq!(network.requests_len(), 1);
        let requests = network.requests.lock().expect("network requests");
        assert_eq!(
            requests[0].url,
            "https://meta.fabricmc.net/v2/versions/loader/1.20.1"
        );
        assert_eq!(requests[0].service.as_str(), "Fabric Meta");
        assert_eq!(requests[0].max_bytes, MAX_JSON_BYTES);
        assert_eq!(requests[0].timeout, LoaderTimeouts::default().metadata);
    }

    #[test]
    fn list_translates_http_not_found_using_the_shared_guidance() {
        let network = FakeNetworkPort::new();
        network.push_error(
            "Fabric Meta request failed: HTTP status client error (404 Not Found)",
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
        let error = FabricLoaderProvider::new()
            .list_versions(&context, &LoaderListRequest::new("1.20.1"))
            .expect_err("HTTP 404 must fail");
        assert_eq!(error, "Fabric Meta did not find the requested resource.");
    }

    #[test]
    fn install_uses_profile_url_validates_profile_id_and_emits_progress() {
        let network = FakeNetworkPort::new();
        network.push_response(
            serde_json::to_vec(&json!({
                "id": "fabric-loader-0.15.7-1.20.1",
                "inheritsFrom": "1.20.1",
                "libraries": [{
                    "name": "example:fixture:1.0",
                    "downloads": {
                        "artifact": {
                            "path": "example/fixture/1.0/fixture-1.0.jar",
                            "url": "https://example.com/fixture-1.0.jar"
                        }
                    }
                }]
            }))
            .expect("profile JSON"),
        );
        let workspace = FakePathPort::new();
        let java = FakeJavaRunner::new();
        let progress = RecordingProgress::default();
        let logger = NoopLogger;
        let timeouts = LoaderTimeouts::new(
            Duration::from_secs(11),
            Duration::from_secs(22),
            Duration::from_secs(33),
        );
        let context = LoaderInstallContext::new(
            &network,
            &network,
            &workspace,
            &java,
            Path::new("game"),
            &progress,
            &logger,
            timeouts,
            Deadline::after(Duration::from_secs(30)),
            Cancellation::new(),
        );
        let output = FabricLoaderProvider::new()
            .install(
                &context,
                &LoaderInstallRequest::new("1.20.1", "0.15.7", "java"),
            )
            .expect("fabric install");
        assert_eq!(output.installed_version_id, "fabric-loader-0.15.7-1.20.1");
        assert!(output.log_path.is_none());
        let requests = network.requests.lock().expect("network requests");
        assert_eq!(requests.len(), 1);
        assert_eq!(
            requests[0].url,
            "https://meta.fabricmc.net/v2/versions/loader/1.20.1/0.15.7/profile/json"
        );
        assert_eq!(requests[0].timeout, timeouts.metadata);
        let profile_path = Path::new("game")
            .join("versions/fabric-loader-0.15.7-1.20.1/fabric-loader-0.15.7-1.20.1.json");
        assert!(workspace.file_bytes(&profile_path).is_some());
        assert_eq!(
            progress.messages(),
            vec![
                "Fetching Fabric profile".to_string(),
                "Fabric profile downloaded".to_string(),
                "Downloading Fabric libraries".to_string(),
                "Downloaded library example/fixture/1.0/fixture-1.0.jar".to_string(),
                "Writing loader version profile".to_string(),
                "Loader version profile written".to_string(),
                "Fabric installation complete".to_string(),
            ]
        );
    }

    #[test]
    fn install_rejects_profile_for_the_wrong_game_version() {
        let network = FakeNetworkPort::new();
        network.push_response(
            serde_json::to_vec(&json!({
                "id": "fabric-loader-0.15.7-1.20.2",
                "inheritsFrom": "1.20.2",
                "libraries": []
            }))
            .expect("profile JSON"),
        );
        let workspace = FakePathPort::new();
        let java = FakeJavaRunner::new();
        let progress = NoopProgress;
        let logger = NoopLogger;
        let context = LoaderInstallContext::new(
            &network,
            &network,
            &workspace,
            &java,
            Path::new("game"),
            &progress,
            &logger,
            LoaderTimeouts::default(),
            Deadline::after(Duration::from_secs(30)),
            Cancellation::new(),
        );
        let error = FabricLoaderProvider::new()
            .install(
                &context,
                &LoaderInstallRequest::new("1.20.1", "0.15.7", "java"),
            )
            .expect_err("mismatched inheritsFrom must fail");
        assert_eq!(
            error,
            "Fabric returned a profile for 1.20.2, not 1.20.1."
        );
    }

    #[test]
    fn expired_deadline_is_reported_before_network_io() {
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
        let error = FabricLoaderProvider::new()
            .list_versions(&context, &LoaderListRequest::new("1.20.1"))
            .expect_err("deadline must fail");
        assert_eq!(error, "Operation deadline exceeded.");
        assert_eq!(network.requests_len(), 0);
    }

    #[test]
    fn provider_is_object_safe_and_descriptor_is_stable() {
        assert_eq!(DESCRIPTOR.id, FABRIC_PROVIDER_ID);
        assert_eq!(DESCRIPTOR.capabilities[0].selection_keys[0], "loader-kind:fabric");
        let _provider: Box<dyn flint_provider_api::LoaderProvider> = Box::new(FabricLoaderProvider::new());
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