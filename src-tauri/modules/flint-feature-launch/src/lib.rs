use flint_core::{
    paths::{component_is_safe, resolve_game_dir, safe_join},
    ports::{ArchiveEntry, ArchiveLimits, PathPort, SafeWorkspace},
};
use flint_feature_accounts as auth;
use flint_feature_settings as settings;
use flint_feature_vanilla::{java, version::{self, Platform, RuleContext}};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::HashMap,
    fs::{self, File},
    io::{self, Read},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{SystemTime, UNIX_EPOCH},
};
use zip::ZipArchive;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LaunchRequest {
    pub game_dir: String,
    pub instance_id: String,
    pub java_path: String,
    pub username: String,
    pub min_memory_mb: u32,
    pub max_memory_mb: u32,
    pub jvm_args: String,
    pub game_args: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LaunchResult {
    pub pid: u32,
    pub command: String,
    pub log_path: String,
}

const MAX_NATIVE_ENTRIES: usize = 10_000;
const MAX_NATIVE_PATH_BYTES: usize = 4_096;
const MAX_NATIVE_FILE_BYTES: u64 = 128 * 1024 * 1024;
const MAX_NATIVE_TOTAL_BYTES: u64 = 512 * 1024 * 1024;

#[derive(Debug)]
struct NativeJarSpec {
    path: PathBuf,
    excludes: Vec<String>,
}

pub fn launch_blocking(request: LaunchRequest) -> Result<LaunchResult, String> {
    validate_launch_request(&request)?;
    version::validate_version_id(&request.instance_id)?;

    let root = resolve_game_dir(&request.game_dir);
    fs::create_dir_all(&root)
        .map_err(|error| format!("Cannot create game directory ({}): {error}", root.display()))?;
    let resolved = version::resolve_version(&root, &request.instance_id)?;
    let metadata = &resolved.metadata;
    let platform = Platform::current();
    let rule_context = platform.rule_context();
    let identity = auth::resolve_launch_identity(&request.username)?;
    let (username, auth_uuid, access_token, user_type) = match identity {
        auth::LaunchIdentity::Offline { username } => {
            let uuid = offline_uuid(&username);
            (username, uuid, "0".to_string(), "legacy".to_string())
        }
        auth::LaunchIdentity::Microsoft {
            username,
            uuid,
            access_token,
        } => (username, uuid, access_token, "msa".to_string()),
    };
    settings::validate_username(&username)?;

    let main_class = version::metadata_main_class(metadata)
        .ok_or_else(|| "Resolved version metadata has no mainClass.".to_string())?;
    let required_java = version::required_java_major(metadata);
    let java_path = java::resolve_java_path(&request.java_path)?;
    let java_details = java::inspect_java(&java_path)
        .map_err(|error| format!("Selected Java executable cannot be used: {error}"))?;
    if let Some(required) = required_java {
        if java_details.major < required {
            return Err(format!(
                "This version requires Java {required} or newer, but Java {} was selected.",
                java_details.major
            ));
        }
    }

    let libraries_dir = root.join("libraries");
    let assets_dir = root.join("assets");
    let version_path = version::version_dir(&root, &request.instance_id);
    let natives_dir = version_path.join("natives");
    prepare_natives(metadata, &rule_context, &libraries_dir, &natives_dir)?;

    let mut classpath_paths = Vec::new();
    let mut missing_libraries = Vec::new();
    for relative in version::classpath_relative_paths(metadata, &rule_context)? {
        let Some(path) = safe_join(&libraries_dir, &relative) else {
            return Err(format!("Unsafe library classpath entry: {relative}"));
        };
        if path.is_file() {
            classpath_paths.push(path.display().to_string());
        } else {
            missing_libraries.push(relative);
        }
    }
    if !missing_libraries.is_empty() {
        let preview = missing_libraries
            .iter()
            .take(4)
            .cloned()
            .collect::<Vec<_>>()
            .join(", ");
        return Err(format!(
            "Missing game libraries: {preview}. Reinstall the version and try again."
        ));
    }

    let client_jar = version::client_jar_path(&root, &resolved.jar_id);
    if !client_jar.is_file() {
        return Err(format!(
            "Client jar is missing: {}. Reinstall the version and try again.",
            client_jar.display()
        ));
    }
    classpath_paths.push(client_jar.display().to_string());
    let classpath = version::dedupe_classpath(classpath_paths).join(classpath_separator());

    let assets_index_name =
        version::asset_index_id(metadata).unwrap_or_else(|| "legacy".to_string());
    let virtual_assets = assets_dir.join("virtual").join(&assets_index_name);
    let game_assets = if virtual_assets.is_dir() {
        virtual_assets
    } else {
        assets_dir.clone()
    };
    let logging_path = metadata
        .get("logging")
        .and_then(|value| value.get("client"))
        .and_then(|value| value.get("file"))
        .and_then(|value| value.get("id"))
        .and_then(Value::as_str)
        .filter(|id| component_is_safe(id))
        .map(|id| assets_dir.join("log_configs").join(id))
        .filter(|path| path.is_file());

    let mut placeholders = HashMap::new();
    let version_type = version::metadata_version_type(metadata);
    insert_placeholder(&mut placeholders, "auth_player_name", username.clone());
    insert_placeholder(
        &mut placeholders,
        "version_name",
        request.instance_id.clone(),
    );
    insert_placeholder(
        &mut placeholders,
        "game_directory",
        root.display().to_string(),
    );
    insert_placeholder(&mut placeholders, "game_dir", root.display().to_string());
    insert_placeholder(
        &mut placeholders,
        "assets_root",
        assets_dir.display().to_string(),
    );
    insert_placeholder(
        &mut placeholders,
        "game_assets",
        game_assets.display().to_string(),
    );
    insert_placeholder(
        &mut placeholders,
        "assets_index_name",
        assets_index_name.clone(),
    );
    insert_placeholder(&mut placeholders, "auth_uuid", auth_uuid.clone());
    insert_placeholder(&mut placeholders, "auth_access_token", access_token.clone());
    insert_placeholder(&mut placeholders, "auth_session", access_token.clone());
    insert_placeholder(&mut placeholders, "clientid", "0");
    insert_placeholder(&mut placeholders, "auth_xuid", "0");
    insert_placeholder(&mut placeholders, "user_type", user_type.clone());
    insert_placeholder(&mut placeholders, "user_properties", "{}");
    insert_placeholder(&mut placeholders, "version_type", version_type.clone());
    insert_placeholder(
        &mut placeholders,
        "natives_directory",
        natives_dir.display().to_string(),
    );
    insert_placeholder(&mut placeholders, "launcher_name", "Flint");
    insert_placeholder(
        &mut placeholders,
        "launcher_version",
        env!("CARGO_PKG_VERSION"),
    );
    insert_placeholder(
        &mut placeholders,
        "launcher_content_version",
        env!("CARGO_PKG_VERSION"),
    );
    insert_placeholder(&mut placeholders, "classpath", classpath.clone());
    insert_placeholder(
        &mut placeholders,
        "classpath_separator",
        classpath_separator(),
    );
    insert_placeholder(
        &mut placeholders,
        "library_directory",
        libraries_dir.display().to_string(),
    );
    insert_placeholder(
        &mut placeholders,
        "resolution_width",
        request.width.to_string(),
    );
    insert_placeholder(
        &mut placeholders,
        "resolution_height",
        request.height.to_string(),
    );
    insert_placeholder(
        &mut placeholders,
        "primary_jar",
        client_jar.display().to_string(),
    );
    insert_placeholder(&mut placeholders, "quickPlayPath", "");
    insert_placeholder(&mut placeholders, "quickPlaySingleplayer", "");
    insert_placeholder(&mut placeholders, "quickPlayMultiplayer", "");
    insert_placeholder(&mut placeholders, "quickPlayRealms", "");
    if let Some(path) = logging_path.as_ref() {
        insert_placeholder(&mut placeholders, "path", path.display().to_string());
    }

    let mut jvm_args = Vec::new();
    jvm_args.push(format!("-Xms{}M", request.min_memory_mb));
    jvm_args.push(format!("-Xmx{}M", request.max_memory_mb));
    jvm_args.extend(
        split_command_line(&request.jvm_args)?
            .into_iter()
            .map(|argument| expand_placeholders(&argument, &placeholders)),
    );

    let metadata_jvm_args = metadata
        .get("arguments")
        .and_then(|value| value.get("jvm"))
        .map(|arguments| collect_argument_values(Some(arguments), &rule_context, &placeholders))
        .unwrap_or_default();
    let metadata_has_classpath = has_classpath_option(&metadata_jvm_args);
    jvm_args.extend(metadata_jvm_args);

    if let Some(path) = logging_path.as_ref() {
        if let Some(argument) = metadata
            .get("logging")
            .and_then(|value| value.get("client"))
            .and_then(|value| value.get("argument"))
            .and_then(Value::as_str)
        {
            let mut logging_strings = placeholders.clone();
            insert_placeholder(&mut logging_strings, "path", path.display().to_string());
            jvm_args.push(expand_placeholders(argument, &logging_strings));
        }
    }

    if !has_jvm_option(&jvm_args, "-Djava.library.path") {
        jvm_args.push(format!("-Djava.library.path={}", natives_dir.display()));
    }
    if !metadata_has_classpath {
        jvm_args.push("-cp".to_string());
        jvm_args.push(classpath.clone());
    }

    let mut game_args = if let Some(arguments) = metadata
        .get("arguments")
        .and_then(|value| value.get("game"))
    {
        collect_argument_values(Some(arguments), &rule_context, &placeholders)
    } else if let Some(legacy) = metadata.get("minecraftArguments").and_then(Value::as_str) {
        split_command_line(legacy)?
            .into_iter()
            .map(|argument| expand_placeholders(&argument, &placeholders))
            .collect()
    } else {
        legacy_game_arguments(
            &request.instance_id,
            &username,
            &root,
            &assets_dir,
            &assets_index_name,
            &auth_uuid,
            &access_token,
            &user_type,
            &version_type,
            request.width,
            request.height,
        )
    };
    game_args.extend(
        split_command_line(&request.game_args)?
            .into_iter()
            .map(|argument| expand_placeholders(&argument, &placeholders)),
    );

    let mut all_args = jvm_args;
    all_args.push(main_class);
    all_args.extend(game_args);

    let log_path = create_log_path(&root, &request.instance_id)?;
    let stdout = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .map_err(|error| format!("Cannot open launch log ({}): {error}", log_path.display()))?;
    let stderr = stdout
        .try_clone()
        .map_err(|error| format!("Cannot duplicate launch log handle: {error}"))?;

    let command_string =
        launch_command_string(&java_path, &all_args, &access_token, &user_type);
    let mut command = Command::new(&java_path);
    command
        .current_dir(&root)
        .args(&all_args)
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr));
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }

    let child = command.spawn().map_err(|error| {
        format!(
            "Cannot start Minecraft with {}: {error}",
            java_path.display()
        )
    })?;

    Ok(LaunchResult {
        pid: child.id(),
        command: command_string,
        log_path: log_path.display().to_string(),
    })
}

