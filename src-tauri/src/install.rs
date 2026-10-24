use crate::{
    models::{InstallProgress, InstallResult, RemoteVersion},
    paths::{
        component_is_safe, is_sha1_hex, path_dedup_key, resolve_game_dir, safe_join, sha1_file,
    },
    version::{self, Platform},
};
use reqwest::blocking::Client;
use serde::Deserialize;
use serde_json::Value;
use sha1::{Digest, Sha1};
use std::{
    collections::HashMap,
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    process, thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Emitter};

const MANIFEST_URL: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
const RESOURCE_BASE_URL: &str = "https://resources.download.minecraft.net";
const DOWNLOAD_ATTEMPTS: usize = 3;

#[derive(Debug, Deserialize)]
struct VersionManifest {
    versions: Vec<ManifestVersion>,
}

#[derive(Debug, Clone, Deserialize)]
struct ManifestVersion {
    id: String,
    #[serde(rename = "type")]
    version_type: String,
    url: String,
    #[serde(default)]
    sha1: Option<String>,
    #[serde(default)]
    size: Option<u64>,
    #[serde(default, rename = "releaseTime")]
    release_time: String,
}

#[derive(Debug, Clone, Deserialize)]
struct AssetObject {
    hash: String,
    size: u64,
}

#[derive(Debug, Default, Deserialize)]
struct AssetIndexDocument {
    #[serde(default)]
    objects: HashMap<String, AssetObject>,
    #[serde(default, rename = "virtual")]
    virtual_assets: bool,
    #[serde(default)]
    map_to_resources: bool,
}

#[derive(Debug, Clone)]
struct DownloadTask {
    url: String,
    destination: PathBuf,
    sha1: Option<String>,
    size: Option<u64>,
    label: String,
}

#[derive(Debug, Clone, Copy, Default)]
struct DownloadOutcome {
    bytes: u64,
    wrote: bool,
}

#[derive(Debug, Default)]
struct InstallStats {
    files_written: u64,
    bytes_downloaded: u64,
}

pub fn fetch_remote_versions_blocking() -> Result<Vec<RemoteVersion>, String> {
    let client = build_client()?;
    let (manifest, _) = fetch_manifest(&client)?;
    Ok(manifest
        .versions
        .into_iter()
        .map(|entry| RemoteVersion {
            id: entry.id,
            version_type: entry.version_type,
            release_time: entry.release_time,
        })
        .collect())
}

pub fn install_blocking(
    app: &AppHandle,
    game_dir: &str,
    version_id: &str,
) -> Result<InstallResult, String> {
    version::validate_version_id(version_id)?;
    let root = resolve_game_dir(game_dir);
    let version_path = version::version_dir(&root, version_id);
    fs::create_dir_all(&version_path).map_err(|error| {
        format!(
            "Cannot create version directory ({}): {error}",
            version_path.display()
        )
    })?;

    emit_progress(app, "metadata", 0, 1, "Fetching Mojang version manifest");
    let client = build_client()?;
    let (manifest, manifest_bytes) = fetch_manifest(&client)?;
    let mut stats = InstallStats {
        bytes_downloaded: manifest_bytes,
        files_written: 0,
    };
    let entry = manifest
        .versions
        .into_iter()
        .find(|entry| entry.id == version_id)
        .ok_or_else(|| format!("Minecraft version not found: {version_id}"))?;

    let metadata_destination = version::version_json_path(&root, version_id);
    let metadata_task = DownloadTask {
        url: entry.url,
        destination: metadata_destination.clone(),
        sha1: entry.sha1,
        size: entry.size,
        label: format!("{version_id} version metadata"),
    };
    let metadata_outcome = download_verified(&client, &metadata_task)?;
    stats.bytes_downloaded += metadata_outcome.bytes;
    if metadata_outcome.wrote {
        stats.files_written += 1;
    }
    emit_progress(app, "metadata", 1, 1, "Version metadata downloaded");

    let metadata = version::load_version_json(&metadata_destination)?;
    if let Some(metadata_id) = metadata.get("id").and_then(Value::as_str) {
        if metadata_id != version_id {
            return Err(format!(
                "Downloaded metadata id mismatch: expected {version_id}, received {metadata_id}."
            ));
        }
    }

    let client_download = metadata
        .get("downloads")
        .and_then(|value| value.get("client"))
        .ok_or_else(|| format!("Version {version_id} does not declare a client jar."))?;
    let client_url = client_download
        .get("url")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("Version {version_id} client jar URL is missing."))?;
    let client_task = DownloadTask {
        url: client_url.to_string(),
        destination: version::client_jar_path(&root, version_id),
        sha1: client_download
            .get("sha1")
            .and_then(Value::as_str)
            .map(|value| value.to_ascii_lowercase()),
        size: client_download.get("size").and_then(Value::as_u64),
        label: format!("{version_id} client jar"),
    };
    validate_https_url(&client_task.url)?;

    let platform = Platform::current();
    let rule_context = platform.rule_context();
    let libraries = build_library_tasks(&root, &metadata, &rule_context)?;
    let logging_task = build_logging_task(&root, &metadata)?;
    let (asset_index_id, asset_index_task) = build_asset_index_task(&root, &metadata)?;

    run_tasks(
        app,
        &client,
        "client",
        std::slice::from_ref(&client_task),
        &mut stats,
    )?;
    run_tasks(app, &client, "libraries", &libraries, &mut stats)?;
    if let Some(task) = asset_index_task.as_ref() {
        run_tasks(
            app,
            &client,
            "asset-index",
            std::slice::from_ref(task),
            &mut stats,
        )?;
    }

    let mut asset_document = None;
    let mut asset_tasks = Vec::new();
    if let Some(index_id) = asset_index_id.as_deref() {
        let index_path = root
            .join("assets")
            .join("indexes")
            .join(format!("{index_id}.json"));
        if index_path.is_file() {
            let document = load_asset_index(&index_path)?;
            asset_tasks = build_asset_object_tasks(&root, &document)?;
            asset_document = Some(document);
        }
    }
    run_tasks(app, &client, "assets", &asset_tasks, &mut stats)?;

    if let (Some(index_id), Some(document)) = (asset_index_id.as_deref(), asset_document.as_ref()) {
        materialize_legacy_assets(&root, index_id, document)?;
    }

    if let Some(task) = logging_task.as_ref() {
        run_tasks(
            app,
            &client,
            "logging",
            std::slice::from_ref(task),
            &mut stats,
        )?;
    }

    let completed_total = 1
        + 1
        + libraries.len() as u64
        + if asset_index_task.is_some() { 1 } else { 0 }
        + asset_tasks.len() as u64
        + if logging_task.is_some() { 1 } else { 0 };
    emit_progress(
        app,
        "complete",
        completed_total,
        completed_total,
        "Installation complete",
    );

    Ok(InstallResult {
        version_id: version_id.to_string(),
        files_written: stats.files_written,
        bytes_downloaded: stats.bytes_downloaded,
        install_path: version_path.display().to_string(),
    })
}

