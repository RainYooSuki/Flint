mod error;
mod install;
mod java;
mod launch;
mod models;
mod paths;
mod settings;
mod version;

use models::{
    InstallResult, InstanceInfo, JavaInfo, LaunchRequest, LaunchResult, LauncherInfo,
    LauncherSettings, RemoteVersion,
};

#[tauri::command]
fn launcher_info() -> LauncherInfo {
    let data_dir = paths::data_dir();
    LauncherInfo {
        version: env!("CARGO_PKG_VERSION").to_string(),
        platform: format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH),
        data_dir: data_dir.display().to_string(),
        default_game_dir: paths::default_game_dir().display().to_string(),
        settings_path: paths::settings_path().display().to_string(),
    }
}

#[tauri::command]
fn load_settings() -> Result<LauncherSettings, String> {
    settings::load_settings()
}

#[tauri::command]
fn save_settings(settings: LauncherSettings) -> Result<LauncherSettings, String> {
    settings::save_settings(settings)
}

#[tauri::command]
fn list_instances(game_dir: String) -> Result<Vec<InstanceInfo>, String> {
    version::list_instances(&game_dir)
}

#[tauri::command]
async fn scan_java() -> Vec<JavaInfo> {
    match tokio::task::spawn_blocking(java::scan_java).await {
        Ok(result) => result,
        Err(error) => {
            eprintln!("Java scan task failed: {error}");
            Vec::new()
        }
    }
}

#[tauri::command]
async fn fetch_remote_versions() -> Result<Vec<RemoteVersion>, String> {
    tokio::task::spawn_blocking(install::fetch_remote_versions_blocking)
        .await
        .map_err(error::background_error)?
}

#[tauri::command]
async fn install_version(
    app: tauri::AppHandle,
    game_dir: String,
    version_id: String,
) -> Result<InstallResult, String> {
    tokio::task::spawn_blocking(move || install::install_blocking(&app, &game_dir, &version_id))
        .await
        .map_err(error::background_error)?
}

#[tauri::command]
async fn launch_instance(request: LaunchRequest) -> Result<LaunchResult, String> {
    tokio::task::spawn_blocking(move || launch::launch_blocking(request))
        .await
        .map_err(error::background_error)?
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            launcher_info,
            load_settings,
            save_settings,
            list_instances,
            scan_java,
            fetch_remote_versions,
            install_version,
            launch_instance
        ])
        .run(tauri::generate_context!())
        .expect("error while running Flint");
}
