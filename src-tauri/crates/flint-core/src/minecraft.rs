//! Generic Minecraft version, profile, library, installer, and workspace helpers.
//!
//! This module contains capability-neutral support for Minecraft providers. It
//! has no source-specific identifiers, hosted event names, or host-framework
//! dependencies.

use crate::{
    net,
    paths,
    ports::{
        Cancellation, Deadline, DownloadRequest, JavaRunRequest, JavaRunner, LoggerPort,
        NetworkPort, NetworkRequest, OutputLimits, PathPort, ProcessOutput, ProgressSink,
        ProgressUpdate, SafeWorkspace, ServiceLabel,
    },
};
use regex::Regex;
use serde_json::{json, Value};
use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
    env,
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};

pub type JsonValue = Value;
pub const MAX_PROFILE_BYTES: u64 = 16 * 1024 * 1024;
pub const MAX_JSON_BYTES: u64 = MAX_PROFILE_BYTES;
pub const MAX_VERSION_JSON_BYTES: u64 = MAX_PROFILE_BYTES;
pub const MAX_HTML_BYTES: u64 = 8 * 1024 * 1024;
pub const MAX_LIBRARY_DOWNLOAD_BYTES: u64 = 128 * 1024 * 1024;
pub const MAX_INSTALLER_BYTES: u64 = 128 * 1024 * 1024;
pub const MAX_INSTALLER_LOG_BYTES: usize = 256 * 1024;
pub const METADATA_HTTP_TIMEOUT: Duration = Duration::from_secs(75);
pub const INSTALLER_HTTP_TIMEOUT: Duration = Duration::from_secs(600);
pub const JAVA_PROCESS_TIMEOUT: Duration = Duration::from_secs(600);
pub const LOADER_METADATA_HTTP_TIMEOUT_SECONDS: u64 = 75;
pub const LOADER_INSTALLER_HTTP_TIMEOUT_SECONDS: u64 = 600;
pub const INSTALLER_TIMEOUT_SECONDS: u64 = 600;

pub fn parse_json(bytes: &[u8]) -> Result<JsonValue, String> {
    serde_json::from_slice(bytes).map_err(|error| error.to_string())
}

pub fn validate_game_version(game_version: &str) -> Result<(), String> {
    if paths::component_is_safe(game_version) {
        Ok(())
    } else {
        Err("The Minecraft version id is invalid.".to_string())
    }
}

pub fn validate_version_id(version_id: &str) -> Result<(), String> {
    if paths::component_is_safe(version_id) {
        Ok(())
    } else {
        Err("Invalid version id.".to_string())
    }
}

pub fn version_dir(root: &Path, version_id: &str) -> Result<PathBuf, String> {
    validate_version_id(version_id)?;
    Ok(root.join("versions").join(version_id))
}

pub fn version_profile_path(root: &Path, version_id: &str) -> Result<PathBuf, String> {
    Ok(version_dir(root, version_id)?.join(format!("{version_id}.json")))
}

pub fn version_json_path(root: &Path, version_id: &str) -> Result<PathBuf, String> {
    version_profile_path(root, version_id)
}

pub fn client_jar_path(root: &Path, version_id: &str) -> Result<PathBuf, String> {
    Ok(version_dir(root, version_id)?.join(format!("{version_id}.jar")))
}

pub fn ensure_launcher_profiles(root: &Path, workspace: &dyn PathPort) -> Result<(), String> {
    ScopedWorkspace::new(root, workspace).ensure_launcher_profiles()
}

pub fn write_version_profile(
    root: &Path,
    workspace: &dyn PathPort,
    version_id: &str,
    profile: &JsonValue,
) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(profile)
        .map_err(|error| format!("Cannot serialize loader profile: {error}"))?;
    ScopedWorkspace::new(root, workspace).write_profile_atomic(version_id, &bytes)
}

pub fn read_version_profile(
    root: &Path,
    workspace: &dyn PathPort,
    version_id: &str,
) -> Result<JsonValue, String> {
    ScopedWorkspace::new(root, workspace).read_version_profile(version_id)
}

pub fn discover_installed_profiles(
    root: &Path,
    workspace: &dyn PathPort,
) -> Result<Vec<InstalledProfile>, String> {
    ScopedWorkspace::new(root, workspace).list_installed_profiles()
}

pub fn find_installed_profile(
    root: &Path,
    workspace: &dyn PathPort,
    expected_id: Option<&str>,
    expected_inherits_from: Option<&str>,
) -> Result<Option<String>, String> {
    ScopedWorkspace::new(root, workspace).find_installed_profile(
        expected_id,
        expected_inherits_from,
    )
}

#[derive(Clone)]
pub struct ScopedWorkspace<'a> {
    root: &'a Path,
    port: &'a dyn PathPort,
}

impl<'a> ScopedWorkspace<'a> {
    pub fn new(root: &'a Path, port: &'a dyn PathPort) -> Self {
        Self { root, port }
    }

