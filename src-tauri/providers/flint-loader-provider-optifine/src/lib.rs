//! OptiFine implementation of the provider-neutral loader contract.

use flint_core::{
    minecraft::{
        fetch_text, install_java_installer, validate_game_version, InstallerContext,
        InstallerRequest, MAX_HTML_BYTES,
    },
    paths::component_is_safe,
};
use flint_provider_api::{
    loader_descriptor_for, LoaderInstallContext, LoaderInstallOutputData, LoaderInstallRequest,
    LoaderListContext, LoaderListRequest, LoaderProvider, LoaderVersionData, Provider,
    ProviderDescriptor, OPTIFINE_CAPABILITIES, OPTIFINE_PROVIDER_ID,
};
use regex::Regex;
use std::{collections::HashSet, path::Path};

const OPTIFINE_DOWNLOADS: &str = "https://optifine.net/downloads";
const DOWNLOADS_SERVICE: &str = "OptiFine downloads";
const INSTALLER_SERVICE: &str = "OptiFine";

pub const DESCRIPTOR: ProviderDescriptor =
    loader_descriptor_for(OPTIFINE_PROVIDER_ID, OPTIFINE_CAPABILITIES);

/// Independent OptiFine loader provider.
#[derive(Debug, Clone, Copy, Default)]
pub struct OptiFineProvider;

impl OptiFineProvider {
    pub const fn new() -> Self {
        Self
    }
}

impl Provider for OptiFineProvider {
    fn descriptor(&self) -> ProviderDescriptor {
        DESCRIPTOR
    }
}

impl LoaderProvider for OptiFineProvider {
    fn list_versions(
        &self,
        context: &LoaderListContext<'_>,
        request: &LoaderListRequest,
    ) -> Result<Vec<LoaderVersionData>, String> {
        list_optifine(context, request)
    }

    fn install(
        &self,
        context: &LoaderInstallContext<'_>,
        request: &LoaderInstallRequest,
    ) -> Result<LoaderInstallOutputData, String> {
        install_optifine(context, request)
    }
}

fn list_optifine(
    context: &LoaderListContext<'_>,
    request: &LoaderListRequest,
) -> Result<Vec<LoaderVersionData>, String> {
    validate_game_version(&request.game_version)?;
    context.check_control()?;
    let html = fetch_text(
        context.network(),
        OPTIFINE_DOWNLOADS,
        DOWNLOADS_SERVICE,
        MAX_HTML_BYTES,
        context.timeouts().metadata,
        context.deadline(),
        context.cancellation(),
    )?;
    let mut result = Vec::new();
    for (index, filename) in extract_optifine_filenames(&html, &request.game_version)
        .into_iter()
        .enumerate()
    {
        let Some(loader_version) = optifine_loader_version(&filename, &request.game_version) else {
            continue;
        };
        if !component_is_safe(&loader_version) {
            continue;
        }
        result.push(LoaderVersionData {
            version: loader_version.clone(),
            label: format!("OptiFine {loader_version}"),
            recommended: index == 0,
            game_version: request.game_version.clone(),
            release_time: None,
        });
    }
    Ok(result)
}