fn build_client() -> Result<Client, String> {
    Client::builder()
        .user_agent(concat!("Flint/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(20))
        .timeout(Duration::from_secs(120))
        .https_only(true)
        .build()
        .map_err(|error| format!("Cannot initialize HTTP client: {error}"))
}

fn fetch_manifest(client: &Client) -> Result<(VersionManifest, u64), String> {
    let response = client
        .get(MANIFEST_URL)
        .send()
        .and_then(|response| response.error_for_status())
        .map_err(|error| format!("Cannot fetch Mojang version manifest: {error}"))?;
    let bytes = response
        .bytes()
        .map_err(|error| format!("Cannot read Mojang version manifest: {error}"))?;
    let manifest = serde_json::from_slice::<VersionManifest>(&bytes)
        .map_err(|error| format!("Mojang version manifest is invalid: {error}"))?;
    Ok((manifest, bytes.len() as u64))
}

fn build_library_tasks(
    root: &Path,
    metadata: &Value,
    rule_context: &version::RuleContext<'_>,
) -> Result<Vec<DownloadTask>, String> {
    let libraries_root = root.join("libraries");
    let mut tasks = Vec::new();
    for library in version::allowed_libraries(metadata, rule_context) {
        if let Some(file) = version::library_file(&library, None)? {
            let task = task_from_library_file(&libraries_root, file, "library")?;
            push_unique_task(&mut tasks, task)?;
        }
        if let Some((classifier, file)) = version::library_native_file(&library, rule_context)? {
            let task = task_from_library_file(
                &libraries_root,
                file,
                &format!("native library ({classifier})"),
            )?;
            push_unique_task(&mut tasks, task)?;
        }
    }
    Ok(tasks)
}

fn task_from_library_file(
    libraries_root: &Path,
    file: version::LibraryFile,
    label: &str,
) -> Result<DownloadTask, String> {
    let url = file
        .url
        .ok_or_else(|| format!("No download URL for {label}: {}", file.path))?;
    validate_https_url(&url)?;
    if let Some(sha1) = file.sha1.as_deref() {
        if !is_sha1_hex(sha1) {
            return Err(format!("Invalid SHA-1 in library metadata: {sha1}"));
        }
    }
    let destination = safe_join(libraries_root, &file.path)
        .ok_or_else(|| format!("Unsafe library path: {}", file.path))?;
    Ok(DownloadTask {
        url,
        destination,
        sha1: file.sha1,
        size: file.size,
        label: label.to_string(),
    })
}

fn build_logging_task(root: &Path, metadata: &Value) -> Result<Option<DownloadTask>, String> {
    let Some(logging_file) = metadata
        .get("logging")
        .and_then(|value| value.get("client"))
        .and_then(|value| value.get("file"))
    else {
        return Ok(None);
    };
    let id = logging_file
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(|| "Logging configuration file id is missing.".to_string())?;
    if !component_is_safe(id) {
        return Err("Logging configuration file id is unsafe.".to_string());
    }
    let url = logging_file
        .get("url")
        .and_then(Value::as_str)
        .ok_or_else(|| "Logging configuration download URL is missing.".to_string())?;
    validate_https_url(url)?;
    let sha1 = logging_file
        .get("sha1")
        .and_then(Value::as_str)
        .map(|value| value.to_ascii_lowercase());
    if let Some(sha1) = sha1.as_deref() {
        if !is_sha1_hex(sha1) {
            return Err("Logging configuration SHA-1 is invalid.".to_string());
        }
    }
    Ok(Some(DownloadTask {
        url: url.to_string(),
        destination: root.join("assets").join("log_configs").join(id),
        sha1,
        size: logging_file.get("size").and_then(Value::as_u64),
        label: "logging configuration".to_string(),
    }))
}

fn build_asset_index_task(
    root: &Path,
    metadata: &Value,
) -> Result<(Option<String>, Option<DownloadTask>), String> {
    let Some(asset_index) = metadata.get("assetIndex") else {
        return Ok((None, None));
    };
    let id = asset_index
        .get("id")
        .and_then(Value::as_str)
        .or_else(|| metadata.get("assets").and_then(Value::as_str))
        .ok_or_else(|| "Asset index id is missing.".to_string())?;
    if !component_is_safe(id) {
        return Err("Asset index id is unsafe.".to_string());
    }
    let url = asset_index
        .get("url")
        .and_then(Value::as_str)
        .ok_or_else(|| "Asset index download URL is missing.".to_string())?;
    validate_https_url(url)?;
    let sha1 = asset_index
        .get("sha1")
        .and_then(Value::as_str)
        .map(|value| value.to_ascii_lowercase());
    if let Some(sha1) = sha1.as_deref() {
        if !is_sha1_hex(sha1) {
            return Err("Asset index SHA-1 is invalid.".to_string());
        }
    }
    let task = DownloadTask {
        url: url.to_string(),
        destination: root
            .join("assets")
            .join("indexes")
            .join(format!("{id}.json")),
        sha1,
        size: asset_index.get("size").and_then(Value::as_u64),
        label: format!("asset index {id}"),
    };
    Ok((Some(id.to_string()), Some(task)))
}

fn load_asset_index(path: &Path) -> Result<AssetIndexDocument, String> {
    let metadata = fs::metadata(path)
        .map_err(|error| format!("Cannot read asset index ({}): {error}", path.display()))?;
    if metadata.len() > 128 * 1024 * 1024 {
        return Err("Asset index is unexpectedly large.".to_string());
    }
    let text = fs::read_to_string(path)
        .map_err(|error| format!("Cannot read asset index ({}): {error}", path.display()))?;
    serde_json::from_str(&text)
        .map_err(|error| format!("Asset index is invalid JSON ({}): {error}", path.display()))
}

fn build_asset_object_tasks(
    root: &Path,
    document: &AssetIndexDocument,
) -> Result<Vec<DownloadTask>, String> {
    let objects_root = root.join("assets").join("objects");
    let mut objects: Vec<&AssetObject> = document.objects.values().collect();
    objects.sort_by(|left, right| left.hash.cmp(&right.hash));

    let mut tasks = Vec::new();
    let mut seen: HashMap<String, u64> = HashMap::new();
    for object in objects {
        let hash = object.hash.to_ascii_lowercase();
        if !is_sha1_hex(&hash) {
            return Err(format!("Asset object has an invalid SHA-1: {hash}"));
        }
        if let Some(existing_size) = seen.get(&hash) {
            if *existing_size != object.size {
                return Err(format!("Asset object has conflicting sizes: {hash}"));
            }
            continue;
        }
        seen.insert(hash.clone(), object.size);

        let relative = format!("{}/{}", &hash[..2], hash);
        let destination = safe_join(&objects_root, &relative)
            .ok_or_else(|| "Asset object path is unsafe.".to_string())?;
        tasks.push(DownloadTask {
            url: format!("{RESOURCE_BASE_URL}/{relative}"),
            destination,
            sha1: Some(hash),
            size: Some(object.size),
            label: "asset object".to_string(),
        });
    }
    Ok(tasks)
}

fn materialize_legacy_assets(
    root: &Path,
    index_id: &str,
    document: &AssetIndexDocument,
) -> Result<(), String> {
    if !document.virtual_assets && !document.map_to_resources {
        return Ok(());
    }
    let mut targets = Vec::new();
    if document.virtual_assets {
        targets.push(root.join("assets").join("virtual").join(index_id));
    }
    if document.map_to_resources {
        targets.push(root.join("resources"));
    }
    for (name, object) in &document.objects {
        let Some(relative) = crate::paths::normalize_relative_metadata_path(name) else {
            continue;
        };
        let hash = object.hash.to_ascii_lowercase();
        if !is_sha1_hex(&hash) {
            continue;
        }
        let source = root
            .join("assets")
            .join("objects")
            .join(&hash[..2])
            .join(&hash);
        if !source.is_file() {
            return Err(format!("Downloaded asset object is missing: {hash}"));
        }
        for target_root in &targets {
            let Some(destination) = safe_join(target_root, &relative) else {
                continue;
            };
            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent).map_err(|error| {
                    format!(
                        "Cannot create legacy asset directory ({}): {error}",
                        parent.display()
                    )
                })?;
            }
            fs::copy(&source, &destination).map_err(|error| {
                format!(
                    "Cannot materialize legacy asset ({}): {error}",
                    destination.display()
                )
            })?;
        }
    }
    Ok(())
}

