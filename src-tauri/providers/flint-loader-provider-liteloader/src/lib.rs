//! LiteLoader implementation of the provider-neutral loader contract.

use flint_core::{
    minecraft::{
        fetch_json, fetch_text, force_https_url, is_jar_zip_prefix, library_file,
        read_version_profile, report_progress, sort_loader_versions_desc,
        translate_network_error, upgrade_library_url, validate_game_version, validate_version_id,
        write_version_profile, NetworkOperation, MAX_HTML_BYTES, MAX_INSTALLER_BYTES,
        MAX_JSON_BYTES,
    },
    paths::component_is_safe,
    ports::{DownloadRequest, NetworkPort, ServiceLabel},
};
use flint_provider_api::{
    loader_descriptor_for, LoaderInstallContext, LoaderInstallOutputData, LoaderInstallRequest,
    LoaderListContext, LoaderListRequest, LoaderProvider, LoaderVersionData, Provider,
    ProviderDescriptor, LITELOADER_CAPABILITIES, LITELOADER_PROVIDER_ID,
};
use regex::Regex;
use serde_json::{json, Value};
use std::collections::HashSet;

const LITELOADER_METADATA: &str = "https://dl.liteloader.com/versions/versions.json";
const METADATA_SERVICE: &str = "LiteLoader";
const REPOSITORY_SERVICE: &str = "LiteLoader repository";

pub const DESCRIPTOR: ProviderDescriptor =
    loader_descriptor_for(LITELOADER_PROVIDER_ID, LITELOADER_CAPABILITIES);

/// Independent LiteLoader provider.
#[derive(Debug, Clone, Copy, Default)]
pub struct LiteLoaderProvider;

impl LiteLoaderProvider {
    pub const fn new() -> Self {
        Self
    }
}

impl Provider for LiteLoaderProvider {
    fn descriptor(&self) -> ProviderDescriptor {
        DESCRIPTOR
    }
}

impl LoaderProvider for LiteLoaderProvider {
    fn list_versions(
        &self,
        context: &LoaderListContext<'_>,
        request: &LoaderListRequest,
    ) -> Result<Vec<LoaderVersionData>, String> {
        list_liteloader(context, request)
    }

    fn install(
        &self,
        context: &LoaderInstallContext<'_>,
        request: &LoaderInstallRequest,
    ) -> Result<LoaderInstallOutputData, String> {
        install_liteloader(context, request)
    }
}

fn list_liteloader(
    context: &LoaderListContext<'_>,
    request: &LoaderListRequest,
) -> Result<Vec<LoaderVersionData>, String> {
    validate_game_version(&request.game_version)?;
    context.check_control()?;
    let metadata = fetch_json(
        context.network(),
        LITELOADER_METADATA,
        METADATA_SERVICE,
        MAX_JSON_BYTES,
        context.timeouts().metadata,
        context.deadline(),
        context.cancellation(),
    )?;
    let mut result = Vec::new();
    for entry in collect_liteloader_entries(&metadata, &request.game_version) {
        if !component_is_safe(&entry.version) {
            continue;
        }
        result.push(LoaderVersionData {
            version: entry.version.clone(),
            label: format!("LiteLoader {}", entry.version),
            recommended: entry.latest,
            game_version: request.game_version.clone(),
            release_time: entry.timestamp.clone(),
        });
    }
    sort_loader_versions_desc(&mut result, |version| version.version.as_str());
    result.sort_by(|left, right| right.recommended.cmp(&left.recommended));
    Ok(result)
}

