use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LauncherInfo {
    pub version: String,
    pub platform: String,
    pub data_dir: String,
    pub default_game_dir: String,
    pub settings_path: String,
}

pub use flint_feature_saves::{SaveInfo, SaveTransferResult};
pub use flint_feature_settings::LauncherSettings;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct InstanceInfo {
    pub id: String,
    pub name: String,
    pub version_type: String,
    pub loader: String,
    pub game_version: String,
    pub path: String,
    pub has_client_jar: bool,
    pub last_played: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct JavaInfo {
    pub path: String,
    pub version: String,
    pub major: u32,
    pub architecture: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RemoteVersion {
    pub id: String,
    pub version_type: String,
    pub release_time: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct InstallProgress {
    pub stage: String,
    pub completed: u64,
    pub total: u64,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct InstallResult {
    pub version_id: String,
    pub files_written: u64,
    pub bytes_downloaded: u64,
    pub install_path: String,
}

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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AccountType {
    Offline,
    Microsoft,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LoginStatus {
    Pending,
    Success,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AccountInfo {
    pub id: String,
    pub username: String,
    pub uuid: Option<String>,
    pub account_type: AccountType,
    pub active: bool,
    pub skin_url: Option<String>,
    pub expires_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MicrosoftDeviceCode {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval: u64,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MicrosoftLoginPoll {
    pub status: LoginStatus,
    pub message: String,
    pub account: Option<AccountInfo>,
    pub slow_down: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LoaderKind {
    Forge,
    Neoforge,
    Fabric,
    Quilt,
    Liteloader,
    Optifine,
}

impl LoaderKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Forge => "forge",
            Self::Neoforge => "neoforge",
            Self::Fabric => "fabric",
            Self::Quilt => "quilt",
            Self::Liteloader => "liteloader",
            Self::Optifine => "optifine",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LoaderVersionInfo {
    pub loader: LoaderKind,
    pub version: String,
    pub label: String,
    pub recommended: bool,
    pub game_version: String,
    pub release_time: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LoaderInstallRequest {
    pub game_dir: String,
    pub game_version: String,
    pub loader: LoaderKind,
    pub loader_version: String,
    pub java_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LoaderInstallResult {
    pub game_dir: String,
    pub game_version: String,
    pub loader: LoaderKind,
    pub loader_version: String,
    pub installed_version_id: String,
    pub install_path: String,
    pub log_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LoaderInstallProgress {
    pub loader: LoaderKind,
    pub game_version: String,
    pub loader_version: String,
    pub stage: String,
    pub completed: u64,
    pub total: u64,
    pub message: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ModSource {
    Curseforge,
    Modrinth,
}

impl ModSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Curseforge => "curseforge",
            Self::Modrinth => "modrinth",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ModDependencyType {
    Required,
    Optional,
    Embedded,
    Incompatible,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ModProject {
    pub id: String,
    pub source: ModSource,
    pub slug: String,
    pub name: String,
    pub summary: String,
    pub author: String,
    pub downloads: u64,
    pub icon_url: Option<String>,
    pub categories: Vec<String>,
    pub project_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ModSearchPage {
    pub source: ModSource,
    pub total: u64,
    pub offset: u32,
    pub limit: u32,
    pub hits: Vec<ModProject>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ModDependencyRef {
    pub source: ModSource,
    pub project_id: String,
    pub version_id: Option<String>,
    pub name: String,
    pub dependency_type: ModDependencyType,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ModVersionInfo {
    pub id: String,
    pub source: ModSource,
    pub project_id: String,
    pub name: String,
    pub version_number: String,
    pub file_name: String,
    pub download_url: Option<String>,
    pub game_versions: Vec<String>,
    pub loaders: Vec<String>,
    pub release_type: String,
    pub published_at: Option<String>,
    pub dependencies: Vec<ModDependencyRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ModDependencyPlan {
    pub root: ModVersionInfo,
    pub required: Vec<ModVersionInfo>,
    pub optional: Vec<ModDependencyRef>,
    pub conflicts: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct InstalledModInfo {
    pub file_name: String,
    pub name: String,
    pub mod_id: Option<String>,
    pub version: Option<String>,
    pub loader: Option<String>,
    pub enabled: bool,
    pub path: String,
    pub source: Option<ModSource>,
    pub project_id: Option<String>,
    pub version_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ModInstallProgress {
    pub project_name: String,
    pub file_name: String,
    pub stage: String,
    pub completed: u64,
    pub total: u64,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ModInstallResult {
    pub root: InstalledModInfo,
    pub dependencies: Vec<InstalledModInfo>,
    pub skipped: Vec<String>,
    pub warnings: Vec<String>,
}

#[cfg(test)]
mod contract_tests {
    use super::*;

    #[test]
    fn enums_use_contract_lowercase_names() {
        assert_eq!(
            serde_json::to_string(&LoaderKind::Neoforge).unwrap(),
            "\"neoforge\""
        );
        assert_eq!(
            serde_json::to_string(&LoaderKind::Liteloader).unwrap(),
            "\"liteloader\""
        );
        assert_eq!(
            serde_json::to_string(&ModSource::Curseforge).unwrap(),
            "\"curseforge\""
        );
        assert_eq!(
            serde_json::to_string(&ModSource::Modrinth).unwrap(),
            "\"modrinth\""
        );
        assert_eq!(
            serde_json::to_string(&AccountType::Microsoft).unwrap(),
            "\"microsoft\""
        );
        assert_eq!(
            serde_json::to_string(&ModDependencyType::Incompatible).unwrap(),
            "\"incompatible\""
        );
    }

    #[test]
    fn changed_models_serialize_with_camel_case_fields() {
        let instance = InstanceInfo {
            id: "id".to_string(),
            name: "Name".to_string(),
            version_type: "release".to_string(),
            loader: "Fabric".to_string(),
            game_version: "1.20.1".to_string(),
            path: "path".to_string(),
            has_client_jar: true,
            last_played: None,
        };
        let value = serde_json::to_value(instance).unwrap();
        assert!(value.get("gameVersion").is_some());
        assert!(value.get("hasClientJar").is_some());

        let save = SaveInfo {
            folder_name: "World".to_string(),
            display_name: "World".to_string(),
            path: "path".to_string(),
            icon_data_url: None,
            last_played: Some(1),
            size_bytes: 2,
            game_mode: Some("survival".to_string()),
            hardcore: false,
            version_name: Some("1.20.1".to_string()),
        };
        let value = serde_json::to_value(save).unwrap();
        assert!(value.get("folderName").is_some());
        assert!(value.get("iconDataUrl").is_some());
        assert!(value.get("lastPlayed").is_some());
        assert!(value.get("sizeBytes").is_some());
        assert!(value.get("versionName").is_some());

        let poll = MicrosoftLoginPoll {
            status: LoginStatus::Pending,
            message: "Waiting".to_string(),
            account: None,
            slow_down: false,
        };
        let value = serde_json::to_value(poll).unwrap();
        assert_eq!(value.get("slowDown"), Some(&serde_json::Value::Bool(false)));
    }
}