    pub fn root(&self) -> &'a Path {
        self.root
    }

    pub fn join(&self, relative: &str) -> Result<PathBuf, String> {
        self.port.safe_join(self.root, relative)
    }

    pub fn safe_join(&self, relative: &str) -> Result<PathBuf, String> {
        self.join(relative)
    }

    pub fn ensure_dir(&self, relative: &str) -> Result<(), String> {
        let path = self.join(relative)?;
        self.port.ensure_directory(&path)
    }

    pub fn ensure_directory(&self, relative: &str) -> Result<(), String> {
        self.ensure_dir(relative)
    }

    pub fn read_file(&self, relative: &str) -> Result<Vec<u8>, String> {
        let path = self.join(relative)?;
        self.port.read_file(&path)
    }

    pub fn read_file_prefix(&self, relative: &str, max_bytes: usize) -> Result<Vec<u8>, String> {
        let path = self.join(relative)?;
        self.port.read_file_prefix(&path, max_bytes)
    }

    pub fn write_file(&self, relative: &str, bytes: &[u8]) -> Result<(), String> {
        let path = self.join(relative)?;
        self.port.write_file(&path, bytes)
    }

    pub fn write_atomic(&self, relative: &str, bytes: &[u8]) -> Result<(), String> {
        let path = self.join(relative)?;
        self.port.write_atomic(&path, bytes)
    }

    pub fn list(&self, relative: &str) -> Result<Vec<PathBuf>, String> {
        let path = self.join(relative)?;
        self.port.list_directory(&path)
    }

    pub fn list_directory(&self, relative: &str) -> Result<Vec<PathBuf>, String> {
        self.list(relative)
    }

    pub fn read(&self, relative: &str) -> Result<Vec<u8>, String> {
        self.read_file(relative)
    }

    pub fn write(&self, relative: &str, bytes: &[u8]) -> Result<(), String> {
        self.write_file(relative, bytes)
    }

    pub fn rename(&self, from: &str, to: &str) -> Result<(), String> {
        let from = self.join(from)?;
        let to = self.join(to)?;
        self.port.rename(&from, &to)
    }

    pub fn remove_file(&self, relative: &str) -> Result<(), String> {
        let path = self.join(relative)?;
        self.port.remove_file(&path)
    }

    pub fn remove_directory(&self, relative: &str) -> Result<(), String> {
        let path = self.join(relative)?;
        self.port.remove_directory(&path)
    }

    pub fn is_regular_file(&self, relative: &str) -> bool {
        self.join(relative)
            .map(|path| self.port.is_regular_file(&path))
            .unwrap_or(false)
    }

    pub fn is_symlink(&self, relative: &str) -> bool {
        self.join(relative)
            .map(|path| self.port.is_symlink(&path))
            .unwrap_or(false)
    }

    pub fn is_directory(&self, relative: &str) -> bool {
        self.join(relative)
            .map(|path| self.port.is_directory(&path))
            .unwrap_or(false)
    }

    pub fn modified(&self, relative: &str) -> Option<SystemTime> {
        self.join(relative)
            .ok()
            .and_then(|path| self.port.modified(&path))
    }

    pub fn version_profile_relative(&self, version_id: &str) -> Result<String, String> {
        validate_version_id(version_id)?;
        Ok(format!("versions/{version_id}/{version_id}.json"))
    }

    pub fn version_profile_path(&self, version_id: &str) -> Result<PathBuf, String> {
        self.join(&self.version_profile_relative(version_id)?)
    }

    pub fn version_json_path(&self, version_id: &str) -> Result<PathBuf, String> {
        self.version_profile_path(version_id)
    }

    pub fn write_profile_atomic(&self, version_id: &str, bytes: &[u8]) -> Result<(), String> {
        self.write_atomic(&self.version_profile_relative(version_id)?, bytes)
    }

    pub fn write_log_atomic(&self, relative: &str, bytes: &[u8]) -> Result<(), String> {
        self.write_atomic(relative, bytes)
    }

    pub fn version_dir(&self, version_id: &str) -> Result<PathBuf, String> {
        validate_version_id(version_id)?;
        self.join(&format!("versions/{version_id}"))
    }

    pub fn client_jar_path(&self, version_id: &str) -> Result<PathBuf, String> {
        validate_version_id(version_id)?;
        self.join(&format!("versions/{version_id}/{version_id}.jar"))
    }

    pub fn ensure_launcher_profiles(&self) -> Result<(), String> {
        let relative = "launcher_profiles.json";
        if self.is_regular_file(relative) {
            return Ok(());
        }
        if self.is_symlink(relative) || self.is_directory(relative) {
            return Err("Refusing to use a non-regular launcher_profiles.json.".to_string());
        }
        let value = json!({
            "profiles": {},
            "settings": {},
            "version": 3
        });
        let bytes = serde_json::to_vec_pretty(&value)
            .map_err(|error| format!("Cannot serialize launcher_profiles.json: {error}"))?;
        self.write_atomic(relative, &bytes)
            .map_err(|error| format!("Cannot create launcher_profiles.json: {error}"))
    }

    pub fn read_version_profile(&self, version_id: &str) -> Result<Value, String> {
        let path = self.version_profile_path(version_id)?;
        if !self.port.is_regular_file(&path) {
            return Err("Version profile is not a regular file.".to_string());
        }
        let bytes = self
            .port
            .read_file_prefix(&path, (MAX_PROFILE_BYTES as usize).saturating_add(1))?;
        if bytes.len() as u64 > MAX_PROFILE_BYTES {
            return Err("Version profile is unexpectedly large.".to_string());
        }
        let value: Value = serde_json::from_slice(&bytes)
            .map_err(|error| format!("Version profile is invalid JSON: {error}"))?;
        if !value.is_object() {
            return Err("Version profile must be a JSON object.".to_string());
        }
        Ok(value)
    }

    pub fn list_installed_profiles(&self) -> Result<Vec<InstalledProfile>, String> {
        let versions = "versions";
        if !self.is_directory(versions) {
            return Ok(Vec::new());
        }
        let mut profiles = Vec::new();
        for path in self.list(versions)? {
            if !self.port.is_directory(&path) {
                continue;
            }
            let Some(id) = path.file_name().and_then(|value| value.to_str()) else {
                continue;
            };
            if validate_version_id(id).is_err() {
                continue;
            }
            let profile_path = path.join(format!("{id}.json"));
            if !self.port.is_regular_file(&profile_path) {
                continue;
            }
            let bytes = match self
                .port
                .read_file_prefix(&profile_path, (MAX_PROFILE_BYTES as usize).saturating_add(1))
            {
                Ok(bytes) if bytes.len() as u64 <= MAX_PROFILE_BYTES => bytes,
                _ => continue,
            };
            let Ok(value) = serde_json::from_slice::<Value>(&bytes) else {
                continue;
            };
            if !value.is_object() {
                continue;
            }
            let inherits_from = value
                .get("inheritsFrom")
                .and_then(Value::as_str)
                .map(str::to_string);
            profiles.push(InstalledProfile {
                id: id.to_string(),
                inherits_from,
                modified: self.port.modified(&profile_path),
            });
        }
        profiles.sort_by(|left, right| right.modified.cmp(&left.modified));
        Ok(profiles)
    }

    pub fn find_installed_profile(
        &self,
        expected_id: Option<&str>,
        expected_inherits_from: Option<&str>,
    ) -> Result<Option<String>, String> {
        if let Some(expected_id) = expected_id {
            validate_version_id(expected_id)?;
            if self.is_regular_file(&format!(
                "versions/{expected_id}/{expected_id}.json"
            )) {
                return Ok(Some(expected_id.to_string()));
            }
        }
        let profiles = self.list_installed_profiles()?;
        for profile in profiles {
            if expected_inherits_from
                .map(|expected| {
                    profile
                        .inherits_from
                        .as_deref()
                        .map(|actual| actual.eq_ignore_ascii_case(expected))
                        .unwrap_or(false)
                })
                .unwrap_or(true)
            {
                return Ok(Some(profile.id));
            }
        }
        Ok(None)
    }

    pub fn read_jar_prefix(&self, relative: &str) -> Result<Vec<u8>, String> {
        self.read_file_prefix(relative, 4)
    }
}

impl SafeWorkspace {
    pub fn scoped<'a>(&'a self, root: &'a Path) -> ScopedWorkspace<'a> {
        ScopedWorkspace::new(root, self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledProfile {
    pub id: String,
    pub inherits_from: Option<String>,
    pub modified: Option<SystemTime>,
}

#[derive(Debug, Clone)]
pub struct MinecraftPlatform {
    pub os_name: String,
    pub os_version: String,
    pub arch: String,
    pub features: HashMap<String, bool>,
}

impl MinecraftPlatform {
    fn default_features() -> HashMap<String, bool> {
        let mut features = HashMap::new();
        features.insert("is_demo_user".to_string(), false);
        features.insert("has_custom_resolution".to_string(), true);
        features.insert("has_quick_plays_support".to_string(), false);
        features.insert("is_quick_play_singleplayer".to_string(), false);
        features.insert("is_quick_play_multiplayer".to_string(), false);
        features.insert("is_quick_play_realms".to_string(), false);
        features
    }

    pub fn current() -> Self {
        Self {
            os_name: normalize_os(env::consts::OS),
            os_version: String::new(),
            arch: normalize_arch(env::consts::ARCH),
            features: Self::default_features(),
        }
    }

    pub fn new(os_name: impl Into<String>, os_version: impl Into<String>, arch: impl Into<String>) -> Self {
        let os_name = os_name.into();
        let arch = arch.into();
        Self {
            os_name: normalize_os(&os_name),
            os_version: os_version.into(),
            arch: normalize_arch(&arch),
            features: Self::default_features(),
        }
    }

    pub fn rule_context(&self) -> MinecraftRuleContext<'_> {
        MinecraftRuleContext {
            os_name: &self.os_name,
            os_version: &self.os_version,
            arch: &self.arch,
            features: &self.features,
        }
    }
}

pub type Platform = MinecraftPlatform;
pub type RuleContext<'a> = MinecraftRuleContext<'a>;
pub type LibraryFile = MinecraftLibraryFile;

pub struct MinecraftRuleContext<'a> {
    pub os_name: &'a str,
    pub os_version: &'a str,
    pub arch: &'a str,
    pub features: &'a HashMap<String, bool>,
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

pub fn rules_allow(rules: Option<&Value>, context: &MinecraftRuleContext<'_>) -> bool {
    let Some(rules_array) = rules.and_then(Value::as_array) else {
        return rules.is_none();
    };
    let mut allowed = false;
    for rule in rules_array {
        if rule_matches(rule, context) {
            match rule.get("action").and_then(Value::as_str) {
                Some("allow") => allowed = true,
                Some("disallow") => allowed = false,
                _ => {}
            }
        }
    }
    allowed
}

fn rule_matches(rule: &Value, context: &MinecraftRuleContext<'_>) -> bool {
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
            if context.features.get(name).copied().unwrap_or(false) != expected {
                return false;
            }
        }
    }
    true
}

