use crate::{
    auth, java,
    models::{LaunchRequest, LaunchResult},
    paths::{is_safe_relative_path, safe_join},
    settings,
    version::{self, Platform, RuleContext},
};

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

const MAX_NATIVE_ENTRIES: usize = 10_000;
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

    let root = crate::paths::resolve_game_dir(&request.game_dir);
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
        .filter(|id| crate::paths::component_is_safe(id))
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

    let display_args: Vec<String> = if user_type == "msa" {
        all_args
            .iter()
            .map(|argument| auth::redact_secret(argument, &access_token))
            .collect()
    } else {
        all_args.clone()
    };
    let command_string = display_command(&java_path, &display_args);
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

    for jar in jars {
        extract_native_jar(&jar, natives_dir)?;
    }
    Ok(())
}

fn extract_native_jar(jar: &NativeJarSpec, destination: &Path) -> Result<(), String> {
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
    if archive.len() > MAX_NATIVE_ENTRIES {
        return Err(format!(
            "Native library contains too many entries: {}",
            jar.path.display()
        ));
    }

    let mut total_uncompressed = 0u64;
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|error| format!("Cannot read native archive entry: {error}"))?;
        if entry.is_dir() {
            continue;
        }
        let archive_name = entry.name().replace('\\', "/");
        if is_excluded(&archive_name, &jar.excludes) {
            continue;
        }
        let Some(enclosed) = entry.enclosed_name() else {
            return Err(format!(
                "Native library contains an unsafe path: {}",
                entry.name()
            ));
        };
        let relative_text = enclosed.to_string_lossy().replace('\\', "/");
        if !is_safe_relative_path(&relative_text) {
            return Err(format!(
                "Native library contains an unsafe path: {}",
                entry.name()
            ));
        }
        let size = entry.size();
        if size > MAX_NATIVE_FILE_BYTES {
            return Err(format!(
                "Native archive entry is too large: {}",
                entry.name()
            ));
        }
        total_uncompressed = total_uncompressed
            .checked_add(size)
            .ok_or_else(|| "Native archive size overflow.".to_string())?;
        if total_uncompressed > MAX_NATIVE_TOTAL_BYTES {
            return Err("Native archive expands beyond the safety limit.".to_string());
        }

        let Some(target) = safe_join(destination, &relative_text) else {
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
        let mut output = File::create(&target).map_err(|error| {
            format!("Cannot extract native file ({}): {error}", target.display())
        })?;
        let written = io::copy(
            &mut entry.by_ref().take(MAX_NATIVE_FILE_BYTES + 1),
            &mut output,
        )
        .map_err(|error| format!("Cannot extract native file ({}): {error}", target.display()))?;
        if written > MAX_NATIVE_FILE_BYTES || written != size {
            return Err(format!(
                "Native archive entry size mismatch while extracting {}.",
                entry.name()
            ));
        }
    }
    Ok(())
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
