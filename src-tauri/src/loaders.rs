mod providers;

use crate::{
    install, java,
    models::{
        LoaderInstallProgress, LoaderInstallRequest, LoaderInstallResult, LoaderKind,
        LoaderVersionInfo,
    },
    net,
    paths::{component_is_safe, resolve_game_dir, safe_join},
    version::{self, Platform},
};
use providers::{LoaderListContext, LoaderProgressSink, LoaderProvider, LoaderProviderContext};
use regex::Regex;
use reqwest::blocking::Client;
use serde_json::{json, Value};
use std::{
    collections::HashSet,
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter};

const FABRIC_LOADER_LIST: &str = "https://meta.fabricmc.net/v2/versions/loader";
const QUILT_LOADER_LIST: &str = "https://meta.quiltmc.org/v3/versions/loader";
const FORGE_MAVEN_METADATA: &str =
    "https://files.minecraftforge.net/net/minecraftforge/forge/maven-metadata.json";
const FORGE_PROMOTIONS: &str =
    "https://files.minecraftforge.net/net/minecraftforge/forge/promotions_slim.json";
const NEOFORGE_MAVEN_METADATA: &str =
    "https://maven.neoforged.net/releases/net/neoforged/neoforge/maven-metadata.xml";
const NEOFORGE_LEGACY_MAVEN_METADATA: &str =
    "https://maven.neoforged.net/releases/net/neoforged/forge/maven-metadata.xml";
const LITELOADER_METADATA: &str = "https://dl.liteloader.com/versions/versions.json";
const OPTIFINE_DOWNLOADS: &str = "https://optifine.net/downloads";
const MAX_JSON_BYTES: u64 = 16 * 1024 * 1024;
const MAX_HTML_BYTES: u64 = 8 * 1024 * 1024;
const MAX_INSTALLER_BYTES: u64 = 128 * 1024 * 1024;
const LOADER_METADATA_HTTP_TIMEOUT_SECONDS: u64 = 75;
const LOADER_INSTALLER_HTTP_TIMEOUT_SECONDS: u64 = 600;
const INSTALLER_TIMEOUT_SECONDS: u64 = 10 * 60;

struct TauriLoaderProgressSink<'a> {
    app: &'a AppHandle,
    request: &'a LoaderInstallRequest,
}

impl LoaderProgressSink for TauriLoaderProgressSink<'_> {
    fn progress(&self, stage: &str, completed: u64, total: u64, message: &str) {
        emit_loader_progress_event(self.app, self.request, stage, completed, total, message);
    }
}

struct NoopLoaderProgressSink;

impl LoaderProgressSink for NoopLoaderProgressSink {
    fn progress(&self, _stage: &str, _completed: u64, _total: u64, _message: &str) {}
}

pub fn list_loader_versions_blocking(
    loader: LoaderKind,
    game_version: &str,
) -> Result<Vec<LoaderVersionInfo>, String> {
    validate_game_version(game_version)?;
    let metadata_client =
        net::http_client(Duration::from_secs(LOADER_METADATA_HTTP_TIMEOUT_SECONDS))?;
    let registry = providers::build_registry()?;
    let provider: &dyn LoaderProvider = providers::select_for_loader(&registry, loader)?;

    let progress = NoopLoaderProgressSink;
    let context = LoaderListContext::new(&metadata_client, game_version, &progress);
    provider.list_versions(&context)
}

