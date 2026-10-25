use serde::{Deserialize, Serialize};

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

pub mod install;
pub mod java;
pub mod version;

pub use install::{fetch_remote_versions_blocking, install_blocking, InstallProgressSink};
pub use java::*;
pub use version::*;

#[cfg(test)]
mod contract_tests {
    use super::*;

    #[test]
    fn vanilla_dtos_preserve_camel_case_contract_fields() {
        let value = serde_json::to_value(InstallProgress {
            stage: "assets".to_string(),
            completed: 1,
            total: 2,
            message: "Downloaded asset".to_string(),
        })
        .expect("install progress serializes");
        assert!(value.get("completed").is_some());
        assert!(value.get("total").is_some());

        let result = serde_json::to_value(InstallResult {
            version_id: "1.20.1".to_string(),
            files_written: 1,
            bytes_downloaded: 2,
            install_path: "versions/1.20.1".to_string(),
        })
        .expect("install result serializes");
        assert!(result.get("versionId").is_some());
        assert!(result.get("filesWritten").is_some());
        assert!(result.get("bytesDownloaded").is_some());
        assert!(result.get("installPath").is_some());

        let version = serde_json::to_value(RemoteVersion {
            id: "1.20.1".to_string(),
            version_type: "release".to_string(),
            release_time: "2023-06-12T00:00:00Z".to_string(),
        })
        .expect("remote version serializes");
        assert!(version.get("versionType").is_some());
        assert!(version.get("releaseTime").is_some());
    }
}