use crate::{
    models::InstanceInfo,
    paths::{
        component_is_safe, normalize_relative_metadata_path, path_dedup_key, resolve_game_dir,
    },
};
use regex::Regex;
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

const MAX_VERSION_JSON_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct ResolvedVersion {
    pub metadata: Value,
    pub jar_id: String,
    #[allow(dead_code)]
    pub chain: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct LibraryFile {
    pub path: String,
    pub url: Option<String>,
    pub sha1: Option<String>,
    pub size: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct Platform {
    pub os_name: String,
    pub os_version: String,
    pub arch: String,
    pub features: HashMap<String, bool>,
}

pub struct RuleContext<'a> {
    pub os_name: &'a str,
    pub os_version: &'a str,
    pub arch: &'a str,
    pub features: &'a HashMap<String, bool>,
}

impl Platform {
    pub fn current() -> Self {
        let os_name = normalize_os(env::consts::OS);
        let arch = normalize_arch(env::consts::ARCH);
        let os_version = detect_os_version(&os_name);
        let mut features = HashMap::new();
        features.insert("is_demo_user".to_string(), false);
        features.insert("has_custom_resolution".to_string(), true);
        features.insert("has_quick_plays_support".to_string(), false);
        features.insert("is_quick_play_singleplayer".to_string(), false);
        features.insert("is_quick_play_multiplayer".to_string(), false);
        features.insert("is_quick_play_realms".to_string(), false);
        Self {
            os_name,
            os_version,
            arch,
            features,
        }
    }

    pub fn rule_context(&self) -> RuleContext<'_> {
        RuleContext {
            os_name: &self.os_name,
            os_version: &self.os_version,
            arch: &self.arch,
            features: &self.features,
        }
    }
}

pub fn normalize_os(value: &str) -> String {
    match value.to_ascii_lowercase().as_str() {
        "osx" | "darwin" | "mac" | "macos" => "macos".to_string(),
        "win" | "win32" | "windows" => "windows".to_string(),
        "linux" => "linux".to_string(),
        other => other.to_string(),
    }
}

pub fn normalize_arch(value: &str) -> String {
    match value.to_ascii_lowercase().as_str() {
        "amd64" | "x86_64" | "x64" => "x86_64".to_string(),
        "x86" | "i386" | "i486" | "i586" | "i686" => "x86".to_string(),
        "arm64" | "aarch64" => "aarch64".to_string(),
        "arm" | "armv7" | "armv7l" => "arm".to_string(),
        other => other.to_string(),
    }
}

pub fn rules_allow(rules: Option<&Value>, context: &RuleContext<'_>) -> bool {
    let Some(rules) = rules.and_then(Value::as_array) else {
        return rules.is_none();
    };
    let mut allowed = false;
    for rule in rules {
        let action = rule.get("action").and_then(Value::as_str);
        if rule_matches(rule, context) {
            match action {
                Some("allow") => allowed = true,
                Some("disallow") => allowed = false,
                _ => {}
            }
        }
    }
    allowed
}

fn rule_matches(rule: &Value, context: &RuleContext<'_>) -> bool {
    if let Some(os) = rule.get("os") {
        if let Some(name) = os.get("name").and_then(Value::as_str) {
            if normalize_os(name) != context.os_name {
                return false;
            }
        }
        if let Some(arch) = os.get("arch").and_then(Value::as_str) {
            if normalize_arch(arch) != context.arch {
                return false;
            }
        }
        if let Some(version) = os.get("version").and_then(Value::as_str) {
            if context.os_version.is_empty() {
                return false;
            }
            let Ok(pattern) = Regex::new(version) else {
                return false;
            };
            if !pattern.is_match(context.os_version) {
                return false;
            }
        }
    }

    if let Some(features) = rule.get("features").and_then(Value::as_object) {
        for (name, expected) in features {
            let Some(expected) = expected.as_bool() else {
                return false;
            };
            let actual = context.features.get(name).copied().unwrap_or(false);
            if actual != expected {
                return false;
            }
        }
    }
    true
}

pub fn library_allowed(library: &Value, context: &RuleContext<'_>) -> bool {
    rules_allow(library.get("rules"), context)
}

pub fn allowed_libraries(metadata: &Value, context: &RuleContext<'_>) -> Vec<Value> {
    metadata
        .get("libraries")
        .and_then(Value::as_array)
        .map(|libraries| {
            libraries
                .iter()
                .filter(|library| library_allowed(library, context))
                .cloned()
                .collect()
        })
        .unwrap_or_default()
}

pub fn merge_version_json(parent: Value, child: Value) -> Value {
    match (parent, child) {
        (Value::Object(parent), Value::Object(child)) => {
            let mut merged = parent;
            for (key, child_value) in child {
                if let Some(parent_value) = merged.remove(&key) {
                    merged.insert(key, merge_value(parent_value, child_value));
                } else {
                    merged.insert(key, child_value);
                }
            }
            Value::Object(merged)
        }
        (_, child) => child,
    }
}

fn merge_value(parent: Value, child: Value) -> Value {
    match (parent, child) {
        (Value::Object(parent), Value::Object(child)) => {
            let mut merged = parent;
            for (key, child_value) in child {
                if let Some(parent_value) = merged.remove(&key) {
                    merged.insert(key, merge_value(parent_value, child_value));
                } else {
                    merged.insert(key, child_value);
                }
            }
            Value::Object(merged)
        }
        (Value::Array(mut parent), Value::Array(child)) => {
            parent.extend(child);
            Value::Array(parent)
        }
        (_, child) => child,
    }
}

pub fn validate_version_id(version_id: &str) -> Result<(), String> {
    if component_is_safe(version_id) {
        Ok(())
    } else {
        Err("Invalid version id.".to_string())
    }
}

pub fn version_json_path(game_dir: &Path, version_id: &str) -> PathBuf {
    game_dir
        .join("versions")
        .join(version_id)
        .join(format!("{version_id}.json"))
}

pub fn version_dir(game_dir: &Path, version_id: &str) -> PathBuf {
    game_dir.join("versions").join(version_id)
}

pub fn client_jar_path(game_dir: &Path, jar_id: &str) -> PathBuf {
    game_dir
        .join("versions")
        .join(jar_id)
        .join(format!("{jar_id}.jar"))
}

pub fn load_version_json(path: &Path) -> Result<Value, String> {
    let info = fs::metadata(path)
        .map_err(|error| format!("Cannot read version metadata ({}): {error}", path.display()))?;
    if info.len() > MAX_VERSION_JSON_BYTES {
        return Err(format!(
            "Version metadata is unexpectedly large ({}).",
            path.display()
        ));
    }
    let text = fs::read_to_string(path)
        .map_err(|error| format!("Cannot read version metadata ({}): {error}", path.display()))?;
    let value: Value = serde_json::from_str(&text).map_err(|error| {
        format!(
            "Version metadata is invalid JSON ({}): {error}",
            path.display()
        )
    })?;
    if !value.is_object() {
        return Err(format!(
            "Version metadata must be a JSON object ({}).",
            path.display()
        ));
    }
    Ok(value)
}