pub fn library_allowed(library: &Value, context: &MinecraftRuleContext<'_>) -> bool {
    rules_allow(library.get("rules"), context)
}

pub fn allowed_libraries(metadata: &Value, context: &MinecraftRuleContext<'_>) -> Vec<Value> {
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
        (Value::Object(mut parent), Value::Object(child)) => {
            for (key, child_value) in child {
                if let Some(parent_value) = parent.remove(&key) {
                    parent.insert(key, merge_value(parent_value, child_value));
                } else {
                    parent.insert(key, child_value);
                }
            }
            Value::Object(parent)
        }
        (Value::Array(mut parent), Value::Array(child)) => {
            parent.extend(child);
            Value::Array(parent)
        }
        (_, child) => child,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MinecraftLibraryFile {
    pub path: String,
    pub url: Option<String>,
    pub sha1: Option<String>,
    pub size: Option<u64>,
}

pub fn maven_artifact_path(name: &str, classifier_override: Option<&str>) -> Result<Option<String>, String> {
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
    let classifier_suffix = classifier.map(|value| format!("-{value}")).unwrap_or_default();
    let relative = format!(
        "{}/{}/{}/{artifact}-{version}{classifier_suffix}.{extension}",
        group.replace('.', "/"),
        artifact,
        version
    );
    paths::normalize_relative_metadata_path(&relative)
        .map(Some)
        .ok_or_else(|| format!("Unsafe Maven artifact path derived from: {name}"))
}

pub fn library_file(library: &Value, classifier: Option<&str>) -> Result<Option<MinecraftLibraryFile>, String> {
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
        .map(str::to_string);
    let maven_path = library
        .get("name")
        .and_then(Value::as_str)
        .map(|name| maven_artifact_path(name, classifier))
        .transpose()?
        .flatten();
    let path = match metadata_path.or(maven_path) {
        Some(path) => paths::normalize_relative_metadata_path(&path)
            .ok_or_else(|| format!("Unsafe library path: {path}"))?,
        None => return Ok(None),
    };
    let explicit_url = download
        .and_then(|value| value.get("url"))
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
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
    Ok(Some(MinecraftLibraryFile { path, url, sha1, size }))
}

pub fn native_classifier(library: &Value, context: &MinecraftRuleContext<'_>) -> Option<String> {
    let natives = library.get("natives")?.as_object()?;
    let candidates: &[&str] = match context.os_name {
        "macos" => &["osx", "macos"],
        "windows" => &["windows"],
        "linux" => &["linux"],
        _ => &[],
    };
    for key in candidates {
        if let Some(value) = natives.get(*key).and_then(Value::as_str) {
            let arch = if context.arch == "x86" || context.arch == "arm" { "32" } else { "64" };
            return Some(value.replace("${arch}", arch));
        }
    }
    None
}

pub fn library_native_file(
    library: &Value,
    context: &MinecraftRuleContext<'_>,
) -> Result<Option<(String, MinecraftLibraryFile)>, String> {
    let Some(classifier) = native_classifier(library, context) else {
        return Ok(None);
    };
    match library_file(library, Some(&classifier))? {
        Some(file) => Ok(Some((classifier, file))),
        None => Ok(None),
    }
}

pub fn force_https(url: &str) -> String {
    force_https_url(url)
}

pub fn force_https_url(url: &str) -> String {
    url.strip_prefix("http://")
        .map(|rest| format!("https://{rest}"))
        .unwrap_or_else(|| url.to_string())
}

pub fn upgrade_library_url(library: &mut Value) {
    if let Some(object) = library.as_object_mut() {
        if object.get("url").is_none() {
            object.insert("url".to_string(), json!("https://repo.maven.apache.org/maven2/"));
        } else if let Some(url) = object.get("url").and_then(Value::as_str) {
            let upgraded = force_https_url(url);
            if upgraded != url {
                object.insert("url".to_string(), json!(upgraded));
            }
        }
    }
}

pub fn parse_xml_versions(text: &str) -> Vec<String> {
    let Ok(regex) = Regex::new(r"(?s)<version>\s*([^<]+?)\s*</version>") else {
        return Vec::new();
    };
    let mut seen = HashSet::new();
    regex
        .captures_iter(text)
        .filter_map(|capture| capture.get(1).map(|value| value.as_str().trim().to_string()))
        .filter(|value| !value.is_empty() && seen.insert(value.clone()))
        .collect()
}

pub fn compare_versions_desc(left: &str, right: &str) -> Ordering {
    compare_loader_versions_desc(left, right)
}

pub fn compare_loader_versions_desc(left: &str, right: &str) -> Ordering {
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
        if ordering != Ordering::Equal {
            return ordering;
        }
    }
    right_parts.len().cmp(&left_parts.len())
}

pub fn sort_loader_versions_desc<T, F>(values: &mut [T], key: F)
where
    F: for<'a> Fn(&'a T) -> &'a str,
{
    values.sort_by(|left, right| compare_loader_versions_desc(key(left), key(right)));
}

pub fn check_control(deadline: &Deadline, cancellation: &Cancellation) -> Result<(), String> {
    deadline.check()?;
    cancellation.check()
}

pub fn report_progress(
    progress: &dyn ProgressSink,
    completed: u64,
    total: Option<u64>,
    message: impl Into<String>,
) {
    progress.report(ProgressUpdate::new(
        completed,
        total,
        Some(message.into()),
    ));
}

pub fn log_info(logger: &dyn LoggerPort, message: &str) {
    logger.info(message);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkOperation {
    Request,
    Download,
}

/// Shared translation of typed-network errors into stable user-facing loader
/// messages. Providers must route network failures through this helper so
/// status, transport, body-limit, and download failures stay consistent.
pub fn translate_network_error(
    service: &str,
    operation: NetworkOperation,
    error: impl std::fmt::Display,
) -> String {
    let sanitized = crate::ports::network::sanitize_network_error(error);
    let detail = strip_network_service_prefix(service, &sanitized);
    if is_network_body_error(&detail) {
        return detail;
    }
    if detail.starts_with("Refusing to use a non-HTTPS URL.") {
        return detail;
    }
    let Some(status) = network_http_status(&detail) else {
        if operation == NetworkOperation::Download
            && (detail.starts_with("Download failed for ")
                || detail.starts_with("Invalid SHA-1 supplied for "))
        {
            return detail;
        }
        return format!("Cannot contact {service}: {detail}");
    };
    match status {
        401 | 403 => format!(
            "{service} rejected the request. Check access permissions or try another version."
        ),
        404 => format!("{service} did not find the requested resource."),
        _ => format!(
            "{service} request failed with HTTP {status}. Check connectivity or try another version."
        ),
    }
}

fn strip_network_service_prefix(service: &str, error: &str) -> String {
    let prefixes = [
        format!("{service} request failed: "),
        format!("{service} request rejected: "),
        format!("{service} download setup failed: "),
        format!("{service}: "),
    ];
    for prefix in prefixes {
        if let Some(detail) = error.strip_prefix(prefix.as_str()) {
            return detail.to_string();
        }
    }
    error.to_string()
}

fn is_network_body_error(error: &str) -> bool {
    error.contains("HTTP response is too large (")
        || error.contains("HTTP response exceeds the ")
        || error.contains("Cannot read HTTP response:")
}

fn network_http_status(error: &str) -> Option<u16> {
    let tail = &error[error.find("HTTP status")?..];
    let open = tail.find('(')?;
    let code = tail[open + 1..]
        .chars()
        .take_while(|character| character.is_ascii_digit())
        .collect::<String>();
    if code.len() == 3 {
        code.parse().ok()
    } else {
        None
    }
}

pub fn fetch_json(
    network: &dyn NetworkPort,
    url: &str,
    service: &str,
    max_bytes: u64,
    timeout: Duration,
    deadline: &Deadline,
    cancellation: &Cancellation,
) -> Result<Value, String> {
    check_control(deadline, cancellation)?;
    let label = ServiceLabel::new(service)
        .map_err(|error| format!("Invalid metadata service label: {error}"))?;
    let request = NetworkRequest::new(url, label, max_bytes, timeout);
    let bytes = network
        .read_bytes(&request)
        .map_err(|error| translate_network_error(service, NetworkOperation::Request, error))?;
    serde_json::from_slice(&bytes)
        .map_err(|error| format!("{service} returned invalid JSON: {error}"))
}

pub fn fetch_text(
    network: &dyn NetworkPort,
    url: &str,
    service: &str,
    max_bytes: u64,
    timeout: Duration,
    deadline: &Deadline,
    cancellation: &Cancellation,
) -> Result<String, String> {
    check_control(deadline, cancellation)?;
    let label = ServiceLabel::new(service)
        .map_err(|error| format!("Invalid metadata service label: {error}"))?;
    let request = NetworkRequest::new(url, label, max_bytes, timeout);
    let bytes = network
        .read_bytes(&request)
        .map_err(|error| translate_network_error(service, NetworkOperation::Request, error))?;
    Ok(String::from_utf8_lossy(&bytes).to_string())
}

pub struct MetaProfileContext<'a> {
    root: &'a Path,
    network: &'a dyn NetworkPort,
    workspace: &'a dyn PathPort,
    progress: &'a dyn ProgressSink,
    logger: &'a dyn LoggerPort,
    platform: &'a MinecraftPlatform,
    deadline: Deadline,
    cancellation: Cancellation,
}