fn validate_launch_request(request: &LaunchRequest) -> Result<(), String> {
    if request.min_memory_mb < 128 {
        return Err("Minimum memory must be at least 128 MB.".to_string());
    }
    if request.max_memory_mb > 131_072 {
        return Err("Maximum memory must not exceed 131072 MB.".to_string());
    }
    if request.min_memory_mb > request.max_memory_mb {
        return Err("Minimum memory cannot exceed maximum memory.".to_string());
    }
    if !(320..=16_384).contains(&request.width) || !(240..=16_384).contains(&request.height) {
        return Err("Window dimensions are outside the supported range.".to_string());
    }
    Ok(())
}

fn prepare_natives(
    metadata: &Value,
    context: &RuleContext<'_>,
    libraries_dir: &Path,
    natives_dir: &Path,
) -> Result<(), String> {
    let mut jars = Vec::new();
    for library in version::allowed_libraries(metadata, context) {
        let Some((_classifier, file)) = version::library_native_file(&library, context)? else {
            continue;
        };
        let Some(path) = safe_join(libraries_dir, &file.path) else {
            return Err(format!("Unsafe native library path: {}", file.path));
        };
        if !path.is_file() {
            return Err(format!(
                "Native library is missing: {}. Reinstall the version and try again.",
                path.display()
            ));
        }
        let excludes = library
            .get("extract")
            .and_then(|value| value.get("exclude"))
            .and_then(Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(Value::as_str)
                    .map(|value| value.replace('\\', "/"))
                    .collect()
            })
            .unwrap_or_default();
        jars.push(NativeJarSpec { path, excludes });
    }

    if natives_dir.exists() {
        fs::remove_dir_all(natives_dir).map_err(|error| {
            format!(
                "Cannot clear the old natives directory ({}): {error}",
                natives_dir.display()
            )
        })?;
    }
    fs::create_dir_all(natives_dir).map_err(|error| {
        format!(
            "Cannot create natives directory ({}): {error}",
            natives_dir.display()
        )
    })?;

    let extraction_result = (|| -> Result<(), String> {
        for jar in jars {
            extract_native_jar(&jar, natives_dir)?;
        }
        Ok(())
    })();
    if let Err(error) = extraction_result {
        let _ = fs::remove_dir_all(natives_dir);
        return Err(error);
    }
    Ok(())
}