pub fn resolve_version(game_dir: &Path, version_id: &str) -> Result<ResolvedVersion, String> {
    validate_version_id(version_id)?;
    let mut current = version_id.to_string();
    let mut visiting = HashSet::new();
    let mut chain = Vec::new();
    let mut merged: Option<Value> = None;
    let mut explicit_jar: Option<String> = None;
    let base_id;

    loop {
        validate_version_id(&current)?;
        if !visiting.insert(current.clone()) {
            return Err(format!("Version inheritance cycle detected at {current}."));
        }
        if chain.len() >= 32 {
            return Err("Version inheritance chain is too deep.".to_string());
        }

        let value = load_version_json(&version_json_path(game_dir, &current))?;
        if explicit_jar.is_none() {
            if let Some(jar) = value.get("jar").and_then(Value::as_str) {
                validate_version_id(jar)?;
                explicit_jar = Some(jar.to_string());
            }
        }

        merged = Some(match merged {
            Some(child) => merge_version_json(value.clone(), child),
            None => value.clone(),
        });
        chain.push(current.clone());

        match value.get("inheritsFrom").and_then(Value::as_str) {
            Some(parent) => current = parent.to_string(),
            None => {
                base_id = current;
                break;
            }
        }
    }

    let metadata = merged.ok_or_else(|| "Version metadata could not be resolved.".to_string())?;
    Ok(ResolvedVersion {
        metadata,
        jar_id: explicit_jar.unwrap_or(base_id),
        chain,
    })
}

pub fn maven_artifact_path(
    name: &str,
    classifier_override: Option<&str>,
) -> Result<Option<String>, String> {
    let (coordinates, extension) = match name.rsplit_once('@') {
        Some((coordinates, extension)) if !extension.is_empty() => (coordinates, extension),
        _ => (name, "jar"),
    };
    let parts: Vec<&str> = coordinates.split(':').collect();
    if parts.len() < 3 {
        return Ok(None);
    }
    if parts.len() > 4 {
        return Err(format!("Unsupported Maven coordinate: {name}"));
    }
    if parts.iter().any(|part| part.is_empty()) {
        return Err(format!("Invalid Maven coordinate: {name}"));
    }

    let group = parts[0];
    let artifact = parts[1];
    let version = parts[2];
    let classifier = classifier_override.or_else(|| parts.get(3).copied());
    let classifier_suffix = classifier
        .map(|value| format!("-{value}"))
        .unwrap_or_default();
    let relative = format!(
        "{}/{}/{}/{artifact}-{version}{classifier_suffix}.{extension}",
        group.replace('.', "/"),
        artifact,
        version
    );
    normalize_relative_metadata_path(&relative)
        .map(Some)
        .ok_or_else(|| format!("Unsafe Maven artifact path derived from: {name}"))
}

pub fn library_file(
    library: &Value,
    classifier: Option<&str>,
) -> Result<Option<LibraryFile>, String> {
    let download = match classifier {
        Some(classifier) => library
            .get("downloads")
            .and_then(|downloads| downloads.get("classifiers"))
            .and_then(|classifiers| classifiers.get(classifier)),
        None => library
            .get("downloads")
            .and_then(|downloads| downloads.get("artifact")),
    };

    let metadata_path = download
        .and_then(|value| value.get("path"))
        .and_then(Value::as_str)
        .map(|value| value.to_string());
    let maven_path = library
        .get("name")
        .and_then(Value::as_str)
        .map(|name| maven_artifact_path(name, classifier))
        .transpose()?
        .flatten();
    let path = match metadata_path.or(maven_path) {
        Some(path) => normalize_relative_metadata_path(&path)
            .ok_or_else(|| format!("Unsafe library path: {path}"))?,
        None => return Ok(None),
    };

    let explicit_url = download
        .and_then(|value| value.get("url"))
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string());
    let url = explicit_url.or_else(|| {
        let base = library
            .get("url")
            .and_then(Value::as_str)
            .unwrap_or("https://libraries.minecraft.net/");
        Some(format!("{}/{}", base.trim_end_matches('/'), path))
    });
    let sha1 = download
        .and_then(|value| value.get("sha1"))
        .and_then(Value::as_str)
        .map(|value| value.to_ascii_lowercase());
    let size = download
        .and_then(|value| value.get("size"))
        .and_then(Value::as_u64);
    Ok(Some(LibraryFile {
        path,
        url,
        sha1,
        size,
    }))
}