pub fn install_loader_blocking(
    app: &AppHandle,
    request: &LoaderInstallRequest,
) -> Result<LoaderInstallResult, String> {
    validate_game_version(&request.game_version)?;
    if !component_is_safe(request.loader_version.trim()) {
        return Err("The selected loader version is invalid.".to_string());
    }
    let root = resolve_game_dir(&request.game_dir);
    fs::create_dir_all(&root)
        .map_err(|error| format!("Cannot create game directory ({}): {error}", root.display()))?;

    let progress = TauriLoaderProgressSink { app, request };
    progress.progress(
        "prepare",
        0,
        1,
        "Preparing loader installation",
    );
    ensure_vanilla_base(app, request, &root, &progress)?;
    progress.progress(
        "prepare",
        1,
        1,
        "Vanilla base version is ready",
    );

    let metadata_client =
        net::http_client(Duration::from_secs(LOADER_METADATA_HTTP_TIMEOUT_SECONDS))?;
    let installer_client =
        net::http_client(Duration::from_secs(LOADER_INSTALLER_HTTP_TIMEOUT_SECONDS))?;
    let registry = providers::build_registry()?;
    let provider: &dyn LoaderProvider = providers::select_for_loader(&registry, request.loader)?;
    let context = LoaderProviderContext::new(
        &metadata_client,
        &installer_client,
        &root,
        request,
        &progress,
    );
    let output = provider.install(&context)?;
    let installed_version_id = output.installed_version_id;
    let log_path = output.log_path;

    let install_path = version::version_dir(&root, &installed_version_id);
    if !version::version_json_path(&root, &installed_version_id).is_file() {
        return Err(format!(
            "Loader installation did not produce version profile {installed_version_id}."
        ));
    }
    progress.progress(
        "complete",
        1,
        1,
        "Loader installation complete",
    );

    Ok(LoaderInstallResult {
        game_dir: root.display().to_string(),
        game_version: request.game_version.clone(),
        loader: request.loader,
        loader_version: request.loader_version.clone(),
        installed_version_id,
        install_path: install_path.display().to_string(),
        log_path,
    })
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
    progress: &dyn LoaderProgressSink,
) -> Result<(), String> {
    let metadata = version::version_json_path(root, &request.game_version);
    let client_jar = version::client_jar_path(root, &request.game_version);
    if metadata.is_file() && client_jar.is_file() {
        return Ok(());
    }
    progress.progress(
        "vanilla",
        0,
        1,
        "Installing the required vanilla version",
    );
    let game_dir = root.display().to_string();
    install::install_blocking(app, &game_dir, &request.game_version)?;
    progress.progress("vanilla", 1, 1, "Vanilla version installed");
    Ok(())
}

fn list_fabric(
    context: &LoaderListContext<'_>,
    loader_kind: LoaderKind,
) -> Result<Vec<LoaderVersionInfo>, String> {
    let client = context.metadata_client();
    let game_version = context.game_version();
    let value = fetch_json(
        client,
        &format!("{FABRIC_LOADER_LIST}/{game_version}"),
        MAX_JSON_BYTES,
        "Fabric Meta",
    )?;
    let mut result = Vec::new();
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
            result.push(LoaderVersionInfo {
                loader: loader_kind,
                version: loader_version.to_string(),
                label: format!("Fabric Loader {loader_version}"),
                recommended: stable,
                game_version: game_version.to_string(),
                release_time: None,
            });
        }
    }
    sort_loader_versions(&mut result);
    Ok(result)
}

fn list_quilt(
    context: &LoaderListContext<'_>,
    loader_kind: LoaderKind,
) -> Result<Vec<LoaderVersionInfo>, String> {
    let client = context.metadata_client();
    let game_version = context.game_version();
    let value = fetch_json(
        client,
        &format!("{QUILT_LOADER_LIST}/{game_version}"),
        MAX_JSON_BYTES,
        "Quilt Meta",
    )?;
    let mut result = Vec::new();
    if let Some(entries) = value.as_array() {
        for (index, entry) in entries.iter().enumerate() {
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
            result.push(LoaderVersionInfo {
                loader: loader_kind,
                version: loader_version.to_string(),
                label: format!("Quilt Loader {loader_version}"),
                recommended: stable || index == 0,
                game_version: game_version.to_string(),
                release_time: None,
            });
        }
    }
    sort_loader_versions(&mut result);
    Ok(result)
}

fn list_forge(
    context: &LoaderListContext<'_>,
    loader_kind: LoaderKind,
) -> Result<Vec<LoaderVersionInfo>, String> {
    let client = context.metadata_client();
    let game_version = context.game_version();
    let metadata = fetch_json(
        client,
        FORGE_MAVEN_METADATA,
        MAX_JSON_BYTES,
        "Minecraft Forge Maven",
    )?;
    let promotions = fetch_json(
        client,
        FORGE_PROMOTIONS,
        MAX_JSON_BYTES,
        "Minecraft Forge promotions",
    )
    .unwrap_or_else(|_| json!({}));
    let recommended_version = promotions
        .get("promos")
        .and_then(|promos| promos.get(format!("{game_version}-recommended")))
        .and_then(Value::as_str)
        .map(|value| value.to_string());
    let Some(raw_versions) = metadata.get(game_version).and_then(Value::as_array) else {
        return Ok(Vec::new());
    };
    let prefix = format!("{game_version}-");
    let mut result = Vec::new();
    for raw in raw_versions {
        let Some(raw) = raw.as_str() else {
            continue;
        };
        let loader_version = raw.strip_prefix(&prefix).unwrap_or(raw);
        if !component_is_safe(loader_version) {
            continue;
        }
        result.push(LoaderVersionInfo {
            loader: loader_kind,
            version: loader_version.to_string(),
            label: format!("Forge {loader_version}"),
            recommended: recommended_version.as_deref() == Some(loader_version),
            game_version: game_version.to_string(),
            release_time: None,
        });
    }
    sort_loader_versions(&mut result);
    Ok(result)
}