fn run_tasks(
    app: &AppHandle,
    client: &Client,
    stage: &str,
    tasks: &[DownloadTask],
    stats: &mut InstallStats,
) -> Result<(), String> {
    let total = tasks.len() as u64;
    emit_progress(app, stage, 0, total, &format!("Starting {stage} downloads"));
    for (index, task) in tasks.iter().enumerate() {
        let outcome = download_verified(client, task)?;
        stats.bytes_downloaded += outcome.bytes;
        if outcome.wrote {
            stats.files_written += 1;
        }
        let completed = index as u64 + 1;
        if completed == total || total <= 20 || completed % 20 == 0 {
            emit_progress(
                app,
                stage,
                completed,
                total,
                &format!("Downloaded {} ({completed}/{total})", task.label),
            );
        }
    }
    Ok(())
}

fn download_verified(client: &Client, task: &DownloadTask) -> Result<DownloadOutcome, String> {
    validate_https_url(&task.url)?;
    if let Some(expected) = task.sha1.as_deref() {
        if !is_sha1_hex(expected) {
            return Err(format!("Invalid expected SHA-1 for {}", task.label));
        }
        if task.destination.is_file()
            && task
                .size
                .map(|size| {
                    fs::metadata(&task.destination)
                        .map(|value| value.len() == size)
                        .unwrap_or(false)
                })
                .unwrap_or(true)
            && sha1_file(&task.destination)
                .map(|actual| actual.eq_ignore_ascii_case(expected))
                .unwrap_or(false)
        {
            return Ok(DownloadOutcome::default());
        }
    }

    let mut last_error = String::new();
    for attempt in 1..=DOWNLOAD_ATTEMPTS {
        match download_once(client, task) {
            Ok(outcome) => return Ok(outcome),
            Err(error) => {
                last_error = error;
                if attempt < DOWNLOAD_ATTEMPTS {
                    thread::sleep(Duration::from_millis(400 * attempt as u64));
                }
            }
        }
    }
    Err(format!(
        "Failed to download {} after {DOWNLOAD_ATTEMPTS} attempts: {last_error}",
        task.label
    ))
}

