//! NeoForge implementation of the provider-neutral loader contract.

use flint_core::{
    minecraft::{
        fetch_text, install_java_installer, parse_xml_versions, sort_loader_versions_desc,
        validate_game_version, InstallerContext, InstallerRequest, MAX_INSTALLER_BYTES,
        MAX_JSON_BYTES,
    },
    paths::component_is_safe,
};
use flint_provider_api::{
    loader_descriptor_for, LoaderInstallContext, LoaderInstallOutputData, LoaderInstallRequest,
    LoaderListContext, LoaderListRequest, LoaderProvider, LoaderVersionData, Provider,
    ProviderDescriptor, NEOFORGE_CAPABILITIES, NEOFORGE_PROVIDER_ID,
};
use std::path::Path;

const NEOFORGE_MAVEN_METADATA: &str =
    "https://maven.neoforged.net/releases/net/neoforged/neoforge/maven-metadata.xml";
const NEOFORGE_LEGACY_MAVEN_METADATA: &str =
    "https://maven.neoforged.net/releases/net/neoforged/forge/maven-metadata.xml";
const METADATA_SERVICE: &str = "NeoForge Maven";
const INSTALLER_SERVICE: &str = "NeoForge";

pub const DESCRIPTOR: ProviderDescriptor =
    loader_descriptor_for(NEOFORGE_PROVIDER_ID, NEOFORGE_CAPABILITIES);

/// Independent NeoForge loader provider.
#[derive(Debug, Clone, Copy, Default)]
pub struct NeoForgeLoaderProvider;

impl NeoForgeLoaderProvider {
    pub const fn new() -> Self {
        Self
    }
}

impl Provider for NeoForgeLoaderProvider {
    fn descriptor(&self) -> ProviderDescriptor {
        DESCRIPTOR
    }
}

impl LoaderProvider for NeoForgeLoaderProvider {
    fn list_versions(
        &self,
        context: &LoaderListContext<'_>,
        request: &LoaderListRequest,
    ) -> Result<Vec<LoaderVersionData>, String> {
        list_neoforge(context, request)
    }

    fn install(
        &self,
        context: &LoaderInstallContext<'_>,
        request: &LoaderInstallRequest,
    ) -> Result<LoaderInstallOutputData, String> {
        install_neoforge(context, request)
    }
}

fn list_neoforge(
    context: &LoaderListContext<'_>,
    request: &LoaderListRequest,
) -> Result<Vec<LoaderVersionData>, String> {
    validate_game_version(&request.game_version)?;
    context.check_control()?;
    let legacy = request.game_version == "1.20.1";
    let url = if legacy {
        NEOFORGE_LEGACY_MAVEN_METADATA
    } else {
        NEOFORGE_MAVEN_METADATA
    };
    let text = fetch_text(
        context.network(),
        url,
        METADATA_SERVICE,
        MAX_JSON_BYTES,
        context.timeouts().metadata,
        context.deadline(),
        context.cancellation(),
    )?;
    let prefix = neoforge_game_prefix(&request.game_version);
    let mut result = Vec::new();
    for raw in parse_xml_versions(&text) {
        let loader_version = if legacy {
            raw.strip_prefix(&format!("{}-", request.game_version))
                .unwrap_or(&raw)
        } else if raw == prefix || raw.starts_with(&format!("{prefix}.")) {
            raw.as_str()
        } else {
            continue;
        };
        if !component_is_safe(loader_version) {
            continue;
        }
        result.push(LoaderVersionData {
            version: loader_version.to_string(),
            label: format!("NeoForge {loader_version}"),
            recommended: false,
            game_version: request.game_version.clone(),
            release_time: None,
        });
    }
    sort_loader_versions_desc(&mut result, |version| version.version.as_str());
    if let Some(first) = result.first_mut() {
        first.recommended = true;
    }
    Ok(result)
}