fn list_neoforge(
    context: &LoaderListContext<'_>,
    loader_kind: LoaderKind,
) -> Result<Vec<LoaderVersionInfo>, String> {
    let client = context.metadata_client();
    let game_version = context.game_version();
    let legacy = game_version == "1.20.1";
    let url = if legacy {
        NEOFORGE_LEGACY_MAVEN_METADATA
    } else {
        NEOFORGE_MAVEN_METADATA
    };
    let text = fetch_text(client, url, MAX_JSON_BYTES, "NeoForge Maven")?;
    let versions = parse_xml_versions(&text);
    let prefix = neoforge_game_prefix(game_version);
    let mut result = Vec::new();
    for raw in versions {
        let loader_version = if legacy {
            raw.strip_prefix(&format!("{game_version}-"))
                .unwrap_or(&raw)
        } else if raw == prefix || raw.starts_with(&format!("{prefix}.")) {
            raw.as_str()
        } else {
            continue;
        };
        if !component_is_safe(loader_version) {
            continue;
        }
        result.push(LoaderVersionInfo {
            loader: loader_kind,
            version: loader_version.to_string(),
            label: format!("NeoForge {loader_version}"),
            recommended: false,
            game_version: game_version.to_string(),
            release_time: None,
        });
    }
    sort_loader_versions(&mut result);
    if let Some(first) = result.first_mut() {
        first.recommended = true;
    }
    Ok(result)
}

fn neoforge_game_prefix(game_version: &str) -> String {
    let parts: Vec<&str> = game_version.split('.').collect();
    match parts.as_slice() {
        ["1", minor] => format!("{minor}.0"),
        ["1", minor, patch, ..] => format!("{minor}.{patch}"),
        _ => game_version.replace('.', "."),
    }
}

fn list_liteloader(
    context: &LoaderListContext<'_>,
    loader_kind: LoaderKind,
) -> Result<Vec<LoaderVersionInfo>, String> {
    let client = context.metadata_client();
    let game_version = context.game_version();
    let value = fetch_json(client, LITELOADER_METADATA, MAX_JSON_BYTES, "LiteLoader")?;
    let entries = collect_liteloader_entries(&value, game_version);
    let mut result = Vec::new();
    for entry in entries {
        if !component_is_safe(&entry.version) {
            continue;
        }
        result.push(LoaderVersionInfo {
            loader: loader_kind,
            version: entry.version.clone(),
            label: format!("LiteLoader {}", entry.version),
            recommended: entry.latest,
            game_version: game_version.to_string(),
            release_time: entry.timestamp.clone(),
        });
    }
    sort_loader_versions(&mut result);
    result.sort_by(|left, right| right.recommended.cmp(&left.recommended));
    Ok(result)
}

fn list_optifine(
    context: &LoaderListContext<'_>,
    loader_kind: LoaderKind,
) -> Result<Vec<LoaderVersionInfo>, String> {
    let client = context.metadata_client();
    let game_version = context.game_version();
    let html = fetch_text(
        client,
        OPTIFINE_DOWNLOADS,
        MAX_HTML_BYTES,
        "OptiFine downloads",
    )?;
    let filenames = extract_optifine_filenames(&html, game_version);
    let mut result = Vec::new();
    for (index, filename) in filenames.into_iter().enumerate() {
        let Some(loader_version) = optifine_loader_version(&filename, game_version) else {
            continue;
        };
        if !component_is_safe(&loader_version) {
            continue;
        }
        result.push(LoaderVersionInfo {
            loader: loader_kind,
            version: loader_version.clone(),
            label: format!("OptiFine {loader_version}"),
            recommended: index == 0,
            game_version: game_version.to_string(),
            release_time: None,
        });
    }
    Ok(result)
}