fn download_once(client: &Client, task: &DownloadTask) -> Result<DownloadOutcome, String> {
    if let Some(parent) = task.destination.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "Cannot create download directory ({}): {error}",
                parent.display()
            )
        })?;
    }
    let file_name = task
        .destination
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("download.bin");
    let part =
        task.destination
            .with_file_name(format!(".{file_name}.{}.{}.part", process::id(), nonce()));

    let result = (|| -> Result<DownloadOutcome, String> {
        let mut response = client
            .get(&task.url)
            .send()
            .and_then(|response| response.error_for_status())
            .map_err(|error| format!("HTTP request failed: {error}"))?;
        let mut file = fs::File::create(&part)
            .map_err(|error| format!("Cannot create partial download file: {error}"))?;
        let mut hasher = Sha1::new();
        let mut bytes = 0u64;
        let mut buffer = [0u8; 64 * 1024];
        loop {
            let count = response
                .read(&mut buffer)
                .map_err(|error| format!("Cannot read HTTP response: {error}"))?;
            if count == 0 {
                break;
            }
            file.write_all(&buffer[..count])
                .map_err(|error| format!("Cannot write partial download file: {error}"))?;
            hasher.update(&buffer[..count]);
            bytes += count as u64;
        }
        file.flush()
            .map_err(|error| format!("Cannot flush partial download file: {error}"))?;
        file.sync_all()
            .map_err(|error| format!("Cannot sync partial download file: {error}"))?;

        if let Some(expected_size) = task.size {
            if bytes != expected_size {
                return Err(format!(
                    "Size mismatch: expected {expected_size} bytes, received {bytes}"
                ));
            }
        }
        if let Some(expected_sha1) = task.sha1.as_deref() {
            let actual = format!("{:x}", hasher.finalize());
            if !actual.eq_ignore_ascii_case(expected_sha1) {
                return Err(format!(
                    "SHA-1 mismatch: expected {expected_sha1}, received {actual}"
                ));
            }
        }

        commit_part(&part, &task.destination)?;
        Ok(DownloadOutcome { bytes, wrote: true })
    })();

    if result.is_err() {
        let _ = fs::remove_file(&part);
    }
    result
}

fn commit_part(part: &Path, destination: &Path) -> Result<(), String> {
    match fs::rename(part, destination) {
        Ok(()) => Ok(()),
        Err(first_error) => {
            if !destination.exists() {
                return Err(first_error.to_string());
            }
            let file_name = destination
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("download.bin");
            let backup = destination.with_file_name(format!(
                ".{file_name}.{}.{}.bak",
                process::id(),
                nonce()
            ));
            fs::rename(destination, &backup)
                .map_err(|error| format!("Cannot move old downloaded file aside: {error}"))?;
            match fs::rename(part, destination) {
                Ok(()) => {
                    let _ = fs::remove_file(&backup);
                    Ok(())
                }
                Err(error) => {
                    let _ = fs::rename(&backup, destination);
                    Err(error.to_string())
                }
            }
        }
    }
}