impl<'a> MetaProfileContext<'a> {
    pub fn new(
        root: &'a Path,
        network: &'a dyn NetworkPort,
        workspace: &'a dyn PathPort,
        progress: &'a dyn ProgressSink,
        logger: &'a dyn LoggerPort,
        platform: &'a MinecraftPlatform,
        deadline: Deadline,
        cancellation: Cancellation,
    ) -> Self {
        Self {
            root,
            network,
            workspace,
            progress,
            logger,
            platform,
            deadline,
            cancellation,
        }
    }

    pub fn root(&self) -> &'a Path { self.root }
    pub fn network(&self) -> &'a dyn NetworkPort { self.network }
    pub fn workspace(&self) -> &'a dyn PathPort { self.workspace }
    pub fn progress(&self) -> &'a dyn ProgressSink { self.progress }
    pub fn logger(&self) -> &'a dyn LoggerPort { self.logger }
    pub fn platform(&self) -> &'a MinecraftPlatform { self.platform }
    pub fn deadline(&self) -> &Deadline { &self.deadline }
    pub fn cancellation(&self) -> &Cancellation { &self.cancellation }
    pub fn check_control(&self) -> Result<(), String> { check_control(&self.deadline, &self.cancellation) }
}

#[derive(Debug, Clone)]
pub struct MetaProfileRequest<'a> {
    pub profile_url: &'a str,
    pub service_name: &'a str,
    pub game_version: &'a str,
    pub profile_max_bytes: u64,
    pub library_max_bytes: u64,
    pub metadata_timeout: Duration,
}