pub fn native_classifier(library: &Value, context: &RuleContext<'_>) -> Option<String> {
    let natives = library.get("natives")?.as_object()?;
    let candidates: &[&str] = match context.os_name {
        "macos" => &["osx", "macos"],
        "windows" => &["windows"],
        "linux" => &["linux"],
        _ => &[],
    };
    for key in candidates {
        if let Some(value) = natives.get(*key).and_then(Value::as_str) {
            let arch = if context.arch == "x86" || context.arch == "arm" {
                "32"
            } else {
                "64"
            };
            return Some(value.replace("${arch}", arch));
        }
    }
    None
}

pub fn library_native_file(
    library: &Value,
    context: &RuleContext<'_>,
) -> Result<Option<(String, LibraryFile)>, String> {
    let Some(classifier) = native_classifier(library, context) else {
        return Ok(None);
    };
    match library_file(library, Some(&classifier))? {
        Some(file) => Ok(Some((classifier, file))),
        None => Ok(None),
    }
}

pub fn classpath_relative_paths(
    metadata: &Value,
    context: &RuleContext<'_>,
) -> Result<Vec<String>, String> {
    let mut paths = Vec::new();
    for library in allowed_libraries(metadata, context) {
        if let Some(file) = library_file(&library, None)? {
            paths.push(file.path);
        }
    }
    Ok(dedupe_classpath(paths))
}

pub fn dedupe_classpath<I>(entries: I) -> Vec<String>
where
    I: IntoIterator<Item = String>,
{
    let mut seen = HashSet::new();
    let mut result = Vec::new();
    for entry in entries {
        if entry.is_empty() {
            continue;
        }
        let key = path_dedup_key(Path::new(&entry));
        if seen.insert(key) {
            result.push(entry);
        }
    }
    result
}

pub fn asset_index_id(metadata: &Value) -> Option<String> {
    metadata
        .get("assetIndex")
        .and_then(|value| value.get("id"))
        .and_then(Value::as_str)
        .or_else(|| metadata.get("assets").and_then(Value::as_str))
        .map(|value| value.to_string())
}

pub fn required_java_major(metadata: &Value) -> Option<u32> {
    metadata
        .get("javaVersion")
        .and_then(|value| value.get("majorVersion"))
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
}

pub fn metadata_version_type(metadata: &Value) -> String {
    metadata
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("release")
        .to_string()
}

pub fn metadata_main_class(metadata: &Value) -> Option<String> {
    metadata
        .get("mainClass")
        .and_then(Value::as_str)
        .map(|value| value.to_string())
}

pub fn detect_loader(version_id: &str, metadata: Option<&Value>) -> String {
    let mut hint = version_id.to_ascii_lowercase();
    if let Some(metadata) = metadata {
        if let Some(main_class) = metadata.get("mainClass").and_then(Value::as_str) {
            hint.push(' ');
            hint.push_str(&main_class.to_ascii_lowercase());
        }
        if let Some(parent) = metadata.get("inheritsFrom").and_then(Value::as_str) {
            hint.push(' ');
            hint.push_str(&parent.to_ascii_lowercase());
        }
        if let Some(libraries) = metadata.get("libraries").and_then(Value::as_array) {
            for library in libraries.iter().take(128) {
                if let Some(name) = library.get("name").and_then(Value::as_str) {
                    hint.push(' ');
                    hint.push_str(&name.to_ascii_lowercase());
                }
            }
        }
    }

    if hint.contains("neoforge") {
        "NeoForge".to_string()
    } else if hint.contains("quilt") {
        "Quilt".to_string()
    } else if hint.contains("fabric") {
        "Fabric".to_string()
    } else if hint.contains("optifine") {
        "OptiFine".to_string()
    } else if hint.contains("forge") {
        "Forge".to_string()
    } else if hint.contains("liteloader") {
        "LiteLoader".to_string()
    } else {
        "Vanilla".to_string()
    }
}