fn push_unique_task(tasks: &mut Vec<DownloadTask>, task: DownloadTask) -> Result<(), String> {
    let key = path_dedup_key(&task.destination);
    if let Some(existing) = tasks
        .iter()
        .find(|existing| path_dedup_key(&existing.destination) == key)
    {
        let same_hash = match (existing.sha1.as_deref(), task.sha1.as_deref()) {
            (Some(left), Some(right)) => left.eq_ignore_ascii_case(right),
            _ => false,
        };
        if same_hash || existing.url == task.url {
            return Ok(());
        }
        return Err(format!(
            "Conflicting downloads target the same file: {}",
            task.destination.display()
        ));
    }
    tasks.push(task);
    Ok(())
}

fn validate_https_url(url: &str) -> Result<(), String> {
    if url.starts_with("https://") {
        Ok(())
    } else {
        Err("Refusing to download from a non-HTTPS URL.".to_string())
    }
}

fn emit_progress(app: &AppHandle, stage: &str, completed: u64, total: u64, message: &str) {
    let _ = app.emit(
        "flint://install-progress",
        InstallProgress {
            stage: stage.to_string(),
            completed,
            total,
            message: message.to_string(),
        },
    );
}

fn nonce() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn https_download_urls_are_enforced() {
        assert!(validate_https_url("https://piston-meta.mojang.com/file").is_ok());
        assert!(validate_https_url("http://example.com/file").is_err());
        assert!(validate_https_url("file:///tmp/file").is_err());
    }

    #[test]
    fn conflicting_download_destinations_are_rejected() {
        let mut tasks = Vec::new();
        push_unique_task(
            &mut tasks,
            DownloadTask {
                url: "https://example.com/a".to_string(),
                destination: PathBuf::from("same.jar"),
                sha1: Some("a9993e364706816aba3e25717850c26c9cd0d89d".to_string()),
                size: Some(3),
                label: "first".to_string(),
            },
        )
        .expect("first task");
        push_unique_task(
            &mut tasks,
            DownloadTask {
                url: "https://example.com/b".to_string(),
                destination: PathBuf::from("same.jar"),
                sha1: Some("b9993e364706816aba3e25717850c26c9cd0d89d".to_string()),
                size: Some(3),
                label: "second".to_string(),
            },
        )
        .expect_err("conflicting destination must fail");
        assert_eq!(tasks.len(), 1);
    }

    #[test]
    fn asset_index_flags_are_deserialized() {
        let document: AssetIndexDocument = serde_json::from_str(
            r#"{"objects":{"icons/icon.png":{"hash":"a9993e364706816aba3e25717850c26c9cd0d89d","size":12}},"virtual":true,"map_to_resources":false}"#,
        )
        .expect("asset index");
        assert!(document.virtual_assets);
        assert!(!document.map_to_resources);
        assert_eq!(document.objects.len(), 1);
    }

    #[test]
    fn asset_object_tasks_use_hashed_resource_layout() {
        let mut objects = HashMap::new();
        objects.insert(
            "minecraft/sounds/test.ogg".to_string(),
            AssetObject {
                hash: "a9993e364706816aba3e25717850c26c9cd0d89d".to_string(),
                size: 3,
            },
        );
        let document = AssetIndexDocument {
            objects,
            virtual_assets: false,
            map_to_resources: false,
        };
        let tasks = build_asset_object_tasks(Path::new("root"), &document).expect("tasks");
        assert_eq!(tasks.len(), 1);
        assert!(tasks[0]
            .destination
            .ends_with("assets/objects/a9/a9993e364706816aba3e25717850c26c9cd0d89d"));
        assert_eq!(
            tasks[0].url,
            "https://resources.download.minecraft.net/a9/a9993e364706816aba3e25717850c26c9cd0d89d"
        );
    }
}
