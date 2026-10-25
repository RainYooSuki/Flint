mod feature_host;
mod install;
mod loaders;
mod models;
mod mods;
mod provider_composition;

pub(crate) use flint_core::{error, paths};
pub(crate) use flint_feature_accounts as auth;
pub(crate) use flint_feature_launch as launch;
pub(crate) use flint_feature_saves as saves;
pub(crate) use flint_feature_settings as settings;
pub(crate) use flint_feature_vanilla as vanilla;
pub(crate) use flint_feature_vanilla::{java, version};

use models::{
    AccountInfo, InstallResult, InstalledModInfo, InstanceInfo, JavaInfo, LaunchRequest,
    LaunchResult, LauncherInfo, LauncherSettings, LoaderInstallRequest, LoaderInstallResult,
    LoaderKind, LoaderVersionInfo, MicrosoftDeviceCode, MicrosoftLoginPoll, ModDependencyPlan,
    ModInstallResult, ModSearchPage, ModSource, ModVersionInfo, RemoteVersion, SaveInfo,
    SaveTransferResult,
};

use tauri::Manager;

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
fn get_host_availability(
    runtime: tauri::State<'_, feature_host::HostRuntime>,
) -> flint_feature_api::FeatureAvailabilitySnapshot {
    runtime.snapshot()
}

#[tauri::command]
fn load_settings(
    runtime: tauri::State<'_, feature_host::HostRuntime>,
) -> Result<LauncherSettings, String> {
    runtime.ensure_feature_ready(feature_host::FEATURE_SETTINGS)?;
    drop(runtime);
    settings::load_settings()
}

#[tauri::command]
fn save_settings(
    runtime: tauri::State<'_, feature_host::HostRuntime>,
    settings: LauncherSettings,
) -> Result<LauncherSettings, String> {
    runtime.ensure_feature_ready(feature_host::FEATURE_SETTINGS)?;
    drop(runtime);
    settings::save_settings(settings)
}

#[tauri::command]
fn list_instances(
    runtime: tauri::State<'_, feature_host::HostRuntime>,
    game_dir: String,
) -> Result<Vec<InstanceInfo>, String> {
    runtime.ensure_feature_ready(feature_host::FEATURE_VANILLA)?;
    drop(runtime);
    version::list_instances(&game_dir)
}

#[tauri::command]
async fn scan_java(
    runtime: tauri::State<'_, feature_host::HostRuntime>,
) -> Result<Vec<JavaInfo>, String> {
    runtime.ensure_feature_ready(feature_host::FEATURE_VANILLA)?;
    drop(runtime);
    match tokio::task::spawn_blocking(java::scan_java).await {
        Ok(result) => Ok(result),
        Err(error) => {
            eprintln!("Java scan task failed: {error}");
            Ok(Vec::new())
        }
    }
}

#[tauri::command]
async fn fetch_remote_versions(
    runtime: tauri::State<'_, feature_host::HostRuntime>,
) -> Result<Vec<RemoteVersion>, String> {
    runtime.ensure_feature_ready(feature_host::FEATURE_VANILLA)?;
    drop(runtime);
    tokio::task::spawn_blocking(install::fetch_remote_versions_blocking)
        .await
        .map_err(error::background_error)?
}

#[tauri::command]
async fn install_version(
    runtime: tauri::State<'_, feature_host::HostRuntime>,
    app: tauri::AppHandle,
    game_dir: String,
    version_id: String,
) -> Result<InstallResult, String> {
    runtime.ensure_feature_ready(feature_host::FEATURE_VANILLA)?;
    drop(runtime);
    tokio::task::spawn_blocking(move || install::install_blocking(&app, &game_dir, &version_id))
        .await
        .map_err(error::background_error)?
}

#[tauri::command]
async fn launch_instance(
    runtime: tauri::State<'_, feature_host::HostRuntime>,
    request: LaunchRequest,
) -> Result<LaunchResult, String> {
    runtime.ensure_feature_ready(feature_host::FEATURE_LAUNCH)?;
    drop(runtime);
    tokio::task::spawn_blocking(move || launch::launch_blocking(request))
        .await
        .map_err(error::background_error)?
}