fn install_meta_profile(
    context: &LoaderProviderContext<'_>,
    service_name: &str,
    profile_url: String,
) -> Result<String, String> {
    let request = context.request();
    let root = context.root();
    let client = context.metadata_client();
    context.emit_progress(
        "profile",
        0,
        1,
        &format!("Fetching {service_name} profile"),
    );
    let profile = fetch_json(
        client,
        &profile_url,
        MAX_JSON_BYTES,
        &format!("{service_name} Meta"),
    )?;
    let installed_id = profile
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{service_name} profile does not declare an id."))?;
    version::validate_version_id(installed_id)?;
    let inherits = profile
        .get("inheritsFrom")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{service_name} profile does not declare inheritsFrom."))?;
    if inherits != request.game_version {
        return Err(format!(
            "{service_name} returned a profile for {inherits}, not {}.",
            request.game_version
        ));
    }
    context.emit_progress(
        "profile",
        1,
        1,
        &format!("{service_name} profile downloaded"),
    );

    let platform = Platform::current();
    let rule_context = platform.rule_context();
    let libraries = version::allowed_libraries(&profile, &rule_context);
    let mut tasks = Vec::new();
    for library in &libraries {
        if let Some(file) = version::library_file(library, None)? {
            tasks.push(file);
        }
        if let Some((_classifier, file)) = version::library_native_file(library, &rule_context)? {
            tasks.push(file);
        }
    }
    let total = tasks.len() as u64;
    context.emit_progress(
        "libraries",
        0,
        total,
        &format!("Downloading {service_name} libraries"),
    );
    let mut seen = HashSet::new();
    let libraries_dir = root.join("libraries");
    for (index, file) in tasks.into_iter().enumerate() {
        let key = file.path.to_ascii_lowercase();
        if !seen.insert(key) {
            continue;
        }
        let Some(url) = file.url.as_deref() else {
            return Err(format!("Library has no download URL: {}", file.path));
        };
        let Some(destination) = safe_join(&libraries_dir, &file.path) else {
            return Err(format!("Unsafe library path: {}", file.path));
        };
        net::download_atomic(
            client,
            url,
            &destination,
            file.sha1.as_deref(),
            MAX_INSTALLER_BYTES,
            &file.path,
        )?;
        context.emit_progress(
            "libraries",
            index as u64 + 1,
            total,
            &format!("Downloaded library {}", file.path),
        );
    }

    context.emit_progress(
        "finalize",
        0,
        1,
        "Writing loader version profile",
    );
    write_version_profile(root, installed_id, &profile)?;
    context.emit_progress(
        "finalize",
        1,
        1,
        "Loader version profile written",
    );
    context.emit_progress(
        "complete",
        1,
        1,
        &format!("{service_name} installation complete"),
    );
    Ok(installed_id.to_string())
}

fn install_forge(
    context: &LoaderProviderContext<'_>,
) -> Result<(String, Option<String>), String> {
    let request = context.request();
    let root = context.root();
    let loader_version = request
        .loader_version
        .strip_prefix(&format!("{}-", request.game_version))
        .unwrap_or(&request.loader_version);
    let maven_version = format!("{}-{loader_version}", request.game_version);
    let url = format!(
        "https://maven.minecraftforge.net/net/minecraftforge/forge/{maven_version}/forge-{maven_version}-installer.jar"
    );
    let file_name = format!("forge-{maven_version}-installer.jar");
    let expected_id = format!("{}-forge-{loader_version}", request.game_version);
    install_java_installer_flow(
        context,
        &url,
        &file_name,
        &["--installClient", &root.display().to_string()],
        "Forge",
        Some(&expected_id),
    )
}

fn install_neoforge(
    context: &LoaderProviderContext<'_>,
) -> Result<(String, Option<String>), String> {
    let request = context.request();
    let root = context.root();
    let legacy = request.game_version == "1.20.1";
    let loader_version = request
        .loader_version
        .strip_prefix(&format!("{}-", request.game_version))
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
    install_java_installer_flow(
        context,
        &url,
        &file_name,
        &["--installClient", &root.display().to_string()],
        "NeoForge",
        Some(&expected_id),
    )
}