fn install_neoforge(
    context: &LoaderInstallContext<'_>,
    request: &LoaderInstallRequest,
) -> Result<LoaderInstallOutputData, String> {
    validate_game_version(&request.game_version)?;
    if !component_is_safe(request.loader_version.trim()) {
        return Err("The selected loader version is invalid.".to_string());
    }
    context.check_control()?;
    let legacy = request.game_version == "1.20.1";
    let loader_prefix = format!("{}-", request.game_version);
    let loader_version = request
        .loader_version
        .strip_prefix(&loader_prefix)
        .unwrap_or(&request.loader_version);
    let (url, file_name, expected_id) = if legacy {
        let maven_version = format!("{}-{loader_version}", request.game_version);
        (
            format!(
                "https://maven.neoforged.net/releases/net/neoforged/forge/{maven_version}/forge-{maven_version}-installer.jar"
            ),
            format!("neoforge-{maven_version}-installer.jar"),
            format!("{}-neoforge-{loader_version}", request.game_version),
        )
    } else {
        (
            format!(
                "https://maven.neoforged.net/releases/net/neoforged/neoforge/{loader_version}/neoforge-{loader_version}-installer.jar"
            ),
            format!("neoforge-{loader_version}-installer.jar"),
            format!("{}-neoforge-{loader_version}", request.game_version),
        )
    };
    let installer_args = vec![
        "--installClient".to_string(),
        context.root().display().to_string(),
    ];
    let log_label = format!(
        "neoforge-{}-{}",
        request.game_version, request.loader_version
    );
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
    let mut installer_request = InstallerRequest::new(
        &url,
        &file_name,
        installer_args,
        INSTALLER_SERVICE,
        &log_label,
        &request.game_version,
        Path::new(&request.java_path),
    );
    installer_request.expected_profile_id = Some(&expected_id);
    installer_request.installer_max_bytes = MAX_INSTALLER_BYTES;
    installer_request.installer_timeout = context.timeouts().installer_http;
    installer_request.java_timeout = context.timeouts().java_process;
    let output = install_java_installer(&installer_context, &installer_request)?;
    Ok(LoaderInstallOutputData {
        installed_version_id: output.installed_version_id,
        log_path: Some(output.log_path),
    })
}