#[tauri::command]
async fn list_accounts(
    runtime: tauri::State<'_, feature_host::HostRuntime>,
) -> Result<Vec<AccountInfo>, String> {
    runtime.ensure_feature_ready(feature_host::FEATURE_ACCOUNTS)?;
    drop(runtime);
    tokio::task::spawn_blocking(auth::list_accounts)
        .await
        .map_err(error::background_error)?
}

#[tauri::command]
async fn begin_microsoft_login(
    runtime: tauri::State<'_, feature_host::HostRuntime>,
) -> Result<MicrosoftDeviceCode, String> {
    runtime.ensure_feature_ready(feature_host::FEATURE_ACCOUNTS)?;
    drop(runtime);
    tokio::task::spawn_blocking(auth::begin_microsoft_login)
        .await
        .map_err(error::background_error)?
}

#[tauri::command]
async fn poll_microsoft_login(
    runtime: tauri::State<'_, feature_host::HostRuntime>,
    device_code: String,
) -> Result<MicrosoftLoginPoll, String> {
    runtime.ensure_feature_ready(feature_host::FEATURE_ACCOUNTS)?;
    drop(runtime);
    tokio::task::spawn_blocking(move || auth::poll_microsoft_login(&device_code))
        .await
        .map_err(error::background_error)?
}

#[tauri::command]
async fn select_account(
    runtime: tauri::State<'_, feature_host::HostRuntime>,
    account_id: String,
) -> Result<Vec<AccountInfo>, String> {
    runtime.ensure_feature_ready(feature_host::FEATURE_ACCOUNTS)?;
    drop(runtime);
    tokio::task::spawn_blocking(move || auth::select_account(&account_id))
        .await
        .map_err(error::background_error)?
}

#[tauri::command]
async fn remove_account(
    runtime: tauri::State<'_, feature_host::HostRuntime>,
    account_id: String,
) -> Result<Vec<AccountInfo>, String> {
    runtime.ensure_feature_ready(feature_host::FEATURE_ACCOUNTS)?;
    drop(runtime);
    tokio::task::spawn_blocking(move || auth::remove_account(&account_id))
        .await
        .map_err(error::background_error)?
}

#[tauri::command]
async fn refresh_active_account(
    runtime: tauri::State<'_, feature_host::HostRuntime>,
) -> Result<AccountInfo, String> {
    runtime.ensure_feature_ready(feature_host::FEATURE_ACCOUNTS)?;
    drop(runtime);
    tokio::task::spawn_blocking(auth::refresh_active_account)
        .await
        .map_err(error::background_error)?
}

#[tauri::command]
async fn list_loader_versions(
    runtime: tauri::State<'_, feature_host::HostRuntime>,
    loader_providers: tauri::State<'_, provider_composition::LoaderProviderState>,
    loader: LoaderKind,
    game_version: String,
) -> Result<Vec<LoaderVersionInfo>, String> {
    runtime.ensure_feature_ready(feature_host::FEATURE_LOADERS)?;
    drop(runtime);
    let registry = loader_providers.registry()?;
    drop(loader_providers);
    tokio::task::spawn_blocking(move || {
        loaders::list_loader_versions_blocking(registry, loader, &game_version)
    })
    .await
    .map_err(error::background_error)?
}

#[tauri::command]
async fn install_loader(
    runtime: tauri::State<'_, feature_host::HostRuntime>,
    loader_providers: tauri::State<'_, provider_composition::LoaderProviderState>,
    app: tauri::AppHandle,
    request: LoaderInstallRequest,
) -> Result<LoaderInstallResult, String> {
    runtime.ensure_feature_ready(feature_host::FEATURE_LOADERS)?;
    drop(runtime);
    let registry = loader_providers.registry()?;
    drop(loader_providers);
    tokio::task::spawn_blocking(move || {
        loaders::install_loader_blocking(&app, registry, &request)
    })
    .await
    .map_err(error::background_error)?
}