impl<'a> MetaProfileRequest<'a> {
    pub fn new(profile_url: &'a str, service_name: &'a str, game_version: &'a str) -> Self {
        Self {
            profile_url,
            service_name,
            game_version,
            profile_max_bytes: MAX_PROFILE_BYTES,
            library_max_bytes: MAX_LIBRARY_DOWNLOAD_BYTES,
            metadata_timeout: METADATA_HTTP_TIMEOUT,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetaProfileOutput {
    pub installed_version_id: String,
    pub libraries_downloaded: u64,
}

pub fn install_meta_profile(
    context: &MetaProfileContext<'_>,
    request: &MetaProfileRequest<'_>,
) -> Result<MetaProfileOutput, String> {
    validate_game_version(request.game_version)?;
    context.check_control()?;
    report_progress(context.progress(), 0, Some(1), format!("Fetching {} profile", request.service_name));
    log_info(context.logger(), &format!("Fetching {} profile", request.service_name));
    let profile_url = force_https_url(request.profile_url);
    let profile = fetch_json(
        context.network(),
        &profile_url,
        request.service_name,
        request.profile_max_bytes,
        request.metadata_timeout,
        context.deadline(),
        context.cancellation(),
    )?;
    context.check_control()?;
    let installed_id = profile
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{} profile does not declare an id.", request.service_name))?;
    validate_version_id(installed_id)?;
    let inherits = profile
        .get("inheritsFrom")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{} profile does not declare inheritsFrom.", request.service_name))?;
    if inherits != request.game_version {
        return Err(format!(
            "{} returned a profile for {inherits}, not {}.",
            request.service_name, request.game_version
        ));
    }
    report_progress(context.progress(), 1, Some(1), format!("{} profile downloaded", request.service_name));

    let rule_context = context.platform().rule_context();
    let libraries = allowed_libraries(&profile, &rule_context);
    let mut tasks = Vec::new();
    for library in &libraries {
        if let Some(file) = library_file(library, None)? {
            tasks.push(file);
        }
        if let Some((_classifier, file)) = library_native_file(library, &rule_context)? {
            tasks.push(file);
        }
    }
    let total = tasks.len() as u64;
    report_progress(context.progress(), 0, Some(total), format!("Downloading {} libraries", request.service_name));
    let workspace = ScopedWorkspace::new(context.root(), context.workspace());
    workspace.ensure_dir("libraries")?;
    let mut seen = HashSet::new();
    let mut downloaded = 0u64;
    for (index, file) in tasks.into_iter().enumerate() {
        context.check_control()?;
        let key = file.path.to_ascii_lowercase();
        if !seen.insert(key) {
            continue;
        }
        let Some(url) = file.url.as_deref() else {
            return Err(format!("Library has no download URL: {}", file.path));
        };
        let url = force_https_url(url);
        let destination = workspace.join(&format!("libraries/{}", file.path))?;
        let download_service = "Minecraft library";
        let label = ServiceLabel::new(download_service)
            .map_err(|error| format!("Invalid library service label: {error}"))?;
        let download = DownloadRequest::new(
            url,
            destination,
            file.sha1,
            label,
            request.library_max_bytes,
            request.metadata_timeout,
        );
        context
            .network()
            .download_atomic(&download)
            .map_err(|error| {
                translate_network_error(
                    download_service,
                    NetworkOperation::Download,
                    error,
                )
            })?;
        downloaded += 1;
        report_progress(
            context.progress(),
            index as u64 + 1,
            Some(total),
            format!("Downloaded library {}", file.path),
        );
    }
    context.check_control()?;
    report_progress(
        context.progress(),
        0,
        Some(1),
        "Writing loader version profile",
    );
    write_version_profile(context.root(), context.workspace(), installed_id, &profile)?;
    report_progress(
        context.progress(),
        1,
        Some(1),
        "Loader version profile written",
    );
    report_progress(
        context.progress(),
        1,
        Some(1),
        format!("{} installation complete", request.service_name),
    );
    Ok(MetaProfileOutput {
        installed_version_id: installed_id.to_string(),
        libraries_downloaded: downloaded,
    })
}

pub struct InstallerContext<'a> {
    root: &'a Path,
    network: &'a dyn NetworkPort,
    workspace: &'a dyn PathPort,
    java: &'a dyn JavaRunner,
    progress: &'a dyn ProgressSink,
    logger: &'a dyn LoggerPort,
    deadline: Deadline,
    cancellation: Cancellation,
}

impl<'a> InstallerContext<'a> {
    pub fn new(
        root: &'a Path,
        network: &'a dyn NetworkPort,
        workspace: &'a dyn PathPort,
        java: &'a dyn JavaRunner,
        progress: &'a dyn ProgressSink,
        logger: &'a dyn LoggerPort,
        deadline: Deadline,
        cancellation: Cancellation,
    ) -> Self {
        Self { root, network, workspace, java, progress, logger, deadline, cancellation }
    }

    pub fn root(&self) -> &'a Path { self.root }
    pub fn network(&self) -> &'a dyn NetworkPort { self.network }
    pub fn workspace(&self) -> &'a dyn PathPort { self.workspace }
    pub fn java(&self) -> &'a dyn JavaRunner { self.java }
    pub fn progress(&self) -> &'a dyn ProgressSink { self.progress }
    pub fn logger(&self) -> &'a dyn LoggerPort { self.logger }
    pub fn deadline(&self) -> &Deadline { &self.deadline }
    pub fn cancellation(&self) -> &Cancellation { &self.cancellation }
    pub fn check_control(&self) -> Result<(), String> { check_control(&self.deadline, &self.cancellation) }
}

#[derive(Debug, Clone)]
pub struct InstallerRequest<'a> {
    pub installer_url: &'a str,
    pub installer_file_name: &'a str,
    pub installer_args: Vec<String>,
    pub service_name: &'a str,
    pub log_label: &'a str,
    pub game_version: &'a str,
    pub java_executable: &'a Path,
    pub expected_profile_id: Option<&'a str>,
    pub installer_max_bytes: u64,
    pub installer_timeout: Duration,
    pub java_timeout: Duration,
    pub output_limits: OutputLimits,
}

impl<'a> InstallerRequest<'a> {
    pub fn new(
        installer_url: &'a str,
        installer_file_name: &'a str,
        installer_args: Vec<String>,
        service_name: &'a str,
        log_label: &'a str,
        game_version: &'a str,
        java_executable: &'a Path,
    ) -> Self {
        Self {
            installer_url,
            installer_file_name,
            installer_args,
            service_name,
            log_label,
            game_version,
            java_executable,
            expected_profile_id: None,
            installer_max_bytes: MAX_INSTALLER_BYTES,
            installer_timeout: INSTALLER_HTTP_TIMEOUT,
            java_timeout: JAVA_PROCESS_TIMEOUT,
            output_limits: OutputLimits::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallerOutput {
    pub installed_version_id: String,
    pub log_path: String,
}

pub fn install_java_installer(
    context: &InstallerContext<'_>,
    request: &InstallerRequest<'_>,
) -> Result<InstallerOutput, String> {
    validate_game_version(request.game_version)?;
    if !paths::component_is_safe(request.installer_file_name) {
        return Err("The installer file name is unsafe.".to_string());
    }
    let installer_url = force_https_url(request.installer_url);
    net::validate_https_url(&installer_url)?;
    let workspace = ScopedWorkspace::new(context.root(), context.workspace());
    workspace.ensure_dir("loader-installers")?;
    let installer_relative = format!("loader-installers/{}", request.installer_file_name);
    let installer_path = workspace.join(&installer_relative)?;
    context.check_control()?;
    let download_service = format!("{} installer", request.service_name);
    let label = ServiceLabel::new(download_service.clone())
        .map_err(|error| format!("Invalid installer service label: {error}"))?;
    let download = DownloadRequest::new(
        installer_url,
        installer_path.clone(),
        None,
        label,
        request.installer_max_bytes,
        request.installer_timeout,
    );
    report_progress(context.progress(), 0, Some(1), format!("Downloading {} installer", request.service_name));
    log_info(
        context.logger(),
        &format!("Downloading {} installer", request.service_name),
    );
    context
        .network()
        .download_atomic(&download)
        .map_err(|error| {
            translate_network_error(
                &download_service,
                NetworkOperation::Download,
                error,
            )
        })?;
    context.check_control()?;
    let prefix = workspace.read_jar_prefix(&installer_relative)?;
    if !is_jar_zip_prefix(&prefix) {
        let _ = workspace.remove_file(&installer_relative);
        return Err(format!(
            "{} upstream did not return a direct JAR. Download and install it manually, then retry instance discovery.",
            request.service_name
        ));
    }
    workspace.ensure_launcher_profiles()?;
    workspace.ensure_dir("logs")?;
    let log_relative = format!(
        "logs/flint-installer-{}-{}.log",
        sanitize_file_component(request.log_label),
        net::nonce()
    );
    let log_path = workspace.join(&log_relative)?;
    report_progress(context.progress(), 1, Some(2), format!("Running {} installer", request.service_name));
    log_info(
        context.logger(),
        &format!("Running {} installer", request.service_name),
    );
    let mut java_request = JavaRunRequest::new(
        request.java_executable.to_path_buf(),
        "-jar",
        request.java_timeout,
    );
    java_request.current_dir = Some(context.root().to_path_buf());
    java_request.output_limits = request.output_limits;
    java_request
        .program_args
        .push(installer_path.to_string_lossy().to_string());
    java_request.program_args.extend(request.installer_args.iter().cloned());
    context.check_control()?;
    let output = context.java().run_java(java_request)?;
    let log_bytes = installer_log_bytes(&output);
    workspace.write_log_atomic(&log_relative, &log_bytes)?;
    if output.timed_out {
        return Err(format!(
            "Loader installer timed out after {} seconds. See {}.",
            request.java_timeout.as_secs(),
            log_path.display()
        ));
    }
    if !output.succeeded() {
        let status = output
            .status_code
            .map(|status| format!("exit status: {status}"))
            .unwrap_or_else(|| "exit status: unknown".to_string());
        return Err(format!(
            "Loader installer exited with {status}. See {}.",
            log_path.display()
        ));
    }
    context.check_control()?;
    report_progress(context.progress(), 2, Some(2), format!("{} installer finished", request.service_name));
    let installed_version_id = workspace
        .find_installed_profile(
            request.expected_profile_id,
            Some(request.game_version),
        )?
        .ok_or_else(|| {
            format!(
                "{} installer finished, but no matching version profile was found. See {}.",
                request.service_name,
                log_path.display()
            )
        })?;
    Ok(InstallerOutput {
        installed_version_id,
        log_path: log_path.display().to_string(),
    })
}

pub fn is_jar_zip_prefix(bytes: &[u8]) -> bool {
    bytes.starts_with(b"PK\x03\x04")
        || bytes.starts_with(b"PK\x05\x06")
        || bytes.starts_with(b"PK\x07\x08")
}

pub fn is_zip_file(bytes: &[u8]) -> bool {
    is_jar_zip_prefix(bytes)
}

pub fn is_jar_file(bytes: &[u8]) -> bool {
    is_jar_zip_prefix(bytes)
}

/// Redacts URL/query credentials and quoted or unquoted authorization,
/// token, secret, and API-key assignments from installer output while
/// retaining surrounding diagnostic text and key labels.
pub fn redact_installer_log(text: &str) -> String {
    let Ok(url_regex) = Regex::new(r#"(?i)https?://[^\s<>"']+"#) else {
        return "[redacted-log]".to_string();
    };
    let Ok(assignment_regex) = Regex::new(
        r#"(?i)(?P<key_quote>["']?)\b(?P<key>access[_-]?token|refresh[_-]?token|api[_-]?key|authorization|proxy[_-]?authorization|auth[_-]?token|client[_-]?secret|secret[_-]?key|token|password|secret|credential|credentials|x-amz-signature|x-amz-credential|x-amz-security-token|sig(?:nature)?)\b(?P<key_close>["']?)(?P<separator>\s*[:=]\s*)(?P<value>(?:bearer|basic)\s+[^\s,;&]+|"(?:\\.|[^"\\])*"|'(?:\\.|[^'\\])*'|[^\s,;&]+)"#,
    ) else {
        return "[redacted-log]".to_string();
    };
    let Ok(bearer_regex) = Regex::new(r"(?i)\bBearer\s+[^\s,;&]+") else {
        return "[redacted-log]".to_string();
    };
    let Ok(query_regex) = Regex::new(
        r#"(?i)([?&](?:sig(?:nature)?|x-amz-signature|x-amz-credential|x-amz-security-token|token|access[_-]?token|refresh[_-]?token|api[_-]?key|key|expires)=)[^&\s,;]*"#,
    ) else {
        return "[redacted-log]".to_string();
    };

    let mut redacted = url_regex
        .replace_all(text, "[redacted-url]")
        .into_owned();
    redacted = assignment_regex
        .replace_all(&redacted, |captures: &regex::Captures<'_>| {
            let key = captures
                .name("key")
                .map(|capture| capture.as_str())
                .unwrap_or("");
            let key_quote = captures
                .name("key_quote")
                .map(|capture| capture.as_str())
                .unwrap_or("");
            let key_close = captures
                .name("key_close")
                .map(|capture| capture.as_str())
                .unwrap_or("");
            let quote = if key_quote.is_empty() {
                key_close
            } else {
                key_quote
            };
            let separator = captures
                .name("separator")
                .map(|capture| capture.as_str())
                .unwrap_or(":");
            let value_quote = captures
                .name("value")
                .map(|capture| capture.as_str())
                .and_then(|value| {
                    if value.starts_with('"') {
                        Some("\"")
                    } else if value.starts_with('\'') {
                        Some("'")
                    } else {
                        None
                    }
                })
                .unwrap_or("");
            if quote.is_empty() {
                format!("{key}{separator}{value_quote}[redacted]{value_quote}")
            } else {
                format!("{quote}{key}{quote}{separator}{value_quote}[redacted]{value_quote}")
            }
        })
        .into_owned();
    redacted = bearer_regex
        .replace_all(&redacted, "Bearer [redacted]")
        .into_owned();
    query_regex
        .replace_all(&redacted, |captures: &regex::Captures<'_>| {
            format!("{}=[redacted]", &captures[1])
        })
        .into_owned()
}

pub const MAX_PROGRESS_MESSAGE_CHARS: usize = 512;

/// Sanitizes provider-produced progress text before it becomes a user-visible event payload.
///
/// Reuses installer-log credential/URL redaction, normalizes control characters and other
/// whitespace, and bounds the result so provider output cannot inject terminal controls or
/// produce unbounded event payloads.
pub fn sanitize_progress_message(text: &str) -> String {
    let redacted = redact_installer_log(text);
    let normalized = redacted
        .chars()
        .map(|character| {
            if character.is_control() || character.is_whitespace() {
                ' '
            } else {
                character
            }
        })
        .collect::<String>();
    let normalized = normalized.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.chars().count() <= MAX_PROGRESS_MESSAGE_CHARS {
        normalized
    } else {
        let mut bounded = normalized
            .chars()
            .take(MAX_PROGRESS_MESSAGE_CHARS.saturating_sub(3))
            .collect::<String>();
        bounded.push_str("...");
        bounded
    }
}

fn installer_log_bytes(output: &ProcessOutput) -> Vec<u8> {
    let stdout = redact_installer_log(&String::from_utf8_lossy(&output.stdout));
    let stderr = redact_installer_log(&String::from_utf8_lossy(&output.stderr));
    let mut log = format!("stdout:\n{stdout}\nstderr:\n{stderr}\n");
    if log.len() > MAX_INSTALLER_LOG_BYTES {
        let marker = "\n[truncated]\n";
        let target = MAX_INSTALLER_LOG_BYTES.saturating_sub(marker.len());
        let mut end = target.min(log.len());
        while end > 0 && !log.is_char_boundary(end) {
            end -= 1;
        }
        log.truncate(end);
        log.push_str(marker);
    }
    log.into_bytes()
}

pub fn sanitize_file_component(value: &str) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::{DownloadRequest, NetworkPort, NoopLogger, NoopProgress, PathPort, ProcessOutput, ProgressUpdate};
    use std::sync::Mutex;

