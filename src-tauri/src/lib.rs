mod auth;
mod error;
mod install;
mod java;
mod launch;
mod loaders;
mod models;
mod mods;
mod net;
mod paths;
mod saves;
mod settings;
mod version;

use models::{
    AccountInfo, InstallResult, InstalledModInfo, InstanceInfo, JavaInfo, LaunchRequest,
    LaunchResult, LauncherInfo, LauncherSettings, LoaderInstallRequest, LoaderInstallResult,
    LoaderKind, LoaderVersionInfo, MicrosoftDeviceCode, MicrosoftLoginPoll, ModDependencyPlan,
    ModInstallResult, ModSearchPage, ModSource, ModVersionInfo, RemoteVersion, SaveInfo,
    SaveTransferResult,
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

#[tauri::command]
async fn list_accounts() -> Result<Vec<AccountInfo>, String> {
    tokio::task::spawn_blocking(auth::list_accounts)
        .await
        .map_err(error::background_error)?
}

#[tauri::command]
async fn begin_microsoft_login() -> Result<MicrosoftDeviceCode, String> {
    tokio::task::spawn_blocking(auth::begin_microsoft_login)
        .await
        .map_err(error::background_error)?
}

#[tauri::command]
async fn poll_microsoft_login(device_code: String) -> Result<MicrosoftLoginPoll, String> {
    tokio::task::spawn_blocking(move || auth::poll_microsoft_login(&device_code))
        .await
        .map_err(error::background_error)?
}

#[tauri::command]
async fn select_account(account_id: String) -> Result<Vec<AccountInfo>, String> {
    tokio::task::spawn_blocking(move || auth::select_account(&account_id))
        .await
        .map_err(error::background_error)?
}

#[tauri::command]
async fn remove_account(account_id: String) -> Result<Vec<AccountInfo>, String> {
    tokio::task::spawn_blocking(move || auth::remove_account(&account_id))
        .await
        .map_err(error::background_error)?
}

#[tauri::command]
async fn refresh_active_account() -> Result<AccountInfo, String> {
    tokio::task::spawn_blocking(auth::refresh_active_account)
        .await
        .map_err(error::background_error)?
}

#[tauri::command]
async fn list_loader_versions(
    loader: LoaderKind,
    game_version: String,
) -> Result<Vec<LoaderVersionInfo>, String> {
    tokio::task::spawn_blocking(move || {
        loaders::list_loader_versions_blocking(loader, &game_version)
    })
    .await
    .map_err(error::background_error)?
}

#[tauri::command]
async fn install_loader(
    app: tauri::AppHandle,
    request: LoaderInstallRequest,
) -> Result<LoaderInstallResult, String> {
    tokio::task::spawn_blocking(move || loaders::install_loader_blocking(&app, &request))
        .await
        .map_err(error::background_error)?
}

#[tauri::command]
async fn search_mods(
    source: ModSource,
    query: String,
    game_version: String,
    loader: String,
    offset: u32,
    limit: u32,
) -> Result<ModSearchPage, String> {
    tokio::task::spawn_blocking(move || {
        mods::search_mods_blocking(source, &query, &game_version, &loader, offset, limit)
    })
    .await
    .map_err(error::background_error)?
}

#[tauri::command]
async fn list_mod_versions(
    source: ModSource,
    project_id: String,
    game_version: String,
    loader: String,
) -> Result<Vec<ModVersionInfo>, String> {
    tokio::task::spawn_blocking(move || {
        mods::list_mod_versions_blocking(source, &project_id, &game_version, &loader)
    })
    .await
    .map_err(error::background_error)?
}

#[tauri::command]
async fn resolve_mod_dependencies(
    source: ModSource,
    project_id: String,
    version_id: String,
    game_version: String,
    loader: String,
) -> Result<ModDependencyPlan, String> {
    tokio::task::spawn_blocking(move || {
        mods::resolve_mod_dependencies_blocking(
            source,
            &project_id,
            &version_id,
            &game_version,
            &loader,
        )
    })
    .await
    .map_err(error::background_error)?
}

#[tauri::command]
async fn install_mod(
    app: tauri::AppHandle,
    game_dir: String,
    source: ModSource,
    project_id: String,
    version_id: String,
    game_version: String,
    loader: String,
    install_dependencies: bool,
) -> Result<ModInstallResult, String> {
    tokio::task::spawn_blocking(move || {
        mods::install_mod_blocking(
            &app,
            &game_dir,
            source,
            &project_id,
            &version_id,
            &game_version,
            &loader,
            install_dependencies,
        )
    })
    .await
    .map_err(error::background_error)?
}

#[tauri::command]
async fn list_installed_mods(game_dir: String) -> Result<Vec<InstalledModInfo>, String> {
    tokio::task::spawn_blocking(move || mods::list_installed_mods_blocking(&game_dir))
        .await
        .map_err(error::background_error)?
}

#[tauri::command]
async fn set_mod_enabled(
    game_dir: String,
    file_name: String,
    enabled: bool,
) -> Result<InstalledModInfo, String> {
    tokio::task::spawn_blocking(move || {
        mods::set_mod_enabled_blocking(&game_dir, &file_name, enabled)
    })
    .await
    .map_err(error::background_error)?
}

#[tauri::command]
async fn delete_mod(game_dir: String, file_name: String) -> Result<(), String> {
    tokio::task::spawn_blocking(move || mods::delete_mod_blocking(&game_dir, &file_name))
        .await
        .map_err(error::background_error)?
}

#[tauri::command]
async fn list_saves(game_dir: String) -> Result<Vec<SaveInfo>, String> {
    tokio::task::spawn_blocking(move || saves::list_saves_blocking(&game_dir))
        .await
        .map_err(error::background_error)?
}

#[tauri::command]
async fn backup_save(game_dir: String, folder_name: String) -> Result<SaveTransferResult, String> {
    tokio::task::spawn_blocking(move || saves::backup_save_blocking(&game_dir, &folder_name))
        .await
        .map_err(error::background_error)?
}

#[tauri::command]
async fn export_save(
    game_dir: String,
    folder_name: String,
    destination: String,
) -> Result<SaveTransferResult, String> {
    tokio::task::spawn_blocking(move || {
        saves::export_save_blocking(&game_dir, &folder_name, &destination)
    })
    .await
    .map_err(error::background_error)?
}

#[tauri::command]
async fn import_save(game_dir: String, archive_path: String) -> Result<SaveInfo, String> {
    tokio::task::spawn_blocking(move || saves::import_save_blocking(&game_dir, &archive_path))
        .await
        .map_err(error::background_error)?
}

#[tauri::command]
async fn rename_save(
    game_dir: String,
    folder_name: String,
    new_name: String,
) -> Result<SaveInfo, String> {
    tokio::task::spawn_blocking(move || {
        saves::rename_save_blocking(&game_dir, &folder_name, &new_name)
    })
    .await
    .map_err(error::background_error)?
}

#[tauri::command]
async fn delete_save(game_dir: String, folder_name: String) -> Result<(), String> {
    tokio::task::spawn_blocking(move || saves::delete_save_blocking(&game_dir, &folder_name))
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
            launch_instance,
            list_accounts,
            begin_microsoft_login,
            poll_microsoft_login,
            select_account,
            remove_account,
            refresh_active_account,
            list_loader_versions,
            install_loader,
            search_mods,
            list_mod_versions,
            resolve_mod_dependencies,
            install_mod,
            list_installed_mods,
            set_mod_enabled,
            delete_mod,
            list_saves,
            backup_save,
            export_save,
            import_save,
            rename_save,
            delete_save
        ])
        .run(tauri::generate_context!())
        .expect("error while running Flint");
}