fn native_archive_limits() -> ArchiveLimits {
    ArchiveLimits {
        max_entries: MAX_NATIVE_ENTRIES,
        max_path_bytes: MAX_NATIVE_PATH_BYTES,
        max_uncompressed_bytes: MAX_NATIVE_TOTAL_BYTES,
    }
}

fn extract_native_jar(jar: &NativeJarSpec, destination: &Path) -> Result<(), String> {
    extract_native_jar_with_limits(jar, destination, &native_archive_limits())
}

fn extract_native_jar_with_limits(
    jar: &NativeJarSpec,
    destination: &Path,
    limits: &ArchiveLimits,
) -> Result<(), String> {
    struct PendingNativeEntry {
        index: usize,
        relative_path: String,
        is_directory: bool,
        excluded: bool,
        uncompressed_size: u64,
    }

    let file = File::open(&jar.path).map_err(|error| {
        format!(
            "Cannot open native library ({}): {error}",
            jar.path.display()
        )
    })?;
    let mut archive = ZipArchive::new(file).map_err(|error| {
        format!(
            "Native library is not a valid ZIP archive ({}): {error}",
            jar.path.display()
        )
    })?;

    if archive.len() > limits.max_entries {
        return Err(format!(
            "Archive contains {} entries, limit {}.",
            archive.len(),
            limits.max_entries
        ));
    }
    let mut pending_entries = Vec::new();
    let mut policy_entries = Vec::new();
    for index in 0..archive.len() {
        let entry = archive
            .by_index(index)
            .map_err(|error| format!("Cannot read native archive entry: {error}"))?;
        let archive_name = entry.name().replace('\\', "/");
        let trimmed = archive_name.trim_end_matches('/');
        if trimmed.is_empty() {
            continue;
        }
        let is_directory = entry.is_dir() || archive_name.ends_with('/');
        let is_symlink = entry
            .unix_mode()
            .map(is_zip_symlink_mode)
            .unwrap_or(false);
        let uncompressed_size = entry.size();
        policy_entries.push(ArchiveEntry {
            relative_path: trimmed.to_string(),
            uncompressed_size,
            is_directory,
            is_symlink,
        });
        pending_entries.push(PendingNativeEntry {
            index,
            relative_path: trimmed.to_string(),
            is_directory,
            excluded: is_excluded(&archive_name, &jar.excludes),
            uncompressed_size,
        });
    }

    SafeWorkspace.validate_archive_entries(&policy_entries, limits)?;

    let max_file_bytes = MAX_NATIVE_FILE_BYTES.min(limits.max_uncompressed_bytes);
    let mut total_written = 0u64;
    let mut created_files = Vec::new();
    let extraction_result = (|| -> Result<(), String> {
        for pending in pending_entries {
            if pending.is_directory || pending.excluded {
                continue;
            }
            if pending.uncompressed_size > MAX_NATIVE_FILE_BYTES {
                return Err(format!(
                    "Native archive entry is too large: {}",
                    pending.relative_path
                ));
            }
            let Some(target) = safe_join(destination, &pending.relative_path) else {
                return Err("Unsafe native extraction destination.".to_string());
            };
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).map_err(|error| {
                    format!(
                        "Cannot create native extraction directory ({}): {error}",
                        parent.display()
                    )
                })?;
            }
            let mut entry = archive
                .by_index(pending.index)
                .map_err(|error| format!("Cannot read native archive entry: {error}"))?;
            let declared_size = entry.size();
            let remaining_total = limits
                .max_uncompressed_bytes
                .saturating_sub(total_written);
            let read_limit = max_file_bytes.min(remaining_total).saturating_add(1);
            let mut output = File::create(&target).map_err(|error| {
                format!("Cannot extract native file ({}): {error}", target.display())
            })?;
            created_files.push(target.clone());
            let written = io::copy(&mut entry.by_ref().take(read_limit), &mut output)
                .map_err(|error| format!("Cannot extract native file ({}): {error}", target.display()))?;
            if written > max_file_bytes {
                return Err(format!(
                    "Native archive entry is too large: {}",
                    pending.relative_path
                ));
            }
            if written > remaining_total {
                return Err("Native archive expands beyond the safety limit.".to_string());
            }
            if written != declared_size {
                return Err(format!(
                    "Native archive entry size mismatch while extracting {}.",
                    pending.relative_path
                ));
            }
            total_written = total_written
                .checked_add(written)
                .ok_or_else(|| "Native archive size overflow.".to_string())?;
        }
        Ok(())
    })();

    if extraction_result.is_err() {
        for path in created_files.iter().rev() {
            let _ = fs::remove_file(path);
            let mut parent = path.parent();
            while let Some(directory) = parent {
                if directory == destination {
                    break;
                }
                let _ = fs::remove_dir(directory);
                parent = directory.parent();
            }
        }
    }
    extraction_result
}