fn install_optifine(
    context: &LoaderProviderContext<'_>,
) -> Result<(String, Option<String>), String> {
    let request = context.request();
    let root = context.root();
    let client = context.metadata_client();
    let html = fetch_text(
        client,
        OPTIFINE_DOWNLOADS,
        MAX_HTML_BYTES,
        "OptiFine downloads",
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

    // OptiFine currently exposes ad-gated HTTP links. We only attempt the
    // HTTPS endpoint and verify that the response is a real ZIP/JAR before
    // executing it. If upstream returns its download HTML page we fail rather
    // than passing a web page to Java or bypassing HTTPS.
    let url = format!("https://optifine.net/adloadx?f={filename}");
    let installer_name = format!("optifine-{}.jar", request.loader_version);
    let expected_id = format!(
        "{}-OptiFine_{}",
        request.game_version, request.loader_version
    );
    install_java_installer_flow(
        context,
        &url,
        &installer_name,
        &["--install"],
        "OptiFine",
        Some(&expected_id),
    )
}

fn install_java_installer_flow(
    context: &LoaderProviderContext<'_>,
    url: &str,
    file_name: &str,
    installer_args: &[&str],
    service_name: &str,
    expected_id: Option<&str>,
) -> Result<(String, Option<String>), String> {
    if !component_is_safe(file_name) {
        return Err("The installer file name is unsafe.".to_string());
    }
    let request = context.request();
    let root = context.root();
    let client = context.installer_client();
    net::validate_https_url(url)?;
    let installer_dir = root.join("loader-installers");
    net::ensure_directory(&installer_dir)?;
    let installer_path = installer_dir.join(file_name);
    context.emit_progress(
        "installer",
        0,
        1,
        &format!("Downloading {service_name} installer"),
    );
    net::download_atomic(
        client,
        url,
        &installer_path,
        None,
        MAX_INSTALLER_BYTES,
        &format!("{service_name} installer"),
    )?;
    if !is_zip_file(&installer_path)? {
        let _ = fs::remove_file(&installer_path);
        return Err(format!(
            "{service_name} upstream did not return a direct JAR. Download and install it manually, then retry instance discovery."
        ));
    }
    ensure_launcher_profiles(root)?;
    let log_path = loader_log_path(
        root,
        request.loader,
        &request.game_version,
        &request.loader_version,
    )?;
    context.emit_progress(
        "installer",
        1,
        2,
        &format!("Running {service_name} installer"),
    );
    run_java_installer(
        &request.java_path,
        &installer_path,
        installer_args,
        root,
        &log_path,
        INSTALLER_TIMEOUT_SECONDS,
    )?;
    context.emit_progress(
        "installer",
        2,
        2,
        &format!("{service_name} installer finished"),
    );

    let installed_id =
        find_installed_profile(root, &request.game_version, request.loader, expected_id)
            .ok_or_else(|| {
                format!(
            "{service_name} installer finished, but no matching version profile was found. See {}.",
            log_path.display()
        )
            })?;
    Ok((installed_id, Some(log_path.display().to_string())))
}

fn install_liteloader(
    context: &LoaderProviderContext<'_>,
) -> Result<String, String> {
    let request = context.request();
    let root = context.root();
    let client = context.metadata_client();
    let metadata = fetch_json(client, LITELOADER_METADATA, MAX_JSON_BYTES, "LiteLoader")?;
    let entries = collect_liteloader_entries(&metadata, &request.game_version);
    let Some(entry) = entries.into_iter().find(|entry| {
        entry.version.eq_ignore_ascii_case(&request.loader_version)
            || entry
                .build
                .as_deref()
                .map(|build| build.eq_ignore_ascii_case(&request.loader_version))
                .unwrap_or(false)
    }) else {
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
    let installer_url =
        resolve_liteloader_jar_url(client, game_node, &entry, &request.game_version)?;
    let libraries_dir = root.join("libraries");
    let relative_jar = format!(
        "com/mumfrey/liteloader/{0}/liteloader-{0}.jar",
        request.loader_version
    );
    let Some(jar_path) = safe_join(&libraries_dir, &relative_jar) else {
        return Err("Unsafe LiteLoader artifact path.".to_string());
    };
    context.emit_progress("libraries", 0, 1, "Downloading LiteLoader");
    net::download_atomic(
        client,
        &installer_url,
        &jar_path,
        None,
        MAX_INSTALLER_BYTES,
        "LiteLoader",
    )?;
    if !is_zip_file(&jar_path)? {
        let _ = fs::remove_file(&jar_path);
        return Err("LiteLoader upstream returned a non-JAR file.".to_string());
    }
    context.emit_progress("libraries", 1, 1, "LiteLoader downloaded");

    let mut profile_libraries = Vec::new();
    profile_libraries.push(json!({
        "name": format!("com.mumfrey:liteloader:{}", request.loader_version),
        "downloads": {
            "artifact": {
                "path": relative_jar,
                "url": installer_url
            }
        }
    }));
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
    for (index, mut library) in additional_libraries.iter().cloned().enumerate() {
        upgrade_library_url(&mut library);
        if let Some(file) = version::library_file(&library, None)? {
            let Some(url) = file.url.as_deref() else {
                return Err(format!("LiteLoader dependency has no URL: {}", file.path));
            };
            let Some(destination) = safe_join(&libraries_dir, &file.path) else {
                return Err(format!("Unsafe LiteLoader dependency path: {}", file.path));
            };
            net::download_atomic(
                client,
                url,
                &destination,
                file.sha1.as_deref(),
                MAX_INSTALLER_BYTES,
                &file.path,
            )?;
        }
        profile_libraries.push(library);
        context.emit_progress(
            "libraries",
            index as u64 + 1,
            total,
            &format!("Downloaded LiteLoader dependency {}", index + 1),
        );
    }

    let tweak_class = entry
        .tweak_class
        .clone()
        .unwrap_or_else(|| "com.mumfrey.liteloader.launch.LiteLoaderTweaker".to_string());
    let parent =
        version::load_version_json(&version::version_json_path(root, &request.game_version))?;
    let installed_id = format!(
        "{}-liteloader-{}",
        request.game_version, request.loader_version
    );
    version::validate_version_id(&installed_id)?;
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
    write_version_profile(root, &installed_id, &profile)?;
    context.emit_progress(
        "complete",
        1,
        1,
        "LiteLoader installation complete",
    );
    Ok(installed_id)
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
            .map(|value| value.to_string());
        let display_version = build.clone().unwrap_or_else(|| version.to_string());
        result.push(LiteEntry {
            version: display_version,
            metadata_version: version.to_string(),
            build,
            file: object
                .get("file")
                .and_then(Value::as_str)
                .map(|value| value.to_string()),
            latest,
            timestamp: object
                .get("timestamp")
                .and_then(Value::as_str)
                .map(|value| value.to_string()),
            tweak_class: object
                .get("tweakClass")
                .and_then(Value::as_str)
                .map(|value| value.to_string()),
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
    client: &Client,
    game_node: &Value,
    entry: &LiteEntry,
    game_version: &str,
) -> Result<String, String> {
    let repo = game_node
        .get("repo")
        .and_then(|repo| repo.get("url"))
        .and_then(Value::as_str)
        .unwrap_or("http://repo.mumfrey.com/content/repositories/snapshots/");
    let repo = force_https(repo);
    if entry.metadata_version.contains("SNAPSHOT") {
        let directory = format!(
            "{}/com/mumfrey/liteloader/{}/",
            repo.trim_end_matches('/'),
            entry.metadata_version
        );
        let listing = fetch_text(client, &directory, MAX_HTML_BYTES, "LiteLoader repository")?;
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

fn run_java_installer(
    java_path: &str,
    installer: &Path,
    installer_args: &[&str],
    working_directory: &Path,
    log_path: &Path,
    timeout_seconds: u64,
) -> Result<(), String> {
    let java = java::resolve_java_path(java_path)?;
    if let Some(parent) = log_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "Cannot create installer log directory ({}): {error}",
                parent.display()
            )
        })?;
    }
    let stdout = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)
        .map_err(|error| {
            format!(
                "Cannot open installer log ({}): {error}",
                log_path.display()
            )
        })?;
    let stderr = stdout
        .try_clone()
        .map_err(|error| format!("Cannot duplicate installer log handle: {error}"))?;
    let mut command = Command::new(&java);
    command
        .arg("-jar")
        .arg(installer)
        .args(installer_args)
        .current_dir(working_directory)
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr));
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let mut child = command.spawn().map_err(|error| {
        format!(
            "Cannot start the loader installer with {}: {error}",
            java.display()
        )
    })?;
    let deadline = Instant::now() + Duration::from_secs(timeout_seconds);
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => return Ok(()),
            Ok(Some(status)) => {
                return Err(format!(
                    "Loader installer exited with {status}. See {}.",
                    log_path.display()
                ));
            }
            Ok(None) => {}
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("Cannot monitor the loader installer: {error}"));
            }
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!(
                "Loader installer timed out after {timeout_seconds} seconds. See {}.",
                log_path.display()
            ));
        }
        thread::sleep(Duration::from_millis(250));
    }
}