#[tauri::command]
async fn search_mods(
    runtime: tauri::State<'_, feature_host::HostRuntime>,
    providers: tauri::State<'_, provider_composition::ModProviderState>,
    source: ModSource,
    query: String,
    game_version: String,
    loader: String,
    offset: u32,
    limit: u32,
) -> Result<ModSearchPage, String> {
    runtime.ensure_feature_ready(feature_host::FEATURE_MODS)?;
    let registry = providers.registry()?;
    drop(runtime);
    drop(providers);
    tokio::task::spawn_blocking(move || {
        mods::search_mods_blocking(
            &registry,
            source,
            &query,
            &game_version,
            &loader,
            offset,
            limit,
        )
    })
    .await
    .map_err(error::background_error)?
}

#[tauri::command]
async fn list_mod_versions(
    runtime: tauri::State<'_, feature_host::HostRuntime>,
    providers: tauri::State<'_, provider_composition::ModProviderState>,
    source: ModSource,
    project_id: String,
    game_version: String,
    loader: String,
) -> Result<Vec<ModVersionInfo>, String> {
    runtime.ensure_feature_ready(feature_host::FEATURE_MODS)?;
    let registry = providers.registry()?;
    drop(runtime);
    drop(providers);
    tokio::task::spawn_blocking(move || {
        mods::list_mod_versions_blocking(
            &registry,
            source,
            &project_id,
            &game_version,
            &loader,
        )
    })
    .await
    .map_err(error::background_error)?
}