fn is_zip_symlink_mode(mode: u32) -> bool {
    mode & 0o170000 == 0o120000
}
fn is_excluded(name: &str, excludes: &[String]) -> bool {
    excludes.iter().any(|exclude| {
        let exclude = exclude.trim_start_matches('/');
        !exclude.is_empty() && (name == exclude || name.starts_with(exclude))
    })
}

pub fn collect_argument_values(
    value: Option<&Value>,
    context: &RuleContext<'_>,
    placeholders: &HashMap<String, String>,
) -> Vec<String> {
    let Some(items) = value.and_then(Value::as_array) else {
        return Vec::new();
    };
    let mut result = Vec::new();
    for item in items {
        if let Some(text) = item.as_str() {
            result.push(expand_placeholders(text, placeholders));
            continue;
        }
        let Some(object) = item.as_object() else {
            continue;
        };
        if let Some(rules) = object.get("rules") {
            if !version::rules_allow(Some(rules), context) {
                continue;
            }
        }
        let Some(value) = object.get("value") else {
            continue;
        };
        if let Some(text) = value.as_str() {
            result.push(expand_placeholders(text, placeholders));
        } else if let Some(values) = value.as_array() {
            for text in values.iter().filter_map(Value::as_str) {
                result.push(expand_placeholders(text, placeholders));
            }
        }
    }
    result
}

pub fn expand_placeholders(value: &str, placeholders: &HashMap<String, String>) -> String {
    let mut result = String::with_capacity(value.len());
    let mut remaining = value;
    while let Some(start) = remaining.find("${") {
        result.push_str(&remaining[..start]);
        let after_start = &remaining[start + 2..];
        if let Some(end) = after_start.find('}') {
            let key = &after_start[..end];
            if let Some(replacement) = placeholders.get(key) {
                result.push_str(replacement);
            } else {
                result.push_str("${");
                result.push_str(key);
                result.push('}');
            }
            remaining = &after_start[end + 1..];
        } else {
            result.push_str(&remaining[start..]);
            remaining = "";
        }
    }
    result.push_str(remaining);
    result
}