fn install_liteloader(
    context: &LoaderInstallContext<'_>,
    request: &LoaderInstallRequest,
) -> Result<LoaderInstallOutputData, String> {
    validate_game_version(&request.game_version)?;
    let loader_version = request.loader_version.trim();
    if !component_is_safe(loader_version) {
        return Err("The selected loader version is invalid.".to_string());
    }
    context.check_control()?;
    let metadata = fetch_json(
        context.metadata_network(),
        LITELOADER_METADATA,
        METADATA_SERVICE,
        MAX_JSON_BYTES,
        context.timeouts().metadata,
        context.deadline(),
        context.cancellation(),
    )?;
    let Some(entry) = collect_liteloader_entries(&metadata, &request.game_version)
        .into_iter()
        .find(|entry| {
            entry.version.eq_ignore_ascii_case(loader_version)
                || entry
                    .build
                    .as_deref()
                    .map(|build| build.eq_ignore_ascii_case(loader_version))
                    .unwrap_or(false)
        })
    else {
        return Err(
            "The selected LiteLoader version is not available for this Minecraft version."
                .to_string(),
        );
    };
    let game_node = metadata
        .get("versions")
        .and_then(|versions| versions.get(&request.game_version))
        .ok_or_else(|| {
            "LiteLoader metadata has no entry for this Minecraft version.".to_string()
        })?;
    let installer_url = resolve_liteloader_jar_url(context, game_node, &entry, &request.game_version)?;
    let workspace = context.scoped_workspace();
    let relative_jar = format!(
        "com/mumfrey/liteloader/{loader_version}/liteloader-{loader_version}.jar"
    );
    let jar_relative = format!("libraries/{relative_jar}");
    let jar_path = workspace
        .join(&jar_relative)
        .map_err(|_| "Unsafe LiteLoader artifact path.".to_string())?;
    report_progress(context.progress(), 0, Some(1), "Downloading LiteLoader");
    let main_download = DownloadRequest::new(
        installer_url.clone(),
        jar_path,
        None,
        ServiceLabel::new(METADATA_SERVICE)
            .map_err(|error| format!("Invalid LiteLoader service label: {error}"))?,
        MAX_INSTALLER_BYTES,
        context.timeouts().metadata,
    );
    context.check_control()?;
    let main_service = main_download.service.as_str().to_string();
    context
        .metadata_network()
        .download_atomic(&main_download)
        .map_err(|error| {
            translate_network_error(
                &main_service,
                NetworkOperation::Download,
                error,
            )
        })?;
    context.check_control()?;
    let prefix = workspace.read_jar_prefix(&jar_relative)?;
    if !is_jar_zip_prefix(&prefix) {
        let _ = workspace.remove_file(&jar_relative);
        return Err("LiteLoader upstream returned a non-JAR file.".to_string());
    }
    report_progress(context.progress(), 1, Some(1), "LiteLoader downloaded");

    let mut profile_libraries = vec![json!({
        "name": format!("com.mumfrey:liteloader:{loader_version}"),
        "downloads": {
            "artifact": {
                "path": relative_jar,
                "url": installer_url
            }
        }
    })];
    let mut additional_libraries = entry.libraries.clone();
    if let Some(common) = game_node
        .get("snapshots")
        .and_then(|snapshots| snapshots.get("libraries"))
        .and_then(Value::as_array)
    {
        additional_libraries.extend(common.iter().cloned());
    }
    let additional_libraries = dedupe_libraries(additional_libraries);
    let total = additional_libraries.len() as u64;
    for (index, mut library) in additional_libraries.into_iter().enumerate() {
        context.check_control()?;
        upgrade_library_url(&mut library);
        if let Some(file) = library_file(&library, None)? {
            let Some(url) = file.url.as_deref() else {
                return Err(format!("LiteLoader dependency has no URL: {}", file.path));
            };
            let destination_relative = format!("libraries/{}", file.path);
            let destination = workspace.join(&destination_relative).map_err(|_| {
                format!("Unsafe LiteLoader dependency path: {}", file.path)
            })?;
            let download = DownloadRequest::new(
                force_https_url(url),
                destination,
                file.sha1,
                ServiceLabel::new("LiteLoader dependency")
                    .map_err(|error| format!("Invalid LiteLoader service label: {error}"))?,
                MAX_INSTALLER_BYTES,
                context.timeouts().metadata,
            );
            let dependency_service = download.service.as_str().to_string();
            context
                .metadata_network()
                .download_atomic(&download)
                .map_err(|error| {
                    translate_network_error(
                        &dependency_service,
                        NetworkOperation::Download,
                        error,
                    )
                })?;
        }
        profile_libraries.push(library);
        report_progress(
            context.progress(),
            index as u64 + 1,
            Some(total),
            format!("Downloaded LiteLoader dependency {}", index + 1),
        );
    }

    let tweak_class = entry
        .tweak_class
        .unwrap_or_else(|| "com.mumfrey.liteloader.launch.LiteLoaderTweaker".to_string());
    let parent = read_version_profile(
        context.root(),
        context.workspace(),
        &request.game_version,
    )?;
    let installed_id = format!("{}-liteloader-{loader_version}", request.game_version);
    validate_version_id(&installed_id)?;
    let mut profile = json!({
        "id": installed_id,
        "inheritsFrom": request.game_version,
        "type": "release",
        "libraries": profile_libraries
    });
    if let Some(main_class) = parent.get("mainClass").and_then(Value::as_str) {
        profile["mainClass"] = json!(main_class);
    }
    if let Some(arguments) = parent.get("minecraftArguments").and_then(Value::as_str) {
        profile["minecraftArguments"] = json!(format!("{arguments} --tweakClass {tweak_class}"));
    } else {
        profile["arguments"] = json!({"game": ["--tweakClass", tweak_class]});
    }
    write_version_profile(
        context.root(),
        context.workspace(),
        &installed_id,
        &profile,
    )?;
    report_progress(
        context.progress(),
        1,
        Some(1),
        "LiteLoader installation complete",
    );
    Ok(LoaderInstallOutputData {
        installed_version_id: installed_id,
        log_path: None,
    })
}