pub fn list_instances(game_dir: &str) -> Result<Vec<InstanceInfo>, String> {
    let root = resolve_game_dir(game_dir);
    let versions_dir = root.join("versions");
    if !versions_dir.exists() {
        return Ok(Vec::new());
    }

    let last_played = read_last_played(&root);
    let entries = fs::read_dir(&versions_dir).map_err(|error| {
        format!(
            "Cannot read versions directory ({}): {error}",
            versions_dir.display()
        )
    })?;
    let mut instances = Vec::new();
    for entry in entries.flatten() {
        let is_dir = entry
            .file_type()
            .map(|file_type| file_type.is_dir())
            .unwrap_or(false);
        if !is_dir {
            continue;
        }
        let id = entry.file_name().to_string_lossy().to_string();
        if !component_is_safe(&id) {
            continue;
        }
        let json_path = entry.path().join(format!("{id}.json"));
        let raw = match load_version_json(&json_path) {
            Ok(value) => value,
            Err(_) => continue,
        };
        let resolved = resolve_version(&root, &id).ok();
        let metadata = resolved
            .as_ref()
            .map(|value| &value.metadata)
            .unwrap_or(&raw);
        let jar_id = resolved
            .as_ref()
            .map(|value| value.jar_id.clone())
            .unwrap_or_else(|| id.clone());
        let has_client_jar = client_jar_path(&root, &jar_id).is_file()
            || entry.path().join(format!("{id}.jar")).is_file();
        let game_version = resolved
            .as_ref()
            .and_then(|value| value.chain.last())
            .cloned()
            .unwrap_or_else(|| id.clone());
        let name = raw
            .get("name")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .unwrap_or(&id)
            .to_string();
        let last = last_played
            .get(&id)
            .cloned()
            .or_else(|| {
                raw.get("lastPlayed")
                    .and_then(Value::as_str)
                    .map(|value| value.to_string())
            })
            .or_else(|| {
                raw.get("lastUsed")
                    .and_then(Value::as_str)
                    .map(|value| value.to_string())
            });
        instances.push(InstanceInfo {
            id: id.clone(),
            name,
            version_type: metadata_version_type(metadata),
            loader: detect_loader(&id, Some(metadata)),
            game_version,
            path: entry.path().display().to_string(),
            has_client_jar,
            last_played: last,
        });
    }

    instances.sort_by(|left, right| {
        left.name
            .to_ascii_lowercase()
            .cmp(&right.name.to_ascii_lowercase())
    });
    Ok(instances)
}

fn read_last_played(root: &Path) -> HashMap<String, String> {
    let mut result = HashMap::new();
    let path = root.join("launcher_profiles.json");
    let Ok(text) = fs::read_to_string(path) else {
        return result;
    };
    let Ok(value) = serde_json::from_str::<Value>(&text) else {
        return result;
    };
    let Some(profiles) = value.get("profiles").and_then(Value::as_object) else {
        return result;
    };
    for (profile_id, profile) in profiles {
        let Some(last_used) = profile.get("lastUsed").and_then(Value::as_str) else {
            continue;
        };
        if let Some(version_id) = profile.get("lastVersionId").and_then(Value::as_str) {
            result.insert(version_id.to_string(), last_used.to_string());
        }
        if component_is_safe(profile_id) {
            result
                .entry(profile_id.clone())
                .or_insert_with(|| last_used.to_string());
        }
    }
    result
}

fn extract_windows_version(text: &str) -> Option<String> {
    let mut best: Option<String> = None;
    let mut current = String::new();
    for character in text.chars().chain(std::iter::once(' ')) {
        if character.is_ascii_digit() || character == '.' {
            current.push(character);
        } else {
            if current.matches('.').count() >= 2
                && current.starts_with(|value: char| value.is_ascii_digit())
            {
                if best
                    .as_ref()
                    .map(|value| current.len() > value.len())
                    .unwrap_or(true)
                {
                    best = Some(current.clone());
                }
            }
            current.clear();
        }
    }
    best
}