pub fn split_command_line(value: &str) -> Result<Vec<String>, String> {
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum QuoteState {
        None,
        Single,
        Double,
    }

    let mut arguments = Vec::new();
    let mut current = String::new();
    let mut state = QuoteState::None;
    let mut started = false;
    let mut chars = value.chars().peekable();

    while let Some(character) = chars.next() {
        match state {
            QuoteState::None => match character {
                '\'' => {
                    state = QuoteState::Single;
                    started = true;
                }
                '"' => {
                    state = QuoteState::Double;
                    started = true;
                }
                character if character.is_whitespace() => {
                    if started {
                        arguments.push(current.clone());
                        current.clear();
                        started = false;
                    }
                }
                _ => {
                    current.push(character);
                    started = true;
                }
            },
            QuoteState::Single => {
                if character == '\'' {
                    state = QuoteState::None;
                } else {
                    current.push(character);
                }
            }
            QuoteState::Double => {
                if character == '"' {
                    state = QuoteState::None;
                } else if character == '\\' {
                    match chars.peek().copied() {
                        Some(next) if next == '"' || next == '\\' => {
                            chars.next();
                            current.push(next);
                        }
                        _ => current.push('\\'),
                    }
                } else {
                    current.push(character);
                }
            }
        }
    }

    if state != QuoteState::None {
        return Err("Unclosed quote in launch arguments.".to_string());
    }
    if started {
        arguments.push(current);
    }
    Ok(arguments)
}

pub fn offline_uuid(username: &str) -> String {
    let digest = md5::compute(format!("OfflinePlayer:{username}").as_bytes());
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&digest.0);
    bytes[6] = (bytes[6] & 0x0f) | 0x30;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    bytes
        .iter()
        .map(|byte| format!("{:02x}", *byte))
        .collect::<String>()
}

fn legacy_game_arguments(
    instance_id: &str,
    username: &str,
    root: &Path,
    assets_dir: &Path,
    assets_index_name: &str,
    uuid: &str,
    access_token: &str,
    user_type: &str,
    version_type: &str,
    width: u32,
    height: u32,
) -> Vec<String> {
    vec![
        "--username".to_string(),
        username.to_string(),
        "--version".to_string(),
        instance_id.to_string(),
        "--gameDir".to_string(),
        root.display().to_string(),
        "--assetsDir".to_string(),
        assets_dir.display().to_string(),
        "--assetIndex".to_string(),
        assets_index_name.to_string(),
        "--uuid".to_string(),
        uuid.to_string(),
        "--accessToken".to_string(),
        access_token.to_string(),
        "--userType".to_string(),
        user_type.to_string(),
        "--versionType".to_string(),
        version_type.to_string(),
        "--width".to_string(),
        width.to_string(),
        "--height".to_string(),
        height.to_string(),
    ]
}

fn insert_placeholder(map: &mut HashMap<String, String>, key: &str, value: impl Into<String>) {
    map.insert(key.to_string(), value.into());
}

fn has_classpath_option(arguments: &[String]) -> bool {
    arguments.iter().any(|argument| {
        argument == "-cp"
            || argument == "-classpath"
            || argument.starts_with("-cp=")
            || argument.starts_with("-classpath=")
    })
}

fn has_jvm_option(arguments: &[String], prefix: &str) -> bool {
    arguments
        .iter()
        .any(|argument| argument == prefix || argument.starts_with(&format!("{prefix}=")))
}

fn classpath_separator() -> &'static str {
    if cfg!(windows) {
        ";"
    } else {
        ":"
    }
}

fn create_log_path(root: &Path, instance_id: &str) -> Result<PathBuf, String> {
    let logs_dir = root.join("logs");
    fs::create_dir_all(&logs_dir).map_err(|error| {
        format!(
            "Cannot create logs directory ({}): {error}",
            logs_dir.display()
        )
    })?;
    let safe_instance: String = instance_id
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '+') {
                character
            } else {
                '_'
            }
        })
        .take(64)
        .collect();
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();
    Ok(logs_dir.join(format!("flint-{safe_instance}-{timestamp}.log")))
}

fn launch_command_string(
    java_path: &Path,
    all_args: &[String],
    access_token: &str,
    user_type: &str,
) -> String {
    let display_args: Vec<String> = if user_type == "msa" {
        all_args
            .iter()
            .map(|argument| auth::redact_secret(argument, access_token))
            .collect()
    } else {
        all_args.to_vec()
    };
    display_command(java_path, &display_args)
}
fn display_command(program: &Path, arguments: &[String]) -> String {
    let mut parts = Vec::with_capacity(arguments.len() + 1);
    parts.push(quote_command_argument(&program.display().to_string()));
    parts.extend(
        arguments
            .iter()
            .map(|argument| quote_command_argument(argument)),
    );
    parts.join(" ")
}

fn quote_command_argument(argument: &str) -> String {
    if cfg!(windows) {
        quote_windows_argument(argument)
    } else {
        quote_posix_argument(argument)
    }
}