    struct FakeNetwork {
        response: Vec<u8>,
        downloads: Mutex<Vec<DownloadRequest>>,
    }

    impl FakeNetwork {
        fn new(response: Vec<u8>) -> Self {
            Self {
                response,
                downloads: Mutex::new(Vec::new()),
            }
        }
    }

    impl NetworkPort for FakeNetwork {
        fn read_json(&self, _request: &NetworkRequest) -> Result<Value, String> {
            serde_json::from_slice(&self.response).map_err(|error| error.to_string())
        }

        fn read_text(&self, _request: &NetworkRequest) -> Result<String, String> {
            Ok(String::from_utf8_lossy(&self.response).to_string())
        }

        fn read_bytes(&self, _request: &NetworkRequest) -> Result<Vec<u8>, String> {
            Ok(self.response.clone())
        }

        fn download_atomic(&self, request: &DownloadRequest) -> Result<u64, String> {
            self.downloads.lock().unwrap().push(request.clone());
            Ok(0)
        }
    }

    struct FakeJava {
        output: ProcessOutput,
        requests: Mutex<Vec<JavaRunRequest>>,
    }

    impl FakeJava {
        fn new(output: ProcessOutput) -> Self {
            Self {
                output,
                requests: Mutex::new(Vec::new()),
            }
        }
    }

    impl JavaRunner for FakeJava {
        fn run_java(&self, request: JavaRunRequest) -> Result<ProcessOutput, String> {
            self.requests.lock().unwrap().push(request);
            Ok(self.output.clone())
        }
    }

    struct MemoryWorkspace {
        files: Mutex<HashMap<PathBuf, Vec<u8>>>,
        dirs: Mutex<HashSet<PathBuf>>,
    }

    impl MemoryWorkspace {
        fn new() -> Self {
            Self {
                files: Mutex::new(HashMap::new()),
                dirs: Mutex::new(HashSet::new()),
            }
        }
    }

    impl PathPort for MemoryWorkspace {
        fn safe_join(&self, base: &Path, relative: &str) -> Result<PathBuf, String> {
            paths::safe_join(base, relative)
                .ok_or_else(|| format!("Unsafe relative path: {relative}"))
        }

        fn ensure_directory(&self, path: &Path) -> Result<(), String> {
            self.dirs.lock().unwrap().insert(path.to_path_buf());
            Ok(())
        }

        fn read_file(&self, path: &Path) -> Result<Vec<u8>, String> {
            self.files
                .lock()
                .unwrap()
                .get(path)
                .cloned()
                .ok_or_else(|| format!("Missing file: {}", path.display()))
        }

        fn read_file_prefix(&self, path: &Path, max_bytes: usize) -> Result<Vec<u8>, String> {
            let bytes = self.read_file(path)?;
            Ok(bytes.into_iter().take(max_bytes).collect())
        }

        fn write_file(&self, path: &Path, bytes: &[u8]) -> Result<(), String> {
            self.files.lock().unwrap().insert(path.to_path_buf(), bytes.to_vec());
            Ok(())
        }

        fn write_atomic(&self, destination: &Path, bytes: &[u8]) -> Result<(), String> {
            let mut parent = destination.parent();
            while let Some(path) = parent {
                if !path.as_os_str().is_empty() {
                    self.dirs.lock().unwrap().insert(path.to_path_buf());
                }
                parent = path.parent();
            }
            self.write_file(destination, bytes)
        }

        fn list_directory(&self, path: &Path) -> Result<Vec<PathBuf>, String> {
            let mut paths = Vec::new();
            {
                let dirs = self.dirs.lock().unwrap();
                for candidate in dirs.iter() {
                    if candidate.parent() == Some(path) {
                        paths.push(candidate.clone());
                    }
                }
            }
            {
                let files = self.files.lock().unwrap();
                for candidate in files.keys() {
                    if candidate.parent() == Some(path) {
                        paths.push(candidate.clone());
                    }
                }
            }
            paths.sort();
            paths.dedup();
            Ok(paths)
        }