fn is_zip_file(path: &Path) -> Result<bool, String> {
    let mut file = fs::File::open(path)
        .map_err(|error| format!("Cannot open installer ({}): {error}", path.display()))?;
    let mut signature = [0u8; 4];
    let count = file
        .read(&mut signature)
        .map_err(|error| format!("Cannot read installer ({}): {error}", path.display()))?;
    Ok(count == 4 && matches!(&signature, b"PK\x03\x04" | b"PK\x05\x06" | b"PK\x07\x08"))
}

fn ensure_launcher_profiles(root: &Path) -> Result<(), String> {
    let path = root.join("launcher_profiles.json");
    if path.is_file() {
        return Ok(());
    }
    let value = json!({
        "profiles": {},
        "settings": {},
        "version": 3
    });
    fs::write(
        &path,
        serde_json::to_vec_pretty(&value)
            .map_err(|error| format!("Cannot serialize launcher_profiles.json: {error}"))?,
    )
    .map_err(|error| format!("Cannot create launcher_profiles.json: {error}"))
}

fn find_installed_profile(
    root: &Path,
    game_version: &str,
    loader: LoaderKind,
    expected_id: Option<&str>,
) -> Option<String> {
    if let Some(expected) = expected_id {
        if version::version_json_path(root, expected).is_file() {
            return Some(expected.to_string());
        }
    }
    let game_dir = root.display().to_string();
    let instances = version::list_instances(&game_dir).ok()?;
    let expected_loader = providers::loader_display_name(loader);
    let mut candidates: Vec<(std::time::SystemTime, String)> = instances
        .into_iter()
        .filter(|instance| {
            instance.game_version.eq_ignore_ascii_case(game_version)
                && instance.loader.eq_ignore_ascii_case(expected_loader)
        })
        .filter_map(|instance| {
            let path = version::version_json_path(root, &instance.id);
            let modified = fs::metadata(path).ok()?.modified().ok()?;
            Some((modified, instance.id))
        })
        .collect();
    candidates.sort_by(|left, right| right.0.cmp(&left.0));
    candidates.into_iter().next().map(|(_, id)| id)
}


