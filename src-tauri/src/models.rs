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

pub use flint_feature_accounts::{
    AccountInfo, AccountType, LoginStatus, MicrosoftDeviceCode, MicrosoftLoginPoll,
};
pub use flint_feature_vanilla::{
    InstallProgress, InstallResult, InstanceInfo, JavaInfo, RemoteVersion,
};

pub use flint_feature_launch::{LaunchRequest, LaunchResult};

pub use flint_feature_loaders::{
    LoaderInstallProgress, LoaderInstallRequest, LoaderInstallResult, LoaderKind, LoaderVersionInfo,
};

pub use flint_feature_mods::{
    InstalledModInfo, ModDependencyPlan, ModDependencyRef, ModDependencyType, ModInstallProgress,
    ModInstallResult, ModProject, ModSearchPage, ModSource, ModVersionInfo,
};

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