fn quote_windows_argument(argument: &str) -> String {
    if !argument.is_empty()
        && !argument
            .chars()
            .any(|character| character.is_whitespace() || character == '"')
    {
        return argument.to_string();
    }
    let mut quoted = String::from("\"");
    let mut backslashes = 0usize;
    for character in argument.chars() {
        if character == '\\' {
            backslashes += 1;
        } else if character == '"' {
            quoted.push_str(&"\\".repeat(backslashes * 2 + 1));
            quoted.push('"');
            backslashes = 0;
        } else {
            quoted.push_str(&"\\".repeat(backslashes));
            quoted.push(character);
            backslashes = 0;
        }
    }
    quoted.push_str(&"\\".repeat(backslashes * 2));
    quoted.push('"');
    quoted
}

fn quote_posix_argument(argument: &str) -> String {
    if argument.is_empty() {
        return "''".to_string();
    }
    format!("'{}'", argument.replace('\'', "'\\''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_context() -> (Platform, HashMap<String, String>) {
        let platform = Platform {
            os_name: "windows".to_string(),
            os_version: "10.0.19045".to_string(),
            arch: "x86_64".to_string(),
            features: HashMap::from([("has_custom_resolution".to_string(), true)]),
        };
        let placeholders = HashMap::from([
            ("auth_player_name".to_string(), "Alex".to_string()),
            ("resolution_width".to_string(), "1280".to_string()),
            ("resolution_height".to_string(), "720".to_string()),
        ]);
        (platform, placeholders)
    }

    use flint_core::ports::ArchiveLimits;
    use std::{io::Write, path::PathBuf};
    use zip::{write::SimpleFileOptions, ZipWriter};

    fn temp_root(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "flint-{label}-{}-{}",
            std::process::id(),
            flint_core::net::nonce()
        ))
    }

    fn write_native_jar(path: &Path, entries: &[(&str, &[u8])]) {
        let file = File::create(path).expect("native jar file");
        let mut writer = ZipWriter::new(file);
        for (name, data) in entries {
            writer
                .start_file(*name, SimpleFileOptions::default())
                .expect("native zip entry");
            writer.write_all(data).expect("native zip data");
        }
        writer.finish().expect("finish native jar");
    }

    fn write_symlink_native_jar(path: &Path, name: &str) {
        let file = File::create(path).expect("symlink native jar");
        let mut writer = ZipWriter::new(file);
        writer
            .start_file(
                name,
                SimpleFileOptions::default().unix_permissions(0o120777),
            )
            .expect("symlink native entry");
        writer.write_all(b"../outside").expect("symlink target");
        writer.finish().expect("finish symlink jar");
    }

    fn patch_zip_uncompressed_size(path: &Path, uncompressed_size: u32) {
        let mut bytes = fs::read(path).expect("read native jar for patch");
        for (signature, offset) in [
            ([0x50_u8, 0x4b, 0x03, 0x04], 22usize),
            ([0x50, 0x4b, 0x01, 0x02], 24usize),
        ] {
            let position = bytes
                .windows(signature.len())
                .position(|window| window == &signature[..])
                .expect("native zip signature");
            let field = position + offset;
            bytes[field..field + 4]
                .copy_from_slice(&uncompressed_size.to_le_bytes());
        }
        fs::write(path, bytes).expect("write patched native jar");
    }

    fn native_spec(path: &Path) -> NativeJarSpec {
        NativeJarSpec {
            path: path.to_path_buf(),
            excludes: Vec::new(),
        }
    }

    fn native_limits(
        max_entries: usize,
        max_uncompressed_bytes: u64,
    ) -> ArchiveLimits {
        ArchiveLimits {
            max_entries,
            max_path_bytes: MAX_NATIVE_PATH_BYTES,
            max_uncompressed_bytes,
        }
    }
    #[test]
    fn expands_repeated_and_embedded_placeholders() {
        let placeholders = HashMap::from([
            ("name".to_string(), "Alex".to_string()),
            ("sep".to_string(), ";".to_string()),
        ]);
        assert_eq!(
            expand_placeholders("--user ${name}${sep}${unknown}", &placeholders),
            "--user Alex;${unknown}"
        );
    }

    #[test]
    fn splits_legacy_arguments_without_shell_execution() {
        let arguments = split_command_line(
            "--username \"Alex Smith\" --path C:\\Games\\Minecraft --empty \"\"",
        )
        .expect("arguments");
        assert_eq!(
            arguments,
            vec![
                "--username",
                "Alex Smith",
                "--path",
                "C:\\Games\\Minecraft",
                "--empty",
                ""
            ]
        );
        assert!(split_command_line("--broken \"quote").is_err());
    }

    #[test]
    fn conditional_arguments_honor_platform_rules() {
        let (platform, placeholders) = test_context();
        let context = platform.rule_context();
        let arguments = serde_json::json!([
            "--always",
            {
                "rules": [{"action": "allow", "os": {"name": "windows"}}],
                "value": ["--width", "${resolution_width}"]
            },
            {
                "rules": [{"action": "allow", "os": {"name": "osx"}}],
                "value": ["--mac-only"]
            },
            {
                "rules": [{"action": "allow", "features": {"has_custom_resolution": true}}],
                "value": "--custom"
            }
        ]);
        assert_eq!(
            collect_argument_values(Some(&arguments), &context, &placeholders),
            vec!["--always", "--width", "1280", "--custom"]
        );
    }

    #[test]
    fn classpath_option_detection_covers_supported_spellings() {
        assert!(has_classpath_option(&["-cp".to_string()]));
        assert!(has_classpath_option(&["-classpath".to_string()]));
        assert!(has_classpath_option(&["-cp=foo".to_string()]));
        assert!(has_classpath_option(&["-classpath=foo".to_string()]));
        assert!(!has_classpath_option(
            &["-Dfile.encoding=UTF-8".to_string()]
        ));
    }

    #[test]
    fn offline_uuid_is_deterministic_and_version_three() {
        let first = offline_uuid("Alex");
        assert_eq!(first, offline_uuid("Alex"));
        assert_ne!(first, offline_uuid("Steve"));
        assert_eq!(first.len(), 32);
        assert_eq!(&first[12..13], "3");
        assert!(matches!(&first[16..17], "8" | "9" | "a" | "b"));
    }

    #[test]
    fn windows_quoting_preserves_spaced_paths_and_quotes() {
        assert_eq!(
            quote_windows_argument("C:\\Games\\Minecraft"),
            "C:\\Games\\Minecraft"
        );
        assert_eq!(
            quote_windows_argument("C:\\My Games\\Minecraft"),
            "\"C:\\My Games\\Minecraft\""
        );
        assert_eq!(quote_windows_argument("a\"b"), "\"a\\\"b\"");
    }

    #[test]
    fn prepare_natives_removes_partial_directory_after_jar_failure() {
        let root = temp_root("native-prepare-cleanup");
        let libraries = root.join("libraries");
        let native_library = libraries.join("org/example/native/1/native-1-natives-windows-64.jar");
        fs::create_dir_all(native_library.parent().expect("library parent"))
            .expect("library directory");
        fs::write(&native_library, b"not a zip").expect("invalid native jar");
        let natives = root.join("natives");
        let (platform, _) = test_context();
        let context = platform.rule_context();
        let metadata = serde_json::json!({
            "libraries": [{
                "name": "org.example:native:1",
                "downloads": {"classifiers": {"natives-windows-64": {"path": "org/example/native/1/native-1-natives-windows-64.jar"}}},
                "natives": {"windows": "natives-windows-${arch}"}
            }]
        });
        let error = prepare_natives(&metadata, &context, &libraries, &natives)
            .expect_err("invalid native jar must fail");
        assert!(error.contains("not a valid ZIP archive"));
        assert!(!natives.exists());
        let _ = fs::remove_dir_all(root);
    }
    #[test]
    fn native_jar_traversal_is_rejected_by_core_archive_policy() {
        let root = temp_root("native-traversal");
        let destination = root.join("natives");
        fs::create_dir_all(&destination).expect("natives directory");
        let jar = root.join("traversal.jar");
        write_native_jar(&jar, &[("../escape.dll", b"bad")]);
        let error = extract_native_jar(&native_spec(&jar), &destination)
            .expect_err("traversal must fail closed");
        assert!(error.contains("Unsafe archive entry path"));
        assert!(!root.join("escape.dll").exists());
        assert_eq!(fs::read_dir(&destination).expect("natives entries").count(), 0);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn native_jar_symlink_is_rejected_by_core_archive_policy() {
        let root = temp_root("native-symlink");
        let destination = root.join("natives");
        fs::create_dir_all(&destination).expect("natives directory");
        let jar = root.join("symlink.jar");
        write_symlink_native_jar(&jar, "native/link.dll");
        let error = extract_native_jar(&native_spec(&jar), &destination)
            .expect_err("symlink must fail closed");
        assert!(error.to_ascii_lowercase().contains("symlink"));
        assert_eq!(fs::read_dir(&destination).expect("natives entries").count(), 0);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn native_jar_duplicate_and_case_alias_entries_fail_before_writing() {
        let root = temp_root("native-duplicate");
        let destination = root.join("natives");
        fs::create_dir_all(&destination).expect("natives directory");
        let jar = root.join("duplicate.jar");
        write_native_jar(
            &jar,
            &[("native/A.dll", b"one"), ("native/a.DLL", b"two")],
        );
        let error = extract_native_jar(&native_spec(&jar), &destination)
            .expect_err("case alias duplicate must fail closed");
        assert!(error.contains("duplicate entry path"));
        assert_eq!(fs::read_dir(&destination).expect("natives entries").count(), 0);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn native_jar_entry_and_total_limits_fail_before_writing() {
        let root = temp_root("native-limits");
        let destination = root.join("natives");
        fs::create_dir_all(&destination).expect("natives directory");
        let jar = root.join("limits.jar");
        write_native_jar(
            &jar,
            &[("native/one.dll", b"one"), ("native/two.dll", b"two")],
        );

        let entry_error = extract_native_jar_with_limits(
            &native_spec(&jar),
            &destination,
            &native_limits(1, MAX_NATIVE_TOTAL_BYTES),
        )
        .expect_err("entry count limit must fail closed");
        assert_eq!(entry_error, "Archive contains 2 entries, limit 1.");
        assert_eq!(fs::read_dir(&destination).expect("natives entries").count(), 0);

        let size_error = extract_native_jar_with_limits(
            &native_spec(&jar),
            &destination,
            &native_limits(MAX_NATIVE_ENTRIES, 1),
        )
        .expect_err("total size limit must fail closed");
        assert!(size_error.contains("uncompressed data exceeds"));

        let per_file_jar = root.join("per-file-limit.jar");
        write_native_jar(&per_file_jar, &[("native/large.dll", b"small")]);
        patch_zip_uncompressed_size(&per_file_jar, (MAX_NATIVE_FILE_BYTES + 1) as u32);
        let per_file_error = extract_native_jar(&native_spec(&per_file_jar), &destination)
            .expect_err("per-file declaration limit must fail closed");
        assert!(per_file_error.contains("too large"));
        assert_eq!(fs::read_dir(&destination).expect("natives entries").count(), 0);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn native_jar_entry_limit_rejects_before_path_validation_or_writing() {
        let root = temp_root("native-early-entry-limit");
        let destination = root.join("natives");
        fs::create_dir_all(&destination).expect("natives directory");
        let jar = root.join("early-limit.jar");
        write_native_jar(
            &jar,
            &[("../escape.dll", b"bad"), ("native/real.dll", b"ok")],
        );
        let error = extract_native_jar_with_limits(
            &native_spec(&jar),
            &destination,
            &native_limits(1, MAX_NATIVE_TOTAL_BYTES),
        )
        .expect_err("entry-count limit must reject before collection/path validation");
        assert_eq!(error, "Archive contains 2 entries, limit 1.");
        assert!(!root.join("escape.dll").exists());
        assert_eq!(fs::read_dir(&destination).expect("natives entries").count(), 0);
        let _ = fs::remove_dir_all(root);
    }
    #[test]
    fn native_jar_partial_output_is_removed_after_extraction_failure() {
        let root = temp_root("native-partial");
        let destination = root.join("natives");
        fs::create_dir_all(destination.join("native").join("blocked")).expect("blocked directory");
        fs::create_dir_all(destination.join("native").join("blocked").join("second.dll"))
            .expect("blocked target directory");
        let jar = root.join("partial.jar");
        write_native_jar(
            &jar,
            &[
                ("native/first.dll", b"first"),
                ("native/blocked/second.dll", b"second"),
            ],
        );
        let error = extract_native_jar(&native_spec(&jar), &destination)
            .expect_err("file/directory collision must fail after partial extraction");
        assert!(error.contains("Cannot extract native file"));
        assert!(!destination.join("native").join("first.dll").exists());
        assert!(!destination.join("native").join("blocked").join("second.dll").is_file());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn native_jar_declared_actual_expansion_fails_and_removes_partial_file() {
        let root = temp_root("native-expansion");
        let destination = root.join("natives");
        fs::create_dir_all(&destination).expect("natives directory");
        let jar = root.join("expanded.jar");
        write_native_jar(&jar, &[("native/expanded.dll", b"expanded native payload")]);
        patch_zip_uncompressed_size(&jar, 1);
        let error = extract_native_jar(&native_spec(&jar), &destination)
            .expect_err("declared/actual size mismatch must fail closed");
        assert!(
            error.contains("size mismatch")
                || error.contains("Cannot extract native file")
        );
        assert!(!destination.join("native").join("expanded.dll").exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn launch_result_command_redacts_microsoft_token_and_keeps_offline_command() {
        let arguments = vec![
            "--accessToken".to_string(),
            "microsoft-secret-token".to_string(),
            "--username".to_string(),
            "Alex".to_string(),
        ];
        let online_command = launch_command_string(
            Path::new("java"),
            &arguments,
            "microsoft-secret-token",
            "msa",
        );
        let online = LaunchResult {
            pid: 0,
            command: online_command,
            log_path: "game/logs/latest.log".to_string(),
        };
        assert!(!online.command.contains("microsoft-secret-token"));
        assert!(online.command.contains("[REDACTED]"));

        let offline_arguments = vec![
            "--accessToken".to_string(),
            "0".to_string(),
            "--username".to_string(),
            "Alex".to_string(),
        ];
        let offline_command =
            launch_command_string(Path::new("java"), &offline_arguments, "0", "legacy");
        let offline = LaunchResult {
            pid: 0,
            command: offline_command,
            log_path: "game/logs/latest.log".to_string(),
        };
        assert!(offline.command.contains("--accessToken 0"));
        assert!(!offline.command.contains("[REDACTED]"));
    }
    #[test]
    fn native_exclusions_match_prefixes() {
        assert!(is_excluded(
            "META-INF/MANIFEST.MF",
            &["META-INF/".to_string()]
        ));
        assert!(!is_excluded(
            "org/lwjgl/lwjgl.dll",
            &["META-INF/".to_string()]
        ));
    }
}