fn neoforge_game_prefix(game_version: &str) -> String {
    let parts: Vec<&str> = game_version.split('.').collect();
    match parts.as_slice() {
        ["1", minor] => format!("{minor}.0"),
        ["1", minor, patch, ..] => format!("{minor}.{patch}"),
        _ => game_version.to_string(),
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use flint_core::ports::{
        Cancellation, Deadline, NoopLogger, ProgressSink, ProgressUpdate,
    };
    use flint_provider_api::test_support::{
        FakeJavaRunner, FakeNetworkPort, FakePathPort,
    };
    use std::{
        path::{Path, PathBuf},
        sync::Mutex,
        time::{Duration, Instant},
    };

    #[derive(Default)]
    struct RecordingProgress {
        updates: Mutex<Vec<ProgressUpdate>>,
    }

    impl ProgressSink for RecordingProgress {
        fn report(&self, update: ProgressUpdate) {
            self.updates
                .lock()
                .expect("progress lock")
                .push(update);
        }
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

    struct Harness {
        metadata: FakeNetworkPort,
        installer: FakeNetworkPort,
        workspace: FakePathPort,
        java: FakeJavaRunner,
        progress: RecordingProgress,
        logger: NoopLogger,
        cancellation: Cancellation,
    }

    impl Harness {
        fn new() -> Self {
            Self {
                metadata: FakeNetworkPort::new(),
                installer: FakeNetworkPort::new(),
                workspace: FakePathPort::new(),
                java: FakeJavaRunner::new(),
                progress: RecordingProgress::default(),
                logger: NoopLogger,
                cancellation: Cancellation::new(),
            }
        }

        fn list_context(&self) -> LoaderListContext<'_> {
            LoaderListContext::new(
                &self.metadata,
                &self.workspace,
                &self.progress,
                &self.logger,
                flint_provider_api::LoaderTimeouts::default(),
                Deadline::after(Duration::from_secs(120)),
                self.cancellation.clone(),
            )
        }

        fn install_context(&self) -> LoaderInstallContext<'_> {
            LoaderInstallContext::new(
                &self.metadata,
                &self.installer,
                &self.workspace,
                &self.java,
                Path::new("game"),
                &self.progress,
                &self.logger,
                flint_provider_api::LoaderTimeouts::default(),
                Deadline::after(Duration::from_secs(120)),
                self.cancellation.clone(),
            )
        }
    }

    fn seeded_install(
        harness: &Harness,
        game_version: &str,
        loader_version: &str,
    ) -> (String, String) {
        let normalized_loader_version = loader_version
            .strip_prefix(&format!("{game_version}-"))
            .unwrap_or(loader_version);
        let expected_id = format!("{game_version}-neoforge-{normalized_loader_version}");
        let installer_name = if game_version == "1.20.1" {
            format!("neoforge-{game_version}-{normalized_loader_version}-installer.jar")
        } else {
            format!("neoforge-{normalized_loader_version}-installer.jar")
        };
        harness.workspace.insert_file(
            PathBuf::from("game")
                .join("loader-installers")
                .join(installer_name),
            b"PK\x03\x04installer".to_vec(),
        );
        harness.workspace.insert_file(
            PathBuf::from("game")
                .join("versions")
                .join(&expected_id)
                .join(format!("{expected_id}.json")),
            br#"{"id":"seeded","inheritsFrom":"1.21.1"}"#.to_vec(),
        );
        (expected_id, loader_version.to_string())
    }

    #[test]
    fn descriptor_declares_the_neoforge_loader_contract() {
        let descriptor = NeoForgeLoaderProvider::new().descriptor();
        assert_eq!(descriptor.id, NEOFORGE_PROVIDER_ID);
        assert_eq!(descriptor.provider_version, "1.0.0");
        assert_eq!(descriptor.provider_api, flint_provider_api::LOADER_PROVIDER_API);
        assert_eq!(descriptor.capabilities[0].id, flint_provider_api::LOADER_INSTALLATION_CAPABILITY_ID);
        assert_eq!(
            descriptor.capabilities[0].selection_keys,
            flint_provider_api::NEOFORGE_SELECTION_KEYS
        );
    }

    #[test]
    fn lists_new_neoforge_versions_newest_first_and_recommends_first() {
        let harness = Harness::new();
        harness.metadata.push_response(
            b"<metadata><versioning><versions>\
                <version>21.1.80</version>\
                <version>21.1.81</version>\
                <version>21.2.1</version>\
            </versions></versioning></metadata>"
                .to_vec(),
        );
        let versions = NeoForgeLoaderProvider::new()
            .list_versions(
                &harness.list_context(),
                &LoaderListRequest::new("1.21.1"),
            )
            .expect("neoforge versions");
        assert_eq!(
            versions.iter().map(|version| version.version.as_str()).collect::<Vec<_>>(),
            vec!["21.1.81", "21.1.80"]
        );
        assert!(versions[0].recommended);
        assert!(versions[1..].iter().all(|version| !version.recommended));
        let requests = harness.metadata.requests.lock().unwrap();
        assert_eq!(requests[0].url, NEOFORGE_MAVEN_METADATA);
        assert_eq!(requests[0].timeout, Duration::from_secs(75));
    }

    #[test]
    fn lists_legacy_neoforge_versions_with_stripped_game_prefix() {
        let harness = Harness::new();
        harness.metadata.push_response(
            b"<metadata><versioning><versions>\
                <version>1.20.1-47.1.100</version>\
                <version>1.20.1-47.1.99</version>\
            </versions></versioning></metadata>"
                .to_vec(),
        );
        let versions = NeoForgeLoaderProvider::new()
            .list_versions(
                &harness.list_context(),
                &LoaderListRequest::new("1.20.1"),
            )
            .expect("legacy neoforge versions");
        assert_eq!(versions[0].version, "47.1.100");
        assert_eq!(versions[1].version, "47.1.99");
        assert!(versions[0].recommended);
        assert_eq!(
            harness.metadata.requests.lock().unwrap()[0].url,
            NEOFORGE_LEGACY_MAVEN_METADATA
        );
    }

    #[test]
    fn maps_game_versions_to_neoforge_maven_prefixes() {
        assert_eq!(neoforge_game_prefix("1.21.1"), "21.1");
        assert_eq!(neoforge_game_prefix("1.20.2"), "20.2");
        assert_eq!(neoforge_game_prefix("1.21"), "21.0");
    }

    #[test]
    fn list_translates_generic_http_status_using_shared_guidance() {
        let harness = Harness::new();
        harness.metadata.push_error(
            "NeoForge Maven request failed: HTTP status server error (503 Service Unavailable)",
        );
        let error = NeoForgeLoaderProvider::new()
            .list_versions(
                &harness.list_context(),
                &LoaderListRequest::new("1.21.1"),
            )
            .expect_err("HTTP 503 must fail");
        assert_eq!(
            error,
            "NeoForge Maven request failed with HTTP 503. Check connectivity or try another version."
        );
    }

    #[test]
    fn installs_new_neoforge_with_exact_url_args_and_timeouts() {
        let harness = Harness::new();
        let (expected_id, loader_version) = seeded_install(&harness, "1.21.1", "47.1.0");
        let request = LoaderInstallRequest::new("1.21.1", loader_version.as_str(), "java");
        let output = NeoForgeLoaderProvider::new()
            .install(&harness.install_context(), &request)
            .expect("neoforge install");
        assert_eq!(output.installed_version_id, expected_id);
        assert!(output.log_path.as_deref().unwrap_or_default().contains("flint-installer-neoforge-"));
        let downloads = harness.installer.downloads.lock().unwrap();
        assert_eq!(downloads.len(), 1);
        assert_eq!(
            downloads[0].url,
            "https://maven.neoforged.net/releases/net/neoforged/neoforge/47.1.0/neoforge-47.1.0-installer.jar"
        );
        assert_eq!(downloads[0].max_bytes, MAX_INSTALLER_BYTES);
        assert_eq!(downloads[0].timeout, Duration::from_secs(600));
        let java_requests = harness.java.requests.lock().unwrap();
        assert_eq!(java_requests.len(), 1);
        assert_eq!(java_requests[0].timeout, Duration::from_secs(600));
        assert_eq!(java_requests[0].main_class, "-jar");
        assert!(java_requests[0].program_args[0].ends_with("neoforge-47.1.0-installer.jar"));
        let installer_args = java_requests[0]
            .program_args
            .iter()
            .skip(1)
            .map(String::as_str)
            .collect::<Vec<_>>();
        assert_eq!(installer_args, vec!["--installClient", "game"]);
        assert_eq!(
            harness.progress.messages(),
            vec![
                "Downloading NeoForge installer".to_string(),
                "Running NeoForge installer".to_string(),
                "NeoForge installer finished".to_string()
            ]
        );
    }

    #[test]
    fn installs_legacy_neoforge_with_the_legacy_installer_shape() {
        let harness = Harness::new();
        let (expected_id, loader_version) = seeded_install(&harness, "1.20.1", "1.20.1-47.1.0");
        let request = LoaderInstallRequest::new("1.20.1", loader_version.as_str(), "java");
        let output = NeoForgeLoaderProvider::new()
            .install(&harness.install_context(), &request)
            .expect("legacy neoforge install");
        assert_eq!(output.installed_version_id, expected_id);
        assert_eq!(
            harness.installer.downloads.lock().unwrap()[0].url,
            "https://maven.neoforged.net/releases/net/neoforged/forge/1.20.1-47.1.0/forge-1.20.1-47.1.0-installer.jar"
        );
    }

    #[test]
    fn deadline_and_cancellation_fail_before_network_io() {
        let harness = Harness::new();
        let expired = LoaderListContext::new(
            &harness.metadata,
            &harness.workspace,
            &harness.progress,
            &harness.logger,
            flint_provider_api::LoaderTimeouts::default(),
            Deadline::at(Instant::now() - Duration::from_secs(1)),
            Cancellation::new(),
        );
        let error = NeoForgeLoaderProvider::new()
            .list_versions(&expired, &LoaderListRequest::new("1.21.1"))
            .expect_err("expired deadline");
        assert_eq!(error, "Operation deadline exceeded.");
        assert_eq!(harness.metadata.requests_len(), 0);

        harness.cancellation.cancel();
        let error = NeoForgeLoaderProvider::new()
            .list_versions(&harness.list_context(), &LoaderListRequest::new("1.21.1"))
            .expect_err("cancelled request");
        assert_eq!(error, "Operation was cancelled.");
        assert_eq!(harness.metadata.requests_len(), 0);
    }

    #[test]
    fn parses_and_deduplicates_neoforge_xml_versions() {
        let xml = "<metadata><versioning><versions>\
            <version> 21.1.80 </version>\
            <version>21.1.80</version>\
            <version>21.1.81</version>\
        </versions></versioning></metadata>";
        assert_eq!(
            parse_xml_versions(xml),
            vec!["21.1.80".to_string(), "21.1.81".to_string()]
        );
    }

    #[test]
    fn provider_is_object_safe() {
        fn assert_send_sync<T: ?Sized + Send + Sync>() {}
        assert_send_sync::<dyn LoaderProvider>();
        let _provider: Box<dyn LoaderProvider> = Box::new(NeoForgeLoaderProvider::new());
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
        ] {
            assert!(!source.contains(forbidden), "forbidden API token present: {forbidden}");
        }
    }
}