        fn rename(&self, from: &Path, to: &Path) -> Result<(), String> {
            let bytes = self.read_file(from)?;
            self.remove_file(from)?;
            self.write_file(to, &bytes)
        }

        fn remove_file(&self, path: &Path) -> Result<(), String> {
            self.files.lock().unwrap().remove(path);
            Ok(())
        }

        fn remove_directory(&self, path: &Path) -> Result<(), String> {
            self.files
                .lock()
                .unwrap()
                .retain(|candidate, _| !candidate.starts_with(path));
            self.dirs.lock().unwrap().remove(path);
            Ok(())
        }

        fn is_regular_file(&self, path: &Path) -> bool {
            self.files.lock().unwrap().contains_key(path)
        }

        fn is_symlink(&self, _path: &Path) -> bool {
            false
        }

        fn is_directory(&self, path: &Path) -> bool {
            self.dirs.lock().unwrap().contains(path)
                || self
                    .files
                    .lock()
                    .unwrap()
                    .keys()
                    .any(|candidate| candidate.starts_with(path))
        }

        fn modified(&self, _path: &Path) -> Option<SystemTime> {
            None
        }

        fn validate_archive_entries(&self, entries: &[crate::ports::ArchiveEntry], limits: &crate::ports::ArchiveLimits) -> Result<(), String> {
            PathPort::validate_archive_entries(&SafeWorkspace, entries, limits)
        }
    }

    #[test]
    fn scoped_workspace_initializes_launcher_profiles_and_versions() {
        let workspace = MemoryWorkspace::new();
        let root = Path::new("game");
        let scope = ScopedWorkspace::new(root, &workspace);
        scope.ensure_launcher_profiles().unwrap();
        assert!(scope.is_regular_file("launcher_profiles.json"));
        let launcher_profiles: Value =
            serde_json::from_slice(&scope.read("launcher_profiles.json").unwrap()).unwrap();
        assert_eq!(launcher_profiles.get("version").and_then(Value::as_u64), Some(3));
        scope.write_profile_atomic("1.20.1", b"{}").unwrap();
        assert!(scope.read_version_profile("1.20.1").unwrap().is_object());
        assert!(scope.version_profile_path("1.20.1").unwrap().ends_with("1.20.1.json"));
        assert!(scope.find_installed_profile(Some("1.20.1"), Some("1.20.1")).unwrap().is_some());
    }

    #[test]
    fn shared_network_error_translation_preserves_loader_semantics() {
        assert_eq!(
            translate_network_error(
                "Fabric Meta",
                NetworkOperation::Request,
                "Fabric Meta request failed: HTTP status client error (404 Not Found)"
            ),
            "Fabric Meta did not find the requested resource."
        );
        assert_eq!(
            translate_network_error(
                "Fabric Meta",
                NetworkOperation::Request,
                "Fabric Meta request failed: HTTP status client error (401 Unauthorized)"
            ),
            "Fabric Meta rejected the request. Check access permissions or try another version."
        );
        assert_eq!(
            translate_network_error(
                "Fabric Meta",
                NetworkOperation::Request,
                "Fabric Meta request failed: HTTP status client error (403 Forbidden)"
            ),
            "Fabric Meta rejected the request. Check access permissions or try another version."
        );
        assert_eq!(
            translate_network_error(
                "Fabric Meta",
                NetworkOperation::Request,
                "Fabric Meta request failed: HTTP status server error (500 Internal Server Error)"
            ),
            "Fabric Meta request failed with HTTP 500. Check connectivity or try another version."
        );
        assert_eq!(
            translate_network_error(
                "Fabric Meta",
                NetworkOperation::Request,
                "Fabric Meta request failed: connection reset while opening https://example.com/path?token=secret"
            ),
            "Cannot contact Fabric Meta: connection reset while opening [redacted-url]"
        );
        assert_eq!(
            translate_network_error(
                "Fabric Meta",
                NetworkOperation::Request,
                "Fabric Meta request failed: HTTP response is too large (999 bytes, limit 16777216)."
            ),
            "HTTP response is too large (999 bytes, limit 16777216)."
        );        assert_eq!(
            translate_network_error(
                "Fabric Meta",
                NetworkOperation::Request,
                "Fabric Meta request failed: Cannot read HTTP response: connection reset"
            ),
            "Cannot read HTTP response: connection reset"
        );
        assert_eq!(
            translate_network_error(
                "Forge installer",
                NetworkOperation::Download,
                "Forge installer: Download failed for Forge installer after 3 attempts: HTTP response is too large (999 bytes, limit 16777216)."
            ),
            "Download failed for Forge installer after 3 attempts: HTTP response is too large (999 bytes, limit 16777216)."
        );
        assert_eq!(
            translate_network_error(
                "Forge installer",
                NetworkOperation::Download,
                "Forge installer: Download failed for Forge installer after 3 attempts: connection reset"
            ),
            "Download failed for Forge installer after 3 attempts: connection reset"
        );
        assert_eq!(
            translate_network_error(
                "Forge installer",
                NetworkOperation::Download,
                "Forge installer: Download failed for Forge installer after 3 attempts: HTTP status client error (404 Not Found)"
            ),
            "Forge installer did not find the requested resource."
        );
    }

    #[test]
    fn version_helpers_handle_rules_native_files_and_version_order() {
        let platform = MinecraftPlatform::new("windows", "10.0", "x86_64");
        let context = platform.rule_context();
        assert_eq!(normalize_os("darwin"), "macos");
        assert_eq!(normalize_arch("amd64"), "x86_64");
        let library = json!({
            "name": "example:artifact:1.0",
            "downloads": {
                "artifact": {"path": "example/artifact/1.0/artifact-1.0.jar", "url": "https://example.com/a.jar", "sha1": "abc"},
                "classifiers": {"natives-64": {"path": "example/artifact/1.0/artifact-1.0-natives-64.jar", "url": "https://example.com/a-natives.jar", "sha1": "def"}}
            },
            "natives": {"windows": "natives-${arch}"}
        });
        let file = library_file(&library, None).unwrap().unwrap();
        assert_eq!(file.path, "example/artifact/1.0/artifact-1.0.jar");
        let (classifier, native) = library_native_file(&library, &context).unwrap().unwrap();
        assert_eq!(classifier, "natives-64");
        assert!(native.path.ends_with("-natives-64.jar"));
        assert_eq!(force_https_url("http://example.com/a.jar"), "https://example.com/a.jar");
        assert_eq!(compare_loader_versions_desc("1.10", "1.9"), Ordering::Less);
        let mut versions = vec!["1.9".to_string(), "1.10".to_string()];
        sort_loader_versions_desc(&mut versions, String::as_str);
        assert_eq!(versions, vec!["1.10".to_string(), "1.9".to_string()]);
    }

    #[test]
    fn meta_profile_installs_libraries_and_writes_profile() {
        let workspace = MemoryWorkspace::new();
        let profile = json!({
            "id": "1.20.1-example",
            "inheritsFrom": "1.20.1",
            "libraries": [{
                "name": "example:library:1.0",
                "downloads": {"artifact": {"path": "example/library/1.0/library-1.0.jar", "url": "https://example.com/library.jar"}}
            }]
        });
        let network = FakeNetwork::new(serde_json::to_vec(&profile).unwrap());
        let progress = NoopProgress;
        let logger = NoopLogger;
        let platform = MinecraftPlatform::new("linux", "", "x86_64");
        let context = MetaProfileContext::new(
            Path::new("game"),
            &network,
            &workspace,
            &progress,
            &logger,
            &platform,
            Deadline::after(Duration::from_secs(120)),
            Cancellation::new(),
        );
        let output = install_meta_profile(
            &context,
            &MetaProfileRequest::new("https://example.com/profile.json", "Example", "1.20.1"),
        )
        .unwrap();
        assert_eq!(output.installed_version_id, "1.20.1-example");
        assert_eq!(output.libraries_downloaded, 1);
        let downloads = network.downloads.lock().unwrap();
        assert_eq!(downloads.len(), 1);
        assert_eq!(downloads[0].max_bytes, MAX_LIBRARY_DOWNLOAD_BYTES);
        assert_eq!(downloads[0].timeout, METADATA_HTTP_TIMEOUT);
    }