#[derive(Debug, Clone)]
struct LiteEntry {
    version: String,
    metadata_version: String,
    build: Option<String>,
    file: Option<String>,
    latest: bool,
    timestamp: Option<String>,
    tweak_class: Option<String>,
    libraries: Vec<Value>,
}

fn collect_liteloader_entries(metadata: &Value, game_version: &str) -> Vec<LiteEntry> {
    let Some(game_node) = metadata
        .get("versions")
        .and_then(|versions| versions.get(game_version))
    else {
        return Vec::new();
    };
    let mut raw_entries = Vec::new();
    walk_liteloader_objects(game_node, false, &mut raw_entries);
    raw_entries.sort_by(|left, right| right.1.cmp(&left.1));
    let mut seen = HashSet::new();
    let mut result = Vec::new();
    for (object, latest) in raw_entries {
        let Some(version) = object.get("version").and_then(Value::as_str) else {
            continue;
        };
        if !seen.insert(version.to_string()) {
            continue;
        }
        let build = object
            .get("build")
            .and_then(Value::as_str)
            .map(str::to_string);
        result.push(LiteEntry {
            version: build.clone().unwrap_or_else(|| version.to_string()),
            metadata_version: version.to_string(),
            build,
            file: object
                .get("file")
                .and_then(Value::as_str)
                .map(str::to_string),
            latest,
            timestamp: object
                .get("timestamp")
                .and_then(Value::as_str)
                .map(str::to_string),
            tweak_class: object
                .get("tweakClass")
                .and_then(Value::as_str)
                .map(str::to_string),
            libraries: object
                .get("libraries")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default(),
        });
    }
    result
}

fn walk_liteloader_objects(value: &Value, latest_parent: bool, result: &mut Vec<(Value, bool)>) {
    match value {
        Value::Object(object) => {
            let is_entry = object.contains_key("tweakClass") && object.contains_key("version");
            if is_entry {
                result.push((value.clone(), latest_parent));
            }
            for (key, child) in object {
                walk_liteloader_objects(child, latest_parent || key == "latest", result);
            }
        }
        Value::Array(array) => {
            for child in array {
                walk_liteloader_objects(child, latest_parent, result);
            }
        }
        _ => {}
    }
}

