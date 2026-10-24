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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", default)]
pub struct LauncherSettings {
    pub game_dir: String,
    pub java_path: String,
    pub username: String,
    pub min_memory_mb: u32,
    pub max_memory_mb: u32,
    pub jvm_args: String,
    pub game_args: String,
    pub width: u32,
    pub height: u32,
}

impl Default for LauncherSettings {
    fn default() -> Self {
        Self {
            game_dir: crate::paths::default_game_dir().display().to_string(),
            java_path: String::new(),
            username: "Player".to_string(),
            min_memory_mb: 1024,
            max_memory_mb: 4096,
            jvm_args: String::new(),
            game_args: String::new(),
            width: 854,
            height: 480,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct InstanceInfo {
    pub id: String,
    pub name: String,
    pub version_type: String,
    pub loader: String,
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