    #[test]
    fn installer_flow_writes_log_and_discovers_profile() {
        let workspace = MemoryWorkspace::new();
        workspace
            .write_atomic(
                Path::new("game/loader-installers/installer.jar"),
                b"PK\x03\x04",
            )
            .unwrap();
        workspace
            .write_atomic(
                Path::new("game/versions/1.20.1-example/1.20.1-example.json"),
                br#"{"id":"1.20.1-example","inheritsFrom":"1.20.1"}"#,
            )
            .unwrap();
        let network = FakeNetwork::new(vec![b'P', b'K', 3, 4]);
        let java = FakeJava::new(ProcessOutput {
            status_code: Some(0),
            stdout: b"ok".to_vec(),
            stderr: Vec::new(),
            timed_out: false,
        });
        let progress = NoopProgress;
        let logger = NoopLogger;
        let installer_path = Path::new("java");
        let context = InstallerContext::new(
            Path::new("game"),
            &network,
            &workspace,
            &java,
            &progress,
            &logger,
            Deadline::after(Duration::from_secs(120)),
            Cancellation::new(),
        );
        let request = InstallerRequest::new(
            "https://example.com/installer.jar",
            "installer.jar",
            vec!["--install".to_string()],
            "Example",
            "example-1.20.1",
            "1.20.1",
            installer_path,
        );
        let output = install_java_installer(&context, &request).unwrap();
        assert_eq!(output.installed_version_id, "1.20.1-example");
        let java_requests = java.requests.lock().unwrap();
        assert_eq!(java_requests.len(), 1);
        assert_eq!(java_requests[0].timeout, JAVA_PROCESS_TIMEOUT);
        assert_eq!(java_requests[0].main_class, "-jar");
        assert!(java_requests[0].program_args[0].ends_with("installer.jar"));
        assert!(workspace
            .files
            .lock()
            .unwrap()
            .keys()
            .any(|path| path.to_string_lossy().ends_with(".log")));
        let downloads = network.downloads.lock().unwrap();
        assert_eq!(downloads.len(), 1);
        assert_eq!(downloads[0].max_bytes, MAX_INSTALLER_BYTES);
        assert_eq!(downloads[0].timeout, INSTALLER_HTTP_TIMEOUT);
    }

    #[test]
    fn fake_network_can_return_invalid_json_and_object_safety_holds() {
        let network = FakeNetwork::new(b"not json".to_vec());
        let deadline = Deadline::after(Duration::from_secs(75));
        let cancellation = Cancellation::new();
        let error = fetch_json(
            &network,
            "https://example.com/metadata.json",
            "Example",
            1024,
            METADATA_HTTP_TIMEOUT,
            &deadline,
            &cancellation,
        )
        .expect_err("invalid JSON must fail");
        assert!(error.starts_with("Example returned invalid JSON:"));
        fn assert_send_sync<T: ?Sized + Send + Sync>() {}
        assert_send_sync::<dyn NetworkPort>();
        assert_send_sync::<dyn JavaRunner>();
        let progress = NoopProgress;
        progress.report(ProgressUpdate::new(1, Some(1), None));
    }

    #[test]
    fn installer_log_is_hard_bounded_after_redaction() {
        let output = ProcessOutput {
            status_code: Some(0),
            stdout: vec![b'x'; MAX_INSTALLER_LOG_BYTES + 1],
            stderr: Vec::new(),
            timed_out: false,
        };
        let log = installer_log_bytes(&output);
        assert!(log.len() <= MAX_INSTALLER_LOG_BYTES);
        assert!(String::from_utf8_lossy(&log).contains("[truncated]"));
    }

    #[test]
    fn installer_log_redacts_urls_tokens_and_api_keys() {
        let output = ProcessOutput {
            status_code: Some(0),
            stdout: b"Downloading https://example.com/file?X-Amz-Signature=deadbeef&access_token=secret\r\nAuthorization: Bearer bearer-token\r\n".to_vec(),
            stderr: b"X-Api-Key: key123\naccess_token=access-value\nrefresh_token=refresh-token\ntoken=plain-token\nX-Amz-Signature=signed-value\n".to_vec(),
            timed_out: false,
        };
        let log = String::from_utf8(installer_log_bytes(&output)).unwrap();
        for secret in ["deadbeef", "secret", "bearer-token", "key123", "access-value", "refresh-token", "plain-token", "signed-value"] {
            assert!(!log.contains(secret), "secret leaked in installer log: {log}");
        }
        assert!(log.contains("Downloading"));
        assert!(log.contains("[redacted-url]"));
        assert!(log.contains("stdout:"));
        assert!(log.contains("stderr:"));
        assert!(log.contains("\r\n"));
        assert!(log.len() <= MAX_INSTALLER_LOG_BYTES);
    }

    #[test]
    fn progress_messages_redact_secrets_urls_and_controls_and_are_bounded() {
        let message = sanitize_progress_message(
            "Downloading https://example.com/file?token=url-secret\nAuthorization: Bearer bearer-secret\tapi_key=api-secret\u{0007}",
        );
        for secret in ["url-secret", "bearer-secret", "api-secret"] {
            assert!(!message.contains(secret), "progress secret leaked: {message}");
        }
        assert!(!message.chars().any(char::is_control));
        assert!(message.contains("[redacted-url]"));
        assert!(message.contains("Downloading"));

        let bounded = sanitize_progress_message(&"x".repeat(MAX_PROGRESS_MESSAGE_CHARS + 32));
        assert_eq!(
            bounded.chars().count(),
            MAX_PROGRESS_MESSAGE_CHARS
        );
        assert!(bounded.ends_with("..."));
    }

    #[test]
    fn installer_log_redacts_quoted_json_keys() {
        let output = ProcessOutput {
            status_code: Some(0),
            stdout: br#"{"access_token":"json-access-value","authorization":"Bearer json-authorization-value"}"#.to_vec(),
            stderr: br#"{"api_key": "json-key-value", "refresh_token": "json-refresh-value"} api_key = "mixed-key-value" authorization: "mixed-auth-value""#.to_vec(),
            timed_out: false,
        };
        let log = String::from_utf8(installer_log_bytes(&output)).unwrap();
        for secret in [
            "json-access-value",
            "json-authorization-value",
            "json-key-value",
            "json-refresh-value",
            "mixed-key-value",
            "mixed-auth-value",
        ] {
            assert!(!log.contains(secret), "quoted JSON secret leaked: {log}");
        }
        assert!(log.contains(r#""access_token":"[redacted]""#));
        assert!(log.contains(r#""authorization":"[redacted]""#));
        assert!(log.contains(r#""api_key": "[redacted]""#));
        assert!(log.contains(r#""refresh_token": "[redacted]""#));
        assert!(log.contains(r#"api_key = "[redacted]""#));
        assert!(log.contains(r#"authorization: "[redacted]""#));
        assert!(log.contains("stdout:"));
        assert!(log.contains("stderr:"));
        assert!(log.len() <= MAX_INSTALLER_LOG_BYTES);
    }

    #[test]
    fn memory_workspace_archive_validation_matches_core_policy() {
        let workspace = MemoryWorkspace::new();
        assert!(workspace
            .validate_archive_entries(&[crate::ports::ArchiveEntry::new("file.bin", 1, false)], &crate::ports::ArchiveLimits::default())
            .is_ok());
    }
}