#[tauri::command]
async fn resolve_mod_dependencies(
    runtime: tauri::State<'_, feature_host::HostRuntime>,
    providers: tauri::State<'_, provider_composition::ModProviderState>,
    source: ModSource,
    project_id: String,
    version_id: String,
    game_version: String,
    loader: String,
) -> Result<ModDependencyPlan, String> {
    runtime.ensure_feature_ready(feature_host::FEATURE_MODS)?;
    let registry = providers.registry()?;
    drop(runtime);
    drop(providers);
    tokio::task::spawn_blocking(move || {
        mods::resolve_mod_dependencies_blocking(
            &registry,
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
    runtime: tauri::State<'_, feature_host::HostRuntime>,
    providers: tauri::State<'_, provider_composition::ModProviderState>,
    app: tauri::AppHandle,
    game_dir: String,
    source: ModSource,
    project_id: String,
    version_id: String,
    game_version: String,
    loader: String,
    install_dependencies: bool,
) -> Result<ModInstallResult, String> {
    runtime.ensure_feature_ready(feature_host::FEATURE_MODS)?;
    let registry = providers.registry()?;
    drop(runtime);
    drop(providers);
    tokio::task::spawn_blocking(move || {
        mods::install_mod_blocking(
            &registry,
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
async fn list_installed_mods(
    runtime: tauri::State<'_, feature_host::HostRuntime>,
    game_dir: String,
) -> Result<Vec<InstalledModInfo>, String> {
    runtime.ensure_feature_ready(feature_host::FEATURE_MODS)?;
    drop(runtime);
    tokio::task::spawn_blocking(move || mods::list_installed_mods_blocking(&game_dir))
        .await
        .map_err(error::background_error)?
}

#[tauri::command]
async fn set_mod_enabled(
    runtime: tauri::State<'_, feature_host::HostRuntime>,
    game_dir: String,
    file_name: String,
    enabled: bool,
) -> Result<InstalledModInfo, String> {
    runtime.ensure_feature_ready(feature_host::FEATURE_MODS)?;
    drop(runtime);
    tokio::task::spawn_blocking(move || {
        mods::set_mod_enabled_blocking(&game_dir, &file_name, enabled)
    })
    .await
    .map_err(error::background_error)?
}

#[tauri::command]
async fn delete_mod(
    runtime: tauri::State<'_, feature_host::HostRuntime>,
    game_dir: String,
    file_name: String,
) -> Result<(), String> {
    runtime.ensure_feature_ready(feature_host::FEATURE_MODS)?;
    drop(runtime);
    tokio::task::spawn_blocking(move || mods::delete_mod_blocking(&game_dir, &file_name))
        .await
        .map_err(error::background_error)?
}

#[tauri::command]
async fn list_saves(
    runtime: tauri::State<'_, feature_host::HostRuntime>,
    game_dir: String,
) -> Result<Vec<SaveInfo>, String> {
    runtime.ensure_feature_ready(feature_host::FEATURE_SAVES)?;
    drop(runtime);
    tokio::task::spawn_blocking(move || saves::list_saves_blocking(&game_dir))
        .await
        .map_err(error::background_error)?
}

#[tauri::command]
async fn backup_save(
    runtime: tauri::State<'_, feature_host::HostRuntime>,
    game_dir: String,
    folder_name: String,
) -> Result<SaveTransferResult, String> {
    runtime.ensure_feature_ready(feature_host::FEATURE_SAVES)?;
    drop(runtime);
    tokio::task::spawn_blocking(move || saves::backup_save_blocking(&game_dir, &folder_name))
        .await
        .map_err(error::background_error)?
}

#[tauri::command]
async fn export_save(
    runtime: tauri::State<'_, feature_host::HostRuntime>,
    game_dir: String,
    folder_name: String,
    destination: String,
) -> Result<SaveTransferResult, String> {
    runtime.ensure_feature_ready(feature_host::FEATURE_SAVES)?;
    drop(runtime);
    tokio::task::spawn_blocking(move || {
        saves::export_save_blocking(&game_dir, &folder_name, &destination)
    })
    .await
    .map_err(error::background_error)?
}

#[tauri::command]
async fn import_save(
    runtime: tauri::State<'_, feature_host::HostRuntime>,
    game_dir: String,
    archive_path: String,
) -> Result<SaveInfo, String> {
    runtime.ensure_feature_ready(feature_host::FEATURE_SAVES)?;
    drop(runtime);
    tokio::task::spawn_blocking(move || saves::import_save_blocking(&game_dir, &archive_path))
        .await
        .map_err(error::background_error)?
}

#[tauri::command]
async fn rename_save(
    runtime: tauri::State<'_, feature_host::HostRuntime>,
    game_dir: String,
    folder_name: String,
    new_name: String,
) -> Result<SaveInfo, String> {
    runtime.ensure_feature_ready(feature_host::FEATURE_SAVES)?;
    drop(runtime);
    tokio::task::spawn_blocking(move || {
        saves::rename_save_blocking(&game_dir, &folder_name, &new_name)
    })
    .await
    .map_err(error::background_error)?
}

#[tauri::command]
async fn delete_save(
    runtime: tauri::State<'_, feature_host::HostRuntime>,
    game_dir: String,
    folder_name: String,
) -> Result<(), String> {
    runtime.ensure_feature_ready(feature_host::FEATURE_SAVES)?;
    drop(runtime);
    tokio::task::spawn_blocking(move || saves::delete_save_blocking(&game_dir, &folder_name))
        .await
        .map_err(error::background_error)?
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            if !app.manage(feature_host::initialize_host_runtime()) {
                eprintln!("Host feature runtime state registration failed; commands remain gated.");
            }
            if !app.manage(provider_composition::ModProviderState::new()) {
                eprintln!("Mod provider registry state registration failed; provider commands will be unavailable.");
            }
            if !app.manage(provider_composition::LoaderProviderState::new()) {
                eprintln!("Loader provider registry state registration failed; loader commands will be unavailable.");
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            launcher_info,
            get_host_availability,
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
        .build(tauri::generate_context!())
        .expect("error while building Flint");

    app.run(|app_handle, event| {
        if matches!(event, tauri::RunEvent::Exit) {
            match app_handle.try_state::<feature_host::HostRuntime>() {
                Some(runtime) => runtime.shutdown(),
                None => eprintln!(
                    "Host feature runtime state was unavailable during exit shutdown."
                ),
            }
        }
    });
}