fn resolve_liteloader_jar_url(
    context: &LoaderInstallContext<'_>,
    game_node: &Value,
    entry: &LiteEntry,
    game_version: &str,
) -> Result<String, String> {
    let repo = game_node
        .get("repo")
        .and_then(|repo| repo.get("url"))
        .and_then(Value::as_str)
        .unwrap_or("http://repo.mumfrey.com/content/repositories/snapshots/");
    let repo = force_https_url(repo);
    if entry.metadata_version.contains("SNAPSHOT") {
        let directory = format!(
            "{}/com/mumfrey/liteloader/{}/",
            repo.trim_end_matches('/'),
            entry.metadata_version
        );
        let listing = fetch_text(
            context.metadata_network(),
            &directory,
            REPOSITORY_SERVICE,
            MAX_HTML_BYTES,
            context.timeouts().metadata,
            context.deadline(),
            context.cancellation(),
        )?;
        let Ok(regex) = Regex::new(r#"href="[^"]*/(liteloader-[^"/]+-release\.jar)""#) else {
            return Err("Cannot parse the LiteLoader repository listing.".to_string());
        };
        let mut names: Vec<String> = regex
            .captures_iter(&listing)
            .filter_map(|capture| capture.get(1).map(|value| value.as_str().to_string()))
            .collect();
        names.sort();
        let name = names
            .pop()
            .ok_or_else(|| "LiteLoader repository did not expose a safe JAR build.".to_string())?;
        Ok(format!("{directory}{name}"))
    } else {
        let filename = entry
            .file
            .clone()
            .unwrap_or_else(|| format!("liteloader-{game_version}.jar"));
        if !component_is_safe(&filename) {
            return Err("LiteLoader metadata contained an unsafe JAR file name.".to_string());
        }
        Ok(format!(
            "{}/{game_version}/{filename}",
            repo.trim_end_matches('/')
        ))
    }
}