fn loader_log_path(
    root: &Path,
    loader: LoaderKind,
    game_version: &str,
    loader_version: &str,
) -> Result<PathBuf, String> {
    let logs = root.join("logs");
    fs::create_dir_all(&logs).map_err(|error| {
        format!(
            "Cannot create loader log directory ({}): {error}",
            logs.display()
        )
    })?;
    Ok(logs.join(format!(
        "flint-loader-{}-{}-{}-{}.log",
        loader.as_str(),
        sanitize_file_component(game_version),
        sanitize_file_component(loader_version),
        net::nonce()
    )))
}

fn sanitize_file_component(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.') {
                character
            } else {
                '_'
            }
        })
        .take(96)
        .collect()
}

fn write_version_profile(root: &Path, version_id: &str, profile: &Value) -> Result<(), String> {
    version::validate_version_id(version_id)?;
    let path = version::version_json_path(root, version_id);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "Cannot create version directory ({}): {error}",
                parent.display()
            )
        })?;
    }
    let bytes = serde_json::to_vec_pretty(profile)
        .map_err(|error| format!("Cannot serialize loader profile: {error}"))?;
    let temp = net::temp_path_near(&path, "tmp");
    fs::write(&temp, bytes)
        .map_err(|error| format!("Cannot write temporary loader profile: {error}"))?;
    match fs::rename(&temp, &path) {
        Ok(()) => Ok(()),
        Err(first_error) => {
            let _ = fs::remove_file(&path);
            fs::rename(&temp, &path).map_err(|error| {
                let _ = fs::remove_file(&temp);
                format!("Cannot install loader profile ({first_error}; {error})")
            })
        }
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

fn upgrade_library_url(library: &mut Value) {
    if let Some(object) = library.as_object_mut() {
        if object.get("url").is_none() {
            object.insert(
                "url".to_string(),
                json!("https://repo.maven.apache.org/maven2/"),
            );
        } else if let Some(url) = object.get("url").and_then(Value::as_str) {
            if let Some(https) = url.strip_prefix("http://") {
                object.insert("url".to_string(), json!(format!("https://{https}")));
            }
        }
    }
}

fn force_https(url: &str) -> String {
    url.strip_prefix("http://")
        .map(|rest| format!("https://{rest}"))
        .unwrap_or_else(|| url.to_string())
}

fn sort_loader_versions(versions: &mut [LoaderVersionInfo]) {
    versions.sort_by(|left, right| compare_versions_desc(&left.version, &right.version));
}

fn compare_versions_desc(left: &str, right: &str) -> std::cmp::Ordering {
    let left_parts: Vec<&str> = left
        .split(|character: char| !character.is_ascii_alphanumeric())
        .collect();
    let right_parts: Vec<&str> = right
        .split(|character: char| !character.is_ascii_alphanumeric())
        .collect();
    for (left_part, right_part) in left_parts.iter().zip(right_parts.iter()) {
        let left_number = left_part.parse::<u64>().ok();
        let right_number = right_part.parse::<u64>().ok();
        let ordering = match (left_number, right_number) {
            (Some(left), Some(right)) => right.cmp(&left),
            _ => right.to_ascii_lowercase().cmp(&left.to_ascii_lowercase()),
        };
        if ordering != std::cmp::Ordering::Equal {
            return ordering;
        }
    }
    right_parts.len().cmp(&left_parts.len())
}

fn parse_xml_versions(text: &str) -> Vec<String> {
    let Ok(regex) = Regex::new(r"(?s)<version>\s*([^<]+?)\s*</version>") else {
        return Vec::new();
    };
    let mut seen = HashSet::new();
    regex
        .captures_iter(text)
        .filter_map(|capture| {
            capture
                .get(1)
                .map(|value| value.as_str().trim().to_string())
        })
        .filter(|value| !value.is_empty() && seen.insert(value.clone()))
        .collect()
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

fn fetch_json(client: &Client, url: &str, max_bytes: u64, service: &str) -> Result<Value, String> {
    let bytes = fetch_bytes(client, url, max_bytes, service)?;
    serde_json::from_slice(&bytes)
        .map_err(|error| format!("{service} returned invalid JSON: {error}"))
}

fn fetch_text(client: &Client, url: &str, max_bytes: u64, service: &str) -> Result<String, String> {
    let bytes = fetch_bytes(client, url, max_bytes, service)?;
    Ok(String::from_utf8_lossy(&bytes).to_string())
}

fn fetch_bytes(
    client: &Client,
    url: &str,
    max_bytes: u64,
    service: &str,
) -> Result<Vec<u8>, String> {
    net::validate_https_url(url)?;
    let response = client
        .get(url)
        .send()
        .map_err(|error| format!("Cannot contact {service}: {error}"))?;
    let status = response.status();
    let bytes = net::read_body_limited(response, max_bytes)?;
    if !status.is_success() {
        return Err(format!(
            "{service} request failed with HTTP {}. Check connectivity or try another version.",
            status.as_u16()
        ));
    }
    Ok(bytes)
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
        "flint://loader-install-progress",
        LoaderInstallProgress {
            loader: request.loader,
            game_version: request.game_version.clone(),
            loader_version: request.loader_version.clone(),
            stage: stage.to_string(),
            completed,
            total,
            message: message.to_string(),
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_fabric_loader_list() {
        let value = json!([
            {"loader": {"version": "0.15.11", "stable": true}},
            {"loader": {"version": "0.16.0", "stable": false}}
        ]);
        assert_eq!(value.as_array().expect("list").len(), 2);
    }

    #[test]
    fn parses_neoforge_xml_versions() {
        let xml = r#"<metadata><versioning><versions>
            <version>21.1.80</version><version>21.1.81</version>
        </versions></versioning></metadata>"#;
        assert_eq!(parse_xml_versions(xml), vec!["21.1.80", "21.1.81"]);
    }

    #[test]
    fn maps_neoforge_game_versions_to_maven_prefixes() {
        assert_eq!(neoforge_game_prefix("1.21.1"), "21.1");
        assert_eq!(neoforge_game_prefix("1.20.2"), "20.2");
        assert_eq!(neoforge_game_prefix("1.21"), "21.0");
    }

    #[test]
    fn extracts_optifine_filenames_for_one_game_version() {
        let html = r#"<a href="x">OptiFine_1.20.1_HD_U_I6.jar</a>
            <a href="y">OptiFine_1.20.1_HD_U_I5.jar</a>
            <a href="z">OptiFine_1.20.2_HD_U_I7.jar</a>"#;
        let files = extract_optifine_filenames(html, "1.20.1");
        assert_eq!(files.len(), 2);
        assert_eq!(
            optifine_loader_version(&files[0], "1.20.1").as_deref(),
            Some("HD_U_I6")
        );
    }

    #[test]
    fn parses_liteloader_latest_metadata() {
        let value = json!({
            "versions": {
                "1.12.2": {
                    "snapshots": {
                        "com.mumfrey:liteloader": {
                            "latest": {
                                "version": "1.12.2-SNAPSHOT",
                                "build": "1.12.2-SNAPSHOT-r4",
                                "tweakClass": "com.mumfrey.liteloader.launch.LiteLoaderTweaker",
                                "libraries": []
                            }
                        }
                    }
                }
            }
        });
        let entries = collect_liteloader_entries(&value, "1.12.2");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].version, "1.12.2-SNAPSHOT-r4");
        assert!(entries[0].latest);
    }

    #[test]
    fn loader_versions_sort_newest_first() {
        let mut versions = vec![
            LoaderVersionInfo {
                loader: LoaderKind::Forge,
                version: "47.2.0".to_string(),
                label: String::new(),
                recommended: false,
                game_version: "1.20.1".to_string(),
                release_time: None,
            },
            LoaderVersionInfo {
                loader: LoaderKind::Forge,
                version: "47.3.0".to_string(),
                label: String::new(),
                recommended: false,
                game_version: "1.20.1".to_string(),
                release_time: None,
            },
        ];
        sort_loader_versions(&mut versions);
        assert_eq!(versions[0].version, "47.3.0");
    }

    #[test]
    fn validates_zip_header_signature() {
        let path = std::env::temp_dir().join(format!("flint-zip-test-{}.jar", net::nonce()));
        fs::write(&path, b"PK\x03\x04test").expect("write test zip header");
        assert!(is_zip_file(&path).expect("check zip"));
        fs::write(&path, b"<html></html>").expect("write test html");
        assert!(!is_zip_file(&path).expect("check html"));
        let _ = fs::remove_file(path);
    }
}