fn detect_os_version(os_name: &str) -> String {
    if os_name == "windows" {
        let mut command = Command::new("cmd");
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        let output = command.args(["/C", "ver"]).output();
        let Ok(output) = output else {
            return String::new();
        };
        let text = String::from_utf8_lossy(&output.stdout);
        extract_windows_version(&text).unwrap_or_default()
    } else if os_name == "macos" {
        Command::new("sw_vers")
            .arg("-productVersion")
            .output()
            .ok()
            .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
            .unwrap_or_default()
    } else {
        let os_release = fs::read_to_string("/etc/os-release").unwrap_or_default();
        for line in os_release.lines() {
            if let Some(value) = line.strip_prefix("VERSION_ID=") {
                return value.trim_matches('"').trim().to_string();
            }
        }
        String::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context() -> Platform {
        let mut features = HashMap::new();
        features.insert("has_custom_resolution".to_string(), true);
        Platform {
            os_name: "windows".to_string(),
            os_version: "10.0.19045".to_string(),
            arch: "x86_64".to_string(),
            features,
        }
    }

    #[test]
    fn inheritance_merge_appends_arrays_and_child_overrides_scalars() {
        let parent = serde_json::json!({
            "id": "parent",
            "mainClass": "old.Main",
            "libraries": [{"name": "a:b:1"}],
            "arguments": {"game": ["--parent"], "jvm": ["-Xparent"]},
            "type": "release"
        });
        let child = serde_json::json!({
            "id": "child",
            "mainClass": "new.Main",
            "libraries": [{"name": "c:d:2"}],
            "arguments": {"game": ["--child"]}
        });
        let merged = merge_version_json(parent, child);
        assert_eq!(merged["id"], "child");
        assert_eq!(merged["mainClass"], "new.Main");
        assert_eq!(merged["type"], "release");
        assert_eq!(merged["libraries"].as_array().expect("libraries").len(), 2);
        assert_eq!(
            merged["arguments"]["game"].as_array().expect("game").len(),
            2
        );
        assert_eq!(merged["arguments"]["jvm"][0], "-Xparent");
    }

    #[test]
    fn platform_rules_match_os_arch_version_and_features() {
        let platform = context();
        let context = platform.rule_context();
        assert!(rules_allow(None, &context));
        assert!(rules_allow(
            Some(&serde_json::json!([{"action": "allow", "os": {"name": "windows"}}])),
            &context
        ));
        assert!(!rules_allow(
            Some(&serde_json::json!([{"action": "allow", "os": {"name": "osx"}}])),
            &context
        ));
        assert!(!rules_allow(
            Some(&serde_json::json!([
                {"action": "allow", "os": {"name": "windows"}},
                {"action": "disallow", "os": {"arch": "x86_64"}}
            ])),
            &context
        ));
        assert!(rules_allow(
            Some(&serde_json::json!([{
                "action": "allow",
                "os": {"name": "windows", "version": "^10\\."},
                "features": {"has_custom_resolution": true}
            }])),
            &context
        ));
        assert!(!rules_allow(
            Some(&serde_json::json!([{
                "action": "allow",
                "features": {"is_demo_user": true}
            }])),
            &context
        ));
    }

    #[test]
    fn maven_coordinates_resolve_to_safe_paths() {
        assert_eq!(
            maven_artifact_path("org.example:demo:1.2.3", None).expect("path"),
            Some("org/example/demo/1.2.3/demo-1.2.3.jar".to_string())
        );
        assert_eq!(
            maven_artifact_path("org.example:demo:1.2.3:natives-windows@zip", None).expect("path"),
            Some("org/example/demo/1.2.3/demo-1.2.3-natives-windows.zip".to_string())
        );
        assert!(maven_artifact_path("org.example:demo:../escape", None).is_err());
    }

    #[test]
    fn native_classifier_substitutes_architecture() {
        let platform = context();
        let context = platform.rule_context();
        let library = serde_json::json!({
            "name": "org.example:native:1",
            "downloads": {"classifiers": {"natives-windows-64": {"path": "org/example/native/1/native-1-natives-windows-64.jar"}}},
            "natives": {"windows": "natives-windows-${arch}"}
        });
        assert_eq!(
            native_classifier(&library, &context),
            Some("natives-windows-64".to_string())
        );
        let file = library_native_file(&library, &context)
            .expect("native descriptor")
            .expect("native file");
        assert_eq!(
            file.1.path,
            "org/example/native/1/native-1-natives-windows-64.jar"
        );
    }

    #[test]
    fn classpath_deduplication_preserves_order() {
        let entries = vec![
            "a.jar".to_string(),
            "b.jar".to_string(),
            "a.jar".to_string(),
            "".to_string(),
        ];
        assert_eq!(dedupe_classpath(entries), vec!["a.jar", "b.jar"]);
    }

    #[test]
    fn loader_detection_uses_id_and_metadata() {
        assert_eq!(detect_loader("1.20.1-forge-47.2.0", None), "Forge");
        assert_eq!(detect_loader("neoforge-21.1.0", None), "NeoForge");
        assert_eq!(detect_loader("fabric-loader-0.15.7-1.20.1", None), "Fabric");
        assert_eq!(detect_loader("quilt-loader-0.24.0", None), "Quilt");
        assert_eq!(detect_loader("1.20.1", None), "Vanilla");
        let metadata =
            serde_json::json!({"mainClass": "net.fabricmc.loader.impl.launch.knot.KnotClient"});
        assert_eq!(detect_loader("custom", Some(&metadata)), "Fabric");
    }

    #[test]
    fn version_ids_reject_path_traversal() {
        assert!(validate_version_id("1.20.1").is_ok());
        assert!(validate_version_id("1.20.1-forge-47.2.0").is_ok());
        assert!(validate_version_id("../escape").is_err());
        assert!(validate_version_id("a/b").is_err());
    }

    #[test]
    fn windows_version_is_extracted_from_localized_ver_output() {
        assert_eq!(
            extract_windows_version("Windows build [10.0.26200.8655]"),
            Some("10.0.26200.8655".to_string())
        );
        assert_eq!(
            extract_windows_version("Microsoft Windows [Version 10.0.19045.4046]"),
            Some("10.0.19045.4046".to_string())
        );
    }

    #[test]
    fn extracts_asset_index_and_required_java_major() {
        let metadata = serde_json::json!({
            "assetIndex": {"id": "17"},
            "javaVersion": {"majorVersion": 21}
        });
        assert_eq!(asset_index_id(&metadata), Some("17".to_string()));
        assert_eq!(required_java_major(&metadata), Some(21));
    }
}

#[cfg(test)]
mod additional_tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn instance_discovery_reports_the_inherited_game_version() {
        let root = std::env::temp_dir().join(format!(
            "flint-instance-version-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or_default()
        ));
        let base_dir = root.join("versions").join("1.20.1");
        let child_id = "1.20.1-fabric-loader-0.15.11";
        let child_dir = root.join("versions").join(child_id);
        fs::create_dir_all(&base_dir).expect("base dir");
        fs::create_dir_all(&child_dir).expect("child dir");
        fs::write(
            base_dir.join("1.20.1.json"),
            br#"{"id":"1.20.1","type":"release","mainClass":"net.minecraft.client.main.Main","libraries":[]}"#,
        )
        .expect("base metadata");
        fs::write(
            child_dir.join(format!("{child_id}.json")),
            br#"{"id":"1.20.1-fabric-loader-0.15.11","inheritsFrom":"1.20.1","mainClass":"net.fabricmc.loader.impl.launch.knot.KnotClient","libraries":[]}"#,
        )
        .expect("child metadata");

        let instances = list_instances(&root.display().to_string()).expect("instances");
        let child = instances
            .iter()
            .find(|instance| instance.id == child_id)
            .expect("child instance");
        assert_eq!(child.game_version, "1.20.1");
        assert_eq!(child.loader, "Fabric");
        let _ = fs::remove_dir_all(root);
    }
}