fn dedupe_libraries(libraries: Vec<Value>) -> Vec<Value> {
    let mut seen = HashSet::new();
    let mut result = Vec::new();
    for library in libraries {
        let key = library
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_ascii_lowercase();
        if key.is_empty() || seen.insert(key) {
            result.push(library);
        }
    }
    result
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

    fn metadata_for(
        file: &str,
        version: &str,
        build: Option<&str>,
        libraries: Value,
    ) -> Value {
        let mut entry = json!({
            "version": version,
            "file": file,
            "tweakClass": "com.mumfrey.liteloader.launch.LiteLoaderTweaker",
            "libraries": libraries
        });
        if let Some(build) = build {
            entry["build"] = json!(build);
        }
        json!({
            "versions": {
                "1.12.2": {
                    "repo": {"url": "http://repo.example/"},
                    "snapshots": {"libraries": []},
                    "latest": entry
                }
            }
        })
    }

    fn seed_parent(harness: &Harness, game_version: &str) {
        harness.workspace.insert_file(
            PathBuf::from("game")
                .join("versions")
                .join(game_version)
                .join(format!("{game_version}.json")),
            br#"{"mainClass":"net.minecraft.client.main.Main","minecraftArguments":"--username ${auth_player_name}"}"#
                .to_vec(),
        );
    }

    fn seed_main_jar(harness: &Harness, loader_version: &str) {
        harness.workspace.insert_file(
            PathBuf::from("game")
                .join("libraries")
                .join("com/mumfrey/liteloader")
                .join(loader_version)
                .join(format!("liteloader-{loader_version}.jar")),
            b"PK\x03\x04liteloader".to_vec(),
        );
    }

    #[test]
    fn descriptor_declares_the_liteloader_contract() {
        let descriptor = LiteLoaderProvider::new().descriptor();
        assert_eq!(descriptor.id, LITELOADER_PROVIDER_ID);
        assert_eq!(descriptor.provider_api, flint_provider_api::LOADER_PROVIDER_API);
        assert_eq!(
            descriptor.capabilities[0].selection_keys,
            flint_provider_api::LITELOADER_SELECTION_KEYS
        );
    }

    #[test]
    fn lists_liteloader_entries_with_timestamp_and_recommended_first() {
        let harness = Harness::new();
        harness.metadata.push_response(
            serde_json::to_vec(&json!({
                "versions": {
                    "1.12.2": {
                        "snapshots": {
                            "com.mumfrey:liteloader": {
                                "latest": {
                                    "version": "1.12.2-SNAPSHOT",
                                    "build": "1.12.2-SNAPSHOT-r4",
                                    "timestamp": "2024-01-02T00:00:00Z",
                                    "tweakClass": "com.mumfrey.liteloader.launch.LiteLoaderTweaker",
                                    "libraries": []
                                },
                                "older": {
                                    "version": "1.12.2-SNAPSHOT",
                                    "build": "1.12.2-SNAPSHOT-r3",
                                    "tweakClass": "com.mumfrey.liteloader.launch.LiteLoaderTweaker",
                                    "libraries": []
                                }
                            }
                        }
                    }
                }
            }))
            .unwrap(),
        );
        let versions = LiteLoaderProvider::new()
            .list_versions(
                &harness.list_context(),
                &LoaderListRequest::new("1.12.2"),
            )
            .expect("liteloader versions");
        assert_eq!(versions.len(), 1);
        assert_eq!(versions[0].version, "1.12.2-SNAPSHOT-r4");
        assert!(versions[0].recommended);
        assert_eq!(
            versions[0].release_time.as_deref(),
            Some("2024-01-02T00:00:00Z")
        );
        assert_eq!(
            harness.metadata.requests.lock().unwrap()[0].timeout,
            Duration::from_secs(75)
        );
    }

    #[test]
    fn list_translates_http_not_found_using_shared_guidance() {
        let harness = Harness::new();
        harness.metadata.push_error(
            "LiteLoader request failed: HTTP status client error (404 Not Found)",
        );
        let error = LiteLoaderProvider::new()
            .list_versions(
                &harness.list_context(),
                &LoaderListRequest::new("1.12.2"),
            )
            .expect_err("HTTP 404 must fail");
        assert_eq!(error, "LiteLoader did not find the requested resource.");
    }

    #[test]
    fn installs_liteloader_with_safe_profile_and_library_paths() {
        let harness = Harness::new();
        seed_parent(&harness, "1.12.2");
        seed_main_jar(&harness, "1.12.2");
        harness.metadata.push_response(
            serde_json::to_vec(&metadata_for(
                "liteloader-release.jar",
                "1.12.2",
                None,
                json!([{"name":"example:dep:1.0","url":"http://repo.example/"}]),
            ))
            .unwrap(),
        );
        let request = LoaderInstallRequest::new("1.12.2", "1.12.2", "java");
        let output = LiteLoaderProvider::new()
            .install(&harness.install_context(), &request)
            .expect("liteloader install");
        assert_eq!(output.installed_version_id, "1.12.2-liteloader-1.12.2");
        assert_eq!(output.log_path, None);
        let downloads = harness.metadata.downloads.lock().unwrap();
        assert_eq!(downloads.len(), 2);
        assert_eq!(
            downloads[0].url,
            "https://repo.example/1.12.2/liteloader-release.jar"
        );
        assert_eq!(
            downloads[1].url,
            "https://repo.example/example/dep/1.0/dep-1.0.jar"
        );
        assert_eq!(downloads[0].max_bytes, MAX_INSTALLER_BYTES);
        assert_eq!(downloads[0].timeout, Duration::from_secs(75));
        drop(downloads);
        let profile_path = PathBuf::from("game")
            .join("versions")
            .join("1.12.2-liteloader-1.12.2")
            .join("1.12.2-liteloader-1.12.2.json");
        let profile: Value = serde_json::from_slice(
            &harness.workspace.file_bytes(&profile_path).expect("written profile"),
        )
        .expect("profile JSON");
        assert_eq!(profile["inheritsFrom"].as_str(), Some("1.12.2"));
        assert_eq!(profile["mainClass"].as_str(), Some("net.minecraft.client.main.Main"));
        assert!(profile["minecraftArguments"]
            .as_str()
            .unwrap_or_default()
            .contains("--tweakClass"));
        assert_eq!(profile["libraries"].as_array().unwrap().len(), 2);
        assert_eq!(
            harness.progress.messages(),
            vec![
                "Downloading LiteLoader".to_string(),
                "LiteLoader downloaded".to_string(),
                "Downloaded LiteLoader dependency 1".to_string(),
                "LiteLoader installation complete".to_string()
            ]
        );
    }

    #[test]
    fn resolves_snapshot_jar_from_https_repository_listing() {
        let harness = Harness::new();
        seed_parent(&harness, "1.12.2");
        seed_main_jar(&harness, "1.12.2-SNAPSHOT-r4");
        harness.metadata.push_response(
            serde_json::to_vec(&metadata_for(
                "ignored.jar",
                "1.12.2-SNAPSHOT",
                Some("1.12.2-SNAPSHOT-r4"),
                json!([]),
            ))
            .unwrap(),
        );
        harness.metadata.push_response(
            br#"<a href="https://repo.example/com/mumfrey/liteloader/1.12.2-SNAPSHOT/liteloader-1.12.2-SNAPSHOT-r3-release.jar">r3</a>
                <a href="https://repo.example/com/mumfrey/liteloader/1.12.2-SNAPSHOT/liteloader-1.12.2-SNAPSHOT-r4-release.jar">r4</a>"#
                .to_vec(),
        );
        let request = LoaderInstallRequest::new("1.12.2", "1.12.2-SNAPSHOT-r4", "java");
        LiteLoaderProvider::new()
            .install(&harness.install_context(), &request)
            .expect("snapshot install");
        let downloads = harness.metadata.downloads.lock().unwrap();
        assert_eq!(
            downloads[0].url,
            "https://repo.example/com/mumfrey/liteloader/1.12.2-SNAPSHOT/liteloader-1.12.2-SNAPSHOT-r4-release.jar"
        );
        let requests = harness.metadata.requests.lock().unwrap();
        assert_eq!(requests.len(), 2);
        assert_eq!(
            requests[1].url,
            "https://repo.example/com/mumfrey/liteloader/1.12.2-SNAPSHOT/"
        );
    }

    #[test]
    fn rejects_unsafe_metadata_file_names_without_downloading() {
        let harness = Harness::new();
        harness.metadata.push_response(
            serde_json::to_vec(&metadata_for(
                "../escape.jar",
                "1.12.2",
                None,
                json!([]),
            ))
            .unwrap(),
        );
        let request = LoaderInstallRequest::new("1.12.2", "1.12.2", "java");
        let error = LiteLoaderProvider::new()
            .install(&harness.install_context(), &request)
            .expect_err("unsafe jar name");
        assert_eq!(
            error,
            "LiteLoader metadata contained an unsafe JAR file name."
        );
        assert!(harness.metadata.downloads.lock().unwrap().is_empty());
    }

    #[test]
    fn rejects_non_jar_download_and_removes_it() {
        let harness = Harness::new();
        seed_parent(&harness, "1.12.2");
        harness.metadata.push_response(
            serde_json::to_vec(&metadata_for(
                "liteloader-release.jar",
                "1.12.2",
                None,
                json!([]),
            ))
            .unwrap(),
        );
        let jar_path = PathBuf::from("game")
            .join("libraries")
            .join("com/mumfrey/liteloader/1.12.2/liteloader-1.12.2.jar");
        harness.workspace.insert_file(
            jar_path.clone(),
            b"<html>not a jar</html>".to_vec(),
        );
        let request = LoaderInstallRequest::new("1.12.2", "1.12.2", "java");
        let error = LiteLoaderProvider::new()
            .install(&harness.install_context(), &request)
            .expect_err("non-jar");
        assert_eq!(error, "LiteLoader upstream returned a non-JAR file.");
        assert!(harness.workspace.file_bytes(&jar_path).is_none());
    }

    #[test]
    fn deadline_and_cancellation_fail_before_metadata_network() {
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
            LiteLoaderProvider::new()
                .list_versions(&expired, &LoaderListRequest::new("1.12.2"))
                .expect_err("expired"),
            "Operation deadline exceeded."
        );
        assert_eq!(harness.metadata.requests_len(), 0);
        harness.cancellation.cancel();
        assert_eq!(
            LiteLoaderProvider::new()
                .list_versions(&harness.list_context(), &LoaderListRequest::new("1.12.2"))
                .expect_err("cancelled"),
            "Operation was cancelled."
        );
        assert_eq!(harness.metadata.requests_len(), 0);
    }

    #[test]
    fn provider_is_object_safe() {
        fn assert_send_sync<T: ?Sized + Send + Sync>() {}
        assert_send_sync::<dyn LoaderProvider>();
        let _provider: Box<dyn LoaderProvider> = Box::new(LiteLoaderProvider::new());
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