fn install_optifine(
    context: &LoaderInstallContext<'_>,
    request: &LoaderInstallRequest,
) -> Result<LoaderInstallOutputData, String> {
    validate_game_version(&request.game_version)?;
    if !component_is_safe(request.loader_version.trim()) {
        return Err("The selected loader version is invalid.".to_string());
    }
    context.check_control()?;
    let html = fetch_text(
        context.metadata_network(),
        OPTIFINE_DOWNLOADS,
        DOWNLOADS_SERVICE,
        MAX_HTML_BYTES,
        context.timeouts().metadata,
        context.deadline(),
        context.cancellation(),
    )?;
    let Some(filename) = extract_optifine_filenames(&html, &request.game_version)
        .into_iter()
        .find(|filename| {
            optifine_loader_version(filename, &request.game_version)
                .map(|version| version.eq_ignore_ascii_case(&request.loader_version))
                .unwrap_or(false)
        })
    else {
        return Err("The selected OptiFine version is no longer listed upstream.".to_string());
    };

    // OptiFine exposes ad-gated links. Only the HTTPS endpoint is attempted,
    // and the core installer path verifies that the response is a real archive
    // before the Java process is started.
    let url = format!("https://optifine.net/adloadx?f={filename}");
    let installer_name = format!("optifine-{}.jar", request.loader_version);
    let expected_id = format!(
        "{}-OptiFine_{}",
        request.game_version, request.loader_version
    );
    let log_label = format!(
        "optifine-{}-{}",
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
        &installer_name,
        vec!["--install".to_string()],
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

fn extract_optifine_filenames(html: &str, game_version: &str) -> Vec<String> {
    let game = regex::escape(game_version);
    let pattern = format!(r"(?i)(?:preview_)?OptiFine_{game}_[A-Za-z0-9._-]+\.jar");
    let Ok(regex) = Regex::new(&pattern) else {
        return Vec::new();
    };
    let mut seen = HashSet::new();
    let mut result = Vec::new();
    for found in regex.find_iter(html) {
        let filename = found.as_str().to_string();
        if seen.insert(filename.clone()) {
            result.push(filename);
        }
    }
    result
}

fn optifine_loader_version(filename: &str, game_version: &str) -> Option<String> {
    let lower = filename.to_ascii_lowercase();
    let prefix_length = if lower.starts_with("preview_optifine_") {
        "preview_optifine_".len()
    } else if lower.starts_with("optifine_") {
        "optifine_".len()
    } else {
        return None;
    };
    let without_prefix = filename.get(prefix_length..)?;
    let expected = format!("{game_version}_");
    let without_game = without_prefix.strip_prefix(&expected)?;
    let version = without_game.strip_suffix(".jar")?;
    if version.is_empty() {
        None
    } else {
        Some(version.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flint_core::ports::{
        Cancellation, Deadline, NoopLogger, ProcessOutput, ProgressSink, ProgressUpdate,
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
                java: FakeJavaRunner::with_output(ProcessOutput {
                    status_code: Some(0),
                    stdout: Vec::new(),
                    stderr: Vec::new(),
                    timed_out: false,
                }),
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

    const DOWNLOADS_HTML: &str = r#"<a href="x">OptiFine_1.20.1_HD_U_I6.jar</a>
        <a href="y">OptiFine_1.20.1_HD_U_I5.jar</a>
        <a href="z">OptiFine_1.20.2_HD_U_I7.jar</a>"#;

    fn seed_installer(harness: &Harness, version: &str, bytes: &[u8]) {
        harness.workspace.insert_file(
            PathBuf::from("game")
                .join("loader-installers")
                .join(format!("optifine-{version}.jar")),
            bytes.to_vec(),
        );
    }

    fn seed_profile(harness: &Harness, game_version: &str, version: &str) {
        let expected_id = format!("{game_version}-OptiFine_{version}");
        harness.workspace.insert_file(
            PathBuf::from("game")
                .join("versions")
                .join(&expected_id)
                .join(format!("{expected_id}.json")),
            br#"{"id":"installed","inheritsFrom":"1.20.1"}"#.to_vec(),
        );
    }

    #[test]
    fn descriptor_declares_the_optifine_contract() {
        let descriptor = OptiFineProvider::new().descriptor();
        assert_eq!(descriptor.id, OPTIFINE_PROVIDER_ID);
        assert_eq!(descriptor.provider_api, flint_provider_api::LOADER_PROVIDER_API);
        assert_eq!(
            descriptor.capabilities[0].selection_keys,
            flint_provider_api::OPTIFINE_SELECTION_KEYS
        );
    }

    #[test]
    fn lists_matching_optifine_files_in_page_order_and_recommends_first() {
        let harness = Harness::new();
        harness.metadata.push_response(DOWNLOADS_HTML.as_bytes().to_vec());
        let versions = OptiFineProvider::new()
            .list_versions(
                &harness.list_context(),
                &LoaderListRequest::new("1.20.1"),
            )
            .expect("optifine versions");
        assert_eq!(
            versions.iter().map(|version| version.version.as_str()).collect::<Vec<_>>(),
            vec!["HD_U_I6", "HD_U_I5"]
        );
        assert!(versions[0].recommended);
        assert!(!versions[1].recommended);
        assert_eq!(
            harness.metadata.requests.lock().unwrap()[0].timeout,
            Duration::from_secs(75)
        );
    }

    #[test]
    fn list_translates_transport_failures_using_shared_guidance() {
        let harness = Harness::new();
        harness.metadata.push_error(
            "OptiFine downloads request failed: connection reset while opening https://optifine.net/downloads?token=secret",
        );
        let error = OptiFineProvider::new()
            .list_versions(
                &harness.list_context(),
                &LoaderListRequest::new("1.20.1"),
            )
            .expect_err("transport failure must fail");
        assert_eq!(
            error,
            "Cannot contact OptiFine downloads: connection reset while opening [redacted-url]"
        );
    }

    #[test]
    fn installs_optifine_through_the_verified_https_installer_flow() {
        let harness = Harness::new();
        seed_installer(&harness, "HD_U_I6", b"PK\x03\x04optifine");
        seed_profile(&harness, "1.20.1", "HD_U_I6");
        harness
            .metadata
            .push_response(DOWNLOADS_HTML.as_bytes().to_vec());
        let request = LoaderInstallRequest::new("1.20.1", "HD_U_I6", "java");
        let output = OptiFineProvider::new()
            .install(&harness.install_context(), &request)
            .expect("optifine install");
        assert_eq!(
            output.installed_version_id,
            "1.20.1-OptiFine_HD_U_I6"
        );
        assert!(output.log_path.as_deref().unwrap_or_default().contains("flint-installer-optifine-"));
        let downloads = harness.installer.downloads.lock().unwrap();
        assert_eq!(downloads.len(), 1);
        assert_eq!(
            downloads[0].url,
            "https://optifine.net/adloadx?f=OptiFine_1.20.1_HD_U_I6.jar"
        );
        assert_eq!(downloads[0].max_bytes, MAX_INSTALLER_BYTES);
        assert_eq!(downloads[0].timeout, Duration::from_secs(600));
        let java_requests = harness.java.requests.lock().unwrap();
        assert_eq!(java_requests.len(), 1);
        assert_eq!(java_requests[0].timeout, Duration::from_secs(600));
        let installer_args = java_requests[0]
            .program_args
            .iter()
            .skip(1)
            .map(String::as_str)
            .collect::<Vec<_>>();
        assert_eq!(installer_args, vec!["--install"]);
        assert_eq!(
            harness.progress.messages(),
            vec![
                "Downloading OptiFine installer".to_string(),
                "Running OptiFine installer".to_string(),
                "OptiFine installer finished".to_string()
            ]
        );
    }

    #[test]
    fn rejects_non_jar_response_without_starting_java_and_removes_it() {
        let harness = Harness::new();
        seed_installer(&harness, "HD_U_I6", b"<html>blocked</html>");
        harness
            .metadata
            .push_response(DOWNLOADS_HTML.as_bytes().to_vec());
        let request = LoaderInstallRequest::new("1.20.1", "HD_U_I6", "java");
        let error = OptiFineProvider::new()
            .install(&harness.install_context(), &request)
            .expect_err("non-jar optifine response");
        assert_eq!(
            error,
            "OptiFine upstream did not return a direct JAR. Download and install it manually, then retry instance discovery."
        );
        assert_eq!(harness.java.requests_len(), 0);
        let installer_path = PathBuf::from("game")
            .join("loader-installers")
            .join("optifine-HD_U_I6.jar");
        assert!(harness.workspace.file_bytes(&installer_path).is_none());
    }

    #[test]
    fn reports_missing_optifine_versions_without_downloading() {
        let harness = Harness::new();
        harness
            .metadata
            .push_response(DOWNLOADS_HTML.as_bytes().to_vec());
        let request = LoaderInstallRequest::new("1.20.1", "HD_U_X9", "java");
        let error = OptiFineProvider::new()
            .install(&harness.install_context(), &request)
            .expect_err("missing optifine version");
        assert_eq!(
            error,
            "The selected OptiFine version is no longer listed upstream."
        );
        assert!(harness.installer.downloads.lock().unwrap().is_empty());
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
        assert_eq!(
            OptiFineProvider::new()
                .list_versions(&expired, &LoaderListRequest::new("1.20.1"))
                .expect_err("expired"),
            "Operation deadline exceeded."
        );
        assert_eq!(harness.metadata.requests_len(), 0);
        harness.cancellation.cancel();
        assert_eq!(
            OptiFineProvider::new()
                .list_versions(&harness.list_context(), &LoaderListRequest::new("1.20.1"))
                .expect_err("cancelled"),
            "Operation was cancelled."
        );
        assert_eq!(harness.metadata.requests_len(), 0);
    }

    #[test]
    fn parses_preview_names_and_rejects_other_game_versions() {
        let html = r#"OptiFine_1.20.1_HD_U_I6.jar
            preview_OptiFine_1.20.1_HD_U_P1.jar
            OptiFine_1.20.2_HD_U_I7.jar"#;
        let filenames = extract_optifine_filenames(html, "1.20.1");
        assert_eq!(filenames.len(), 2);
        assert_eq!(
            optifine_loader_version(&filenames[0], "1.20.1").as_deref(),
            Some("HD_U_I6")
        );
        assert_eq!(
            optifine_loader_version(&filenames[1], "1.20.1").as_deref(),
            Some("HD_U_P1")
        );
    }

    #[test]
    fn provider_is_object_safe() {
        fn assert_send_sync<T: ?Sized + Send + Sync>() {}
        assert_send_sync::<dyn LoaderProvider>();
        let _provider: Box<dyn LoaderProvider> = Box::new(OptiFineProvider::new());
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
