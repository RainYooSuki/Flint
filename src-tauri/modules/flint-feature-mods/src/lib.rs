use serde::{Deserialize, Serialize};

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
use flint_core::{
    minecraft::sanitize_progress_message,
    net,
    paths::resolve_game_dir,
};
use flint_core::ports::{
    Cancellation, Deadline, DownloadRequest, NetworkPort, NoopLogger, NoopProgress,
    ReqwestNetwork, ServiceLabel,
};
use flint_provider_api::{
    mod_provider_requirement, validate_mod_provider_id, ModDependencyData, ModDependencyKind,
    ModDependencyVersionRequest, ModListVersionsRequest, ModProvider, ModProviderContext,
    ModProjectData, ModSearchRequest, ModSearchResultData, ModVersionData, ModVersionRequest,
    ProviderRegistry, ProviderRequirement, ScopedCredential, CURSEFORGE_SELECTION_KEY,
    MODRINTH_SELECTION_KEY,
};
use flint_feature_settings as settings;
use regex::Regex;
use serde_json::Value;
use std::{
    collections::HashSet,
    fs,
    io::Read,
    path::{Path, PathBuf},
    time::Duration,
};
use zip::ZipArchive;

static NOOP_PROGRESS: NoopProgress = NoopProgress;
static NOOP_LOGGER: NoopLogger = NoopLogger;

const MAX_MOD_DOWNLOAD: u64 = 512 * 1024 * 1024;
const MAX_MOD_METADATA_ENTRY: u64 = 2 * 1024 * 1024;
const MAX_MOD_ARCHIVE_ENTRIES: usize = 20_000;
const MAX_DEPENDENCY_DEPTH: usize = 8;
const MAX_DEPENDENCY_NODES: usize = 64;
const SEARCH_MODS_TIMEOUT: Duration = Duration::from_secs(45);
const LIST_MOD_VERSIONS_TIMEOUT: Duration = Duration::from_secs(45);
const RESOLVE_MOD_DEPENDENCIES_TIMEOUT: Duration = Duration::from_secs(60);
const INSTALL_MOD_TIMEOUT: Duration = Duration::from_secs(600);

/// Object-safe mod install progress boundary.
pub trait ModInstallProgressSink: Send + Sync {
    fn report(&self, progress: ModInstallProgress);
}

pub fn search_mods_blocking(
    registry: &ProviderRegistry<dyn ModProvider>,
    source: ModSource,
    query: &str,
    game_version: &str,
    loader: &str,
    offset: u32,
    limit: u32,
) -> Result<ModSearchPage, String> {
    validate_search_inputs(query, game_version, offset, limit)?;
    let loader = validate_mod_loader(loader)?;
    let timeout = SEARCH_MODS_TIMEOUT;
    let network = ReqwestNetwork::new(timeout)?;
    let selected = select_for_source(registry, source, &network, timeout)?;
    let result = selected.provider.search(
        &selected.context,
        &ModSearchRequest {
            query: query.trim().to_string(),
            game_version: game_version.trim().to_string(),
            loader: loader.to_string(),
            offset,
            limit,
        },
    )?;
    Ok(mod_search_page_from_data(result, source))
}

pub fn list_mod_versions_blocking(
    registry: &ProviderRegistry<dyn ModProvider>,
    source: ModSource,
    project_id: &str,
    game_version: &str,
    loader: &str,
) -> Result<Vec<ModVersionInfo>, String> {
    validate_mod_provider_id(project_id)?;
    validate_filter_text(game_version, "game version")?;
    let loader = validate_mod_loader(loader)?;
    let timeout = LIST_MOD_VERSIONS_TIMEOUT;
    let network = ReqwestNetwork::new(timeout)?;
    let selected = select_for_source(registry, source, &network, timeout)?;
    let versions = selected.provider.list_versions(
        &selected.context,
        &ModListVersionsRequest {
            project_id: project_id.to_string(),
            game_version: game_version.trim().to_string(),
            loader: loader.to_string(),
        },
    )?;
    Ok(versions
        .into_iter()
        .map(|version| mod_version_from_data(version, source))
        .collect())
}

pub fn resolve_mod_dependencies_blocking(
    registry: &ProviderRegistry<dyn ModProvider>,
    source: ModSource,
    project_id: &str,
    version_id: &str,
    game_version: &str,
    loader: &str,
) -> Result<ModDependencyPlan, String> {
    validate_mod_provider_id(project_id)?;
    if !version_id.trim().is_empty() {
        validate_mod_provider_id(version_id)?;
    }
    validate_filter_text(game_version, "game version")?;
    let loader = validate_mod_loader(loader)?;
    let timeout = RESOLVE_MOD_DEPENDENCIES_TIMEOUT;
    let network = ReqwestNetwork::new(timeout)?;
    let selected = select_for_source(registry, source, &network, timeout)?;
    let root = load_root_version(
        selected.provider,
        &selected.context,
        source,
        project_id,
        version_id,
        game_version.trim(),
        loader,
    )?;
    build_dependency_plan(
        selected.provider,
        &selected.context,
        source,
        root,
        game_version.trim(),
        loader,
    )
}

pub fn install_mod_blocking(
    registry: &ProviderRegistry<dyn ModProvider>,
    sink: &dyn ModInstallProgressSink,
    game_dir: &str,
    source: ModSource,
    project_id: &str,
    version_id: &str,
    game_version: &str,
    loader: &str,
    install_dependencies: bool,
) -> Result<ModInstallResult, String> {
    validate_mod_provider_id(project_id)?;
    if !version_id.trim().is_empty() {
        validate_mod_provider_id(version_id)?;
    }
    validate_filter_text(game_version, "game version")?;
    let loader = validate_mod_loader(loader)?;
    let game_root = resolve_game_dir(game_dir);
    let mods_dir = game_root.join("mods");
    net::ensure_directory(&mods_dir)?;
    let timeout = INSTALL_MOD_TIMEOUT;
    let network = ReqwestNetwork::new(timeout)?;
    let selected = select_for_source(registry, source, &network, timeout)?;
    let root_version = load_root_version(
        selected.provider,
        &selected.context,
        source,
        project_id,
        version_id,
        game_version.trim(),
        loader,
    )?;
    let mut warnings = Vec::new();
    let mut plan_required = Vec::new();
    if install_dependencies {
        let plan = build_dependency_plan(
            selected.provider,
            &selected.context,
            source,
            root_version.clone(),
            game_version.trim(),
            loader,
        )?;
        warnings.extend(plan.warnings);
        warnings.extend(plan.optional.iter().map(|dependency| {
            format!("Optional dependency {} was not installed.", dependency.name)
        }));
        warnings.extend(plan.conflicts.iter().cloned());
        plan_required = plan.required;
    }

    let mut tasks = Vec::new();
    tasks.push(root_version);
    for version in plan_required {
        let duplicate = tasks.iter().any(|existing: &ModVersionInfo| {
            existing.source == version.source
                && !existing.project_id.is_empty()
                && existing
                    .project_id
                    .eq_ignore_ascii_case(&version.project_id)
        });
        if !duplicate {
            tasks.push(version);
        }
    }

    let total = tasks.len() as u64;
    let mut root_result: Option<InstalledModInfo> = None;
    let mut dependency_results = Vec::new();
    let mut skipped = Vec::new();
    for (index, version) in tasks.iter().enumerate() {
        emit_mod_progress(
            sink,
            &version.name,
            version.file_name.as_str(),
            "download",
            index as u64,
            total,
            &format!("Installing {}", version.file_name),
        );
        let (installed, was_skipped) =
            install_single_mod(&network, &mods_dir, version, loader, INSTALL_MOD_TIMEOUT)?;
        if was_skipped {
            skipped.push(version.file_name.clone());
        }
        if index == 0 {
            root_result = Some(installed);
        } else if !was_skipped {
            dependency_results.push(installed);
        }
        emit_mod_progress(
            sink,
            &version.name,
            version.file_name.as_str(),
            "download",
            index as u64 + 1,
            total,
            &format!("Processed {}", version.file_name),
        );
    }

    let root = root_result.ok_or_else(|| "The selected mod was not installed.".to_string())?;
    emit_mod_progress(
        sink,
        &root.name,
        &root.file_name,
        "complete",
        total,
        total,
        "Mod installation complete",
    );
    Ok(ModInstallResult {
        root,
        dependencies: dependency_results,
        skipped,
        warnings,
    })
}

pub fn list_installed_mods_blocking(game_dir: &str) -> Result<Vec<InstalledModInfo>, String> {
    let mods_dir = resolve_game_dir(game_dir).join("mods");
    if !mods_dir.exists() {
        return Ok(Vec::new());
    }
    let entries = fs::read_dir(&mods_dir).map_err(|error| {
        format!(
            "Cannot read mods directory ({}): {error}",
            mods_dir.display()
        )
    })?;
    let mut mods = Vec::new();
    for entry in entries.flatten() {
        let file_type = match entry.file_type() {
            Ok(value) => value,
            Err(_) => continue,
        };
        if !file_type.is_file() {
            continue;
        }
        let file_name = entry.file_name().to_string_lossy().to_string();
        if !is_mod_file_name(&file_name) || !validate_mod_file_name(&file_name).is_ok() {
            continue;
        }
        mods.push(parse_installed_mod(&mods_dir, &file_name));
    }
    mods.sort_by(|left, right| {
        left.name
            .to_ascii_lowercase()
            .cmp(&right.name.to_ascii_lowercase())
    });
    Ok(mods)
}

pub fn set_mod_enabled_blocking(
    game_dir: &str,
    file_name: &str,
    enabled: bool,
) -> Result<InstalledModInfo, String> {
    validate_mod_file_name(file_name)?;
    let mods_dir = resolve_game_dir(game_dir).join("mods");
    let source = mods_dir.join(file_name);
    ensure_regular_file(&source)?;
    let currently_enabled = file_name.to_ascii_lowercase().ends_with(".jar");
    if currently_enabled == enabled {
        return Ok(parse_installed_mod(&mods_dir, file_name));
    }

    let target_name = if enabled {
        file_name
            .strip_suffix(".disabled")
            .ok_or_else(|| "A disabled mod must end in .jar.disabled.".to_string())?
            .to_string()
    } else {
        if !file_name.to_ascii_lowercase().ends_with(".jar") {
            return Err("Only .jar mod files can be disabled.".to_string());
        }
        format!("{file_name}.disabled")
    };
    validate_mod_file_name(&target_name)?;
    let target = mods_dir.join(&target_name);
    if target.exists() {
        return Err(format!(
            "Cannot change mod state because another file already exists: {}",
            target.display()
        ));
    }
    fs::rename(&source, &target).map_err(|error| {
        format!(
            "Cannot change mod state ({} -> {}): {error}",
            source.display(),
            target.display()
        )
    })?;
    Ok(parse_installed_mod(&mods_dir, &target_name))
}

pub fn delete_mod_blocking(game_dir: &str, file_name: &str) -> Result<(), String> {
    validate_mod_file_name(file_name)?;
    let mods_dir = resolve_game_dir(game_dir).join("mods");
    let path = mods_dir.join(file_name);
    ensure_regular_file(&path)?;
    if !is_mod_file_name(file_name) {
        return Err("The requested file is not a managed mod JAR.".to_string());
    }
    fs::remove_file(&path)
        .map_err(|error| format!("Cannot delete mod ({}): {error}", path.display()))?;
    let sidecar = sidecar_path(&mods_dir, file_name);
    if let Ok(sidecar) = sidecar {
        if sidecar.exists() {
            let _ = fs::remove_file(sidecar);
        }
    }
    Ok(())
}

fn validate_search_inputs(
    query: &str,
    game_version: &str,
    offset: u32,
    limit: u32,
) -> Result<(), String> {
    if query.chars().count() > 200 {
        return Err("The mod search query is too long.".to_string());
    }
    validate_filter_text(game_version, "game version")?;
    if offset > 100_000 {
        return Err("The mod search offset is too large.".to_string());
    }
    if !(1..=50).contains(&limit) {
        return Err("Mod search limit must be between 1 and 50.".to_string());
    }
    Ok(())
}

fn validate_filter_text(value: &str, label: &str) -> Result<(), String> {
    let value = value.trim();
    if value.is_empty() || value.len() > 64 || value.chars().any(char::is_control) {
        return Err(format!("The {label} filter is invalid."));
    }
    Ok(())
}

fn validate_mod_loader(loader: &str) -> Result<&str, String> {
    match loader {
        "fabric" | "forge" | "neoforge" | "quilt" => Ok(loader),
        _ => Err(format!(
            "Unsupported mod loader '{loader}'. Supported loaders are fabric, forge, neoforge, and quilt."
        )),
    }
}

fn require_curseforge_key() -> Result<String, String> {
    let settings = settings::load_settings()?;
    let key = settings.curseforge_api_key.trim().to_string();
    if key.is_empty() {
        return Err(
            "CurseForge requires an API key. Add one in Settings before searching or installing."
                .to_string(),
        );
    }
    Ok(key)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CredentialKind {
    None,
    CurseForgeApiKey,
}

struct SourceBinding {
    requirement: ProviderRequirement,
    credential: CredentialKind,
}

fn source_binding(source: ModSource) -> SourceBinding {
    match source {
        ModSource::Modrinth => SourceBinding {
            requirement: mod_provider_requirement(Some(MODRINTH_SELECTION_KEY)),
            credential: CredentialKind::None,
        },
        ModSource::Curseforge => SourceBinding {
            requirement: mod_provider_requirement(Some(CURSEFORGE_SELECTION_KEY)),
            credential: CredentialKind::CurseForgeApiKey,
        },
    }
}

fn load_credential(kind: CredentialKind) -> Result<Option<String>, String> {
    match kind {
        CredentialKind::None => Ok(None),
        CredentialKind::CurseForgeApiKey => require_curseforge_key().map(Some),
    }
}

struct SelectedModProvider<'a> {
    provider: &'a dyn ModProvider,
    context: ModProviderContext<'a>,
}

fn select_for_source<'a>(
    registry: &'a ProviderRegistry<dyn ModProvider>,
    source: ModSource,
    network: &'a dyn NetworkPort,
    timeout: Duration,
) -> Result<SelectedModProvider<'a>, String> {
    let binding = source_binding(source);
    let credential = load_credential(binding.credential)?;
    let provider = registry.select(&binding.requirement).map_err(|error| {
        format!(
            "Cannot select the {} mod provider: {error}",
            source.as_str()
        )
    })?;
    let context = ModProviderContext::new(
        network,
        credential.map(ScopedCredential::new),
        &NOOP_PROGRESS,
        &NOOP_LOGGER,
        timeout,
        Deadline::after(Duration::from_secs(3_600)),
        Cancellation::new(),
    );
    Ok(SelectedModProvider { provider, context })
}

fn mod_search_page_from_data(data: ModSearchResultData, source: ModSource) -> ModSearchPage {
    ModSearchPage {
        source,
        total: data.total,
        offset: data.offset,
        limit: data.limit,
        hits: data
            .hits
            .into_iter()
            .map(|project| mod_project_from_data(project, source))
            .collect(),
    }
}

fn mod_project_from_data(data: ModProjectData, source: ModSource) -> ModProject {
    ModProject {
        id: data.id,
        source,
        slug: data.slug,
        name: data.name,
        summary: data.summary,
        author: data.author,
        downloads: data.downloads,
        icon_url: data.icon_url,
        categories: data.categories,
        project_url: data.project_url,
    }
}

fn mod_version_from_data(data: ModVersionData, source: ModSource) -> ModVersionInfo {
    ModVersionInfo {
        id: data.id,
        source,
        project_id: data.project_id,
        name: data.name,
        version_number: data.version_number,
        file_name: data.file_name,
        download_url: data.download_url,
        game_versions: data.game_versions,
        loaders: data.loaders,
        release_type: data.release_type,
        published_at: data.published_at,
        dependencies: data
            .dependencies
            .into_iter()
            .map(|dependency| mod_dependency_from_data(dependency, source))
            .collect(),
    }
}

fn mod_dependency_from_data(data: ModDependencyData, source: ModSource) -> ModDependencyRef {
    ModDependencyRef {
        source,
        project_id: data.project_id,
        version_id: data.version_id,
        name: data.name,
        dependency_type: mod_dependency_kind_to_host(data.dependency_type),
    }
}

fn mod_dependency_to_data(dependency: &ModDependencyRef) -> ModDependencyData {
    ModDependencyData {
        project_id: dependency.project_id.clone(),
        version_id: dependency.version_id.clone(),
        name: dependency.name.clone(),
        dependency_type: mod_dependency_kind_from_host(dependency.dependency_type),
    }
}

fn mod_dependency_kind_to_host(kind: ModDependencyKind) -> ModDependencyType {
    match kind {
        ModDependencyKind::Required => ModDependencyType::Required,
        ModDependencyKind::Optional => ModDependencyType::Optional,
        ModDependencyKind::Embedded => ModDependencyType::Embedded,
        ModDependencyKind::Incompatible => ModDependencyType::Incompatible,
    }
}

fn mod_dependency_kind_from_host(kind: ModDependencyType) -> ModDependencyKind {
    match kind {
        ModDependencyType::Required => ModDependencyKind::Required,
        ModDependencyType::Optional => ModDependencyKind::Optional,
        ModDependencyType::Embedded => ModDependencyKind::Embedded,
        ModDependencyType::Incompatible => ModDependencyKind::Incompatible,
    }
}

fn load_root_version(
    provider: &dyn ModProvider,
    context: &ModProviderContext<'_>,
    source: ModSource,
    project_id: &str,
    version_id: &str,
    game_version: &str,
    loader: &str,
) -> Result<ModVersionInfo, String> {
    let version_id = version_id.trim();
    let versions = if version_id.is_empty() {
        provider
            .list_versions(
                context,
                &ModListVersionsRequest {
                    project_id: project_id.to_string(),
                    game_version: game_version.to_string(),
                    loader: loader.to_string(),
                },
            )?
            .into_iter()
            .map(|version| mod_version_from_data(version, source))
            .collect()
    } else {
        let version = provider.version_by_id(
            context,
            &ModVersionRequest {
                project_id: project_id.to_string(),
                version_id: version_id.to_string(),
                game_version: game_version.to_string(),
                loader: loader.to_string(),
            },
        )?;
        vec![mod_version_from_data(version, source)]
    };
    versions
        .into_iter()
        .find(|version| {
            version.download_url.is_some()
                && (game_version.is_empty()
                    || version.game_versions.is_empty()
                    || version
                        .game_versions
                        .iter()
                        .any(|candidate| candidate.eq_ignore_ascii_case(game_version)))
                && (version.loaders.is_empty()
                    || version
                        .loaders
                        .iter()
                        .any(|candidate| candidate.eq_ignore_ascii_case(loader)))
        })
        .or_else(|| {
            if version_id.is_empty() {
                return None;
            }
            None
        })
        .ok_or_else(|| {
            "No compatible downloadable file was found for the selected mod version.".to_string()
        })
}

fn build_dependency_plan(
    provider: &dyn ModProvider,
    context: &ModProviderContext<'_>,
    source: ModSource,
    root: ModVersionInfo,
    game_version: &str,
    loader: &str,
) -> Result<ModDependencyPlan, String> {
    if root.download_url.is_none() {
        return Err("The selected mod version has no safe HTTPS download URL.".to_string());
    }
    resolve_dependency_plan_with(root, |dependency| {
        let data = provider.dependency_version(
            context,
            &ModDependencyVersionRequest {
                dependency: mod_dependency_to_data(dependency),
                game_version: game_version.to_string(),
                loader: loader.to_string(),
            },
        )?;
        Ok(data.map(|version| mod_version_from_data(version, source)))
    })
}

fn resolve_dependency_plan_with<F>(
    root: ModVersionInfo,
    mut fetch: F,
) -> Result<ModDependencyPlan, String>
where
    F: FnMut(&ModDependencyRef) -> Result<Option<ModVersionInfo>, String>,
{
    let mut plan = ModDependencyPlan {
        root: root.clone(),
        required: Vec::new(),
        optional: Vec::new(),
        conflicts: Vec::new(),
        warnings: Vec::new(),
    };
    let mut context = DependencyContext {
        nodes: 1,
        seen_projects: HashSet::new(),
        path: HashSet::new(),
    };
    let root_key = version_project_key(&root);
    context.seen_projects.insert(root_key.clone());
    context.path.insert(version_identity(&root));
    visit_dependencies(&root, 0, &mut context, &mut fetch, &mut plan)?;
    Ok(plan)
}

struct DependencyContext {
    nodes: usize,
    seen_projects: HashSet<String>,
    path: HashSet<String>,
}

fn visit_dependencies<F>(
    version: &ModVersionInfo,
    depth: usize,
    context: &mut DependencyContext,
    fetch: &mut F,
    plan: &mut ModDependencyPlan,
) -> Result<(), String>
where
    F: FnMut(&ModDependencyRef) -> Result<Option<ModVersionInfo>, String>,
{
    if depth > MAX_DEPENDENCY_DEPTH {
        return Err(format!(
            "Dependency graph exceeds the maximum depth of {MAX_DEPENDENCY_DEPTH}."
        ));
    }
    for dependency in &version.dependencies {
        match dependency.dependency_type {
            ModDependencyType::Required => {
                let identity = dependency_identity(dependency);
                if context.path.contains(&identity) {
                    plan.warnings.push(format!(
                        "Dependency cycle detected at {}; the repeated dependency was skipped.",
                        dependency.name
                    ));
                    continue;
                }
                let project_key = dependency_project_key(dependency);
                if context.seen_projects.contains(&project_key) {
                    plan.warnings.push(format!(
                        "Dependency {} is already present in the resolved graph.",
                        dependency.name
                    ));
                    continue;
                }
                context.nodes += 1;
                if context.nodes > MAX_DEPENDENCY_NODES {
                    return Err(format!(
                        "Dependency graph exceeds the maximum of {MAX_DEPENDENCY_NODES} nodes."
                    ));
                }
                let Some(required) = fetch(dependency)? else {
                    return Err(format!(
                        "Required dependency {} has no compatible downloadable file.",
                        dependency.name
                    ));
                };
                if required.download_url.is_none() {
                    return Err(format!(
                        "Required dependency {} has no safe HTTPS download URL.",
                        dependency.name
                    ));
                }
                let required_project_key = version_project_key(&required);
                if context.seen_projects.contains(&required_project_key) {
                    plan.warnings.push(format!(
                        "Dependency {} resolves to a project already present in the graph.",
                        dependency.name
                    ));
                    continue;
                }
                context.seen_projects.insert(required_project_key);
                context.path.insert(version_identity(&required));
                plan.required.push(required.clone());
                visit_dependencies(&required, depth + 1, context, fetch, plan)?;
                context.path.remove(&version_identity(&required));
            }
            ModDependencyType::Optional => plan.optional.push(dependency.clone()),
            ModDependencyType::Embedded => plan.warnings.push(format!(
                "Embedded dependency {} is bundled and was not installed separately.",
                dependency.name
            )),
            ModDependencyType::Incompatible => plan.conflicts.push(format!(
                "{} is marked incompatible with {}.",
                dependency.name, version.name
            )),
        }
    }
    Ok(())
}

fn version_identity(version: &ModVersionInfo) -> String {
    format!(
        "{}:{}:{}",
        version.source.as_str(),
        version.project_id,
        version.id
    )
}

fn dependency_identity(dependency: &ModDependencyRef) -> String {
    format!(
        "{}:{}:{}",
        dependency.source.as_str(),
        dependency.project_id,
        dependency.version_id.as_deref().unwrap_or_default()
    )
}

fn version_project_key(version: &ModVersionInfo) -> String {
    if version.project_id.is_empty() {
        version.id.clone()
    } else {
        format!("{}:{}", version.source.as_str(), version.project_id)
    }
}

fn dependency_project_key(dependency: &ModDependencyRef) -> String {
    if dependency.project_id.is_empty() {
        format!(
            "{}:{}",
            dependency.source.as_str(),
            dependency.version_id.as_deref().unwrap_or_default()
        )
    } else {
        format!("{}:{}", dependency.source.as_str(), dependency.project_id)
    }
}
fn mod_download_service_label(file_name: &str) -> Result<ServiceLabel, String> {
    let sanitized = file_name
        .chars()
        .take(80)
        .map(|character| {
            if character.is_ascii_alphanumeric()
                || matches!(character, ' ' | '-' | '_' | '.')
            {
                character
            } else {
                '-'
            }
        })
        .collect::<String>();
    let sanitized = sanitized.trim();
    let label = if sanitized.is_empty() {
        "Mod download"
    } else {
        sanitized
    };
    ServiceLabel::new(label).map_err(|error| format!("Cannot initialize mod download: {error}"))
}

fn install_single_mod(
    network: &dyn NetworkPort,
    mods_dir: &Path,
    version: &ModVersionInfo,
    requested_loader: &str,
    timeout: Duration,
) -> Result<(InstalledModInfo, bool), String> {
    validate_mod_file_name(&version.file_name)?;
    if !version.file_name.to_ascii_lowercase().ends_with(".jar") {
        return Err(format!(
            "The provider file is not a JAR: {}",
            version.file_name
        ));
    }
    let download_url = version
        .download_url
        .as_deref()
        .ok_or_else(|| format!("{} has no safe download URL.", version.file_name))?;
    net::validate_https_url(download_url)?;
    let sidecar = read_sidecar(mods_dir, &version.file_name).ok().flatten();
    if let Some(sidecar) = sidecar.as_ref() {
        if sidecar.source == Some(version.source)
            && sidecar.project_id.as_deref() == Some(version.project_id.as_str())
            && sidecar.version_id.as_deref() == Some(version.id.as_str())
        {
            if let Some(existing_name) = existing_mod_name(mods_dir, &version.file_name) {
                return Ok((parse_installed_mod(mods_dir, &existing_name), true));
            }
        }
    }

    let destination = mods_dir.join(&version.file_name);
    if destination.exists() {
        return Err(format!(
            "A mod file named {} already exists. Remove it or choose another version.",
            version.file_name
        ));
    }
    let service = mod_download_service_label(&version.file_name)?;
    let request = DownloadRequest::new(
        download_url.to_string(),
        destination.clone(),
        None,
        service,
        MAX_MOD_DOWNLOAD,
        timeout,
    );
    network.download_atomic(&request)?;
    if let Err(error) = validate_downloaded_mod_archive(&destination) {
        let _ = fs::remove_file(&destination);
        return Err(error);
    }

    let sidecar_value = ModSidecar {
        source: Some(version.source),
        project_id: Some(version.project_id.clone()),
        version_id: Some(version.id.clone()),
        project_name: Some(version.name.clone()),
        loader: Some(requested_loader.to_string()),
    };
    if let Err(error) = write_sidecar(mods_dir, &version.file_name, &sidecar_value) {
        let _ = fs::remove_file(&destination);
        return Err(error);
    }

    let mut installed = parse_installed_mod(mods_dir, &version.file_name);
    if installed.source.is_none() {
        installed.source = Some(version.source);
    }
    if installed.project_id.is_none() {
        installed.project_id = Some(version.project_id.clone());
    }
    if installed.version_id.is_none() {
        installed.version_id = Some(version.id.clone());
    }
    if installed.loader.is_none() {
        installed.loader = Some(requested_loader.to_string());
    }
    Ok((installed, false))
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct ModSidecar {
    #[serde(default)]
    source: Option<ModSource>,
    #[serde(default)]
    project_id: Option<String>,
    #[serde(default)]
    version_id: Option<String>,
    #[serde(default)]
    project_name: Option<String>,
    #[serde(default)]
    loader: Option<String>,
}

#[derive(Debug, Clone, Default)]
struct ModMetadata {
    name: Option<String>,
    mod_id: Option<String>,
    version: Option<String>,
    loader: Option<String>,
}

fn parse_installed_mod(mods_dir: &Path, file_name: &str) -> InstalledModInfo {
    let path = mods_dir.join(file_name);
    let enabled = file_name.to_ascii_lowercase().ends_with(".jar");
    let sidecar = read_sidecar(mods_dir, file_name).ok().flatten();
    let metadata = if path.is_file() {
        parse_mod_jar_metadata(&path)
    } else {
        ModMetadata::default()
    };
    let fallback_name = mod_display_stem(file_name);
    InstalledModInfo {
        file_name: file_name.to_string(),
        name: metadata
            .name
            .clone()
            .or_else(|| {
                sidecar
                    .as_ref()
                    .and_then(|value| value.project_name.clone())
            })
            .unwrap_or(fallback_name),
        mod_id: metadata.mod_id,
        version: metadata.version,
        loader: metadata
            .loader
            .or_else(|| sidecar.as_ref().and_then(|value| value.loader.clone())),
        enabled,
        path: path.display().to_string(),
        source: sidecar.as_ref().and_then(|value| value.source),
        project_id: sidecar.as_ref().and_then(|value| value.project_id.clone()),
        version_id: sidecar.as_ref().and_then(|value| value.version_id.clone()),
    }
}

fn validate_downloaded_mod_archive(path: &Path) -> Result<(), String> {
    validate_downloaded_mod_archive_with_limits(path, MAX_MOD_ARCHIVE_ENTRIES)
}

fn validate_downloaded_mod_archive_with_limits(
    path: &Path,
    max_entries: usize,
) -> Result<(), String> {
    let file = fs::File::open(path)
        .map_err(|error| format!("Cannot open downloaded mod archive: {error}"))?;
    let archive = ZipArchive::new(file)
        .map_err(|_| "The provider download is not a valid JAR/ZIP archive.".to_string())?;
    if archive.len() > max_entries {
        return Err("The downloaded mod archive contains too many entries.".to_string());
    }
    Ok(())
}

fn parse_mod_jar_metadata(path: &Path) -> ModMetadata {
    let file = match fs::File::open(path) {
        Ok(value) => value,
        Err(_) => return ModMetadata::default(),
    };
    let mut archive = match ZipArchive::new(file) {
        Ok(value) => value,
        Err(_) => return ModMetadata::default(),
    };
    if archive.len() > MAX_MOD_ARCHIVE_ENTRIES {
        return ModMetadata::default();
    }

    if let Some(text) = read_zip_text(&mut archive, "fabric.mod.json") {
        if let Ok(value) = serde_json::from_str::<Value>(&text) {
            return ModMetadata {
                name: value
                    .get("name")
                    .and_then(Value::as_str)
                    .map(|value| value.to_string()),
                mod_id: value
                    .get("id")
                    .and_then(Value::as_str)
                    .map(|value| value.to_string()),
                version: value
                    .get("version")
                    .and_then(Value::as_str)
                    .map(|value| value.to_string()),
                loader: Some("fabric".to_string()),
            };
        }
    }

    if let Some(text) = read_zip_text(&mut archive, "quilt.mod.json") {
        if let Ok(value) = serde_json::from_str::<Value>(&text) {
            let loader = value.get("quilt_loader").unwrap_or(&Value::Null);
            return ModMetadata {
                name: loader
                    .get("metadata")
                    .and_then(|metadata| metadata.get("name"))
                    .and_then(Value::as_str)
                    .map(|value| value.to_string()),
                mod_id: loader
                    .get("id")
                    .and_then(Value::as_str)
                    .map(|value| value.to_string()),
                version: loader
                    .get("version")
                    .and_then(Value::as_str)
                    .map(|value| value.to_string()),
                loader: Some("quilt".to_string()),
            };
        }
    }

    if let Some(text) = read_zip_text(&mut archive, "META-INF/neoforge.mods.toml") {
        let metadata = parse_forge_mods_toml(&text, "neoforge");
        if metadata.mod_id.is_some() || metadata.name.is_some() {
            return metadata;
        }
    }
    if let Some(text) = read_zip_text(&mut archive, "META-INF/mods.toml") {
        let metadata = parse_forge_mods_toml(&text, "forge");
        if metadata.mod_id.is_some() || metadata.name.is_some() {
            return metadata;
        }
    }

    if let Some(text) = read_zip_text(&mut archive, "mcmod.info") {
        if let Ok(value) = serde_json::from_str::<Value>(&text) {
            let entry = value
                .as_array()
                .and_then(|array| array.first())
                .or_else(|| {
                    value
                        .get("modList")
                        .and_then(Value::as_array)
                        .and_then(|array| array.first())
                })
                .unwrap_or(&value);
            return ModMetadata {
                name: entry
                    .get("name")
                    .and_then(Value::as_str)
                    .map(|value| value.to_string()),
                mod_id: entry
                    .get("modid")
                    .and_then(Value::as_str)
                    .map(|value| value.to_string()),
                version: entry
                    .get("version")
                    .and_then(Value::as_str)
                    .map(|value| value.to_string()),
                loader: Some("forge".to_string()),
            };
        }
    }

    ModMetadata::default()
}

fn read_zip_text(archive: &mut ZipArchive<fs::File>, name: &str) -> Option<String> {
    let mut entry = archive.by_name(name).ok()?;
    if entry.size() > MAX_MOD_METADATA_ENTRY {
        return None;
    }
    let mut bytes = Vec::new();
    let limit = MAX_MOD_METADATA_ENTRY + 1;
    entry.by_ref().take(limit).read_to_end(&mut bytes).ok()?;
    if bytes.len() as u64 > MAX_MOD_METADATA_ENTRY {
        return None;
    }
    String::from_utf8(bytes).ok()
}

fn parse_forge_mods_toml(text: &str, loader: &str) -> ModMetadata {
    let mod_id = toml_string_field(text, "modId");
    let display_name = toml_string_field(text, "displayName");
    let version = toml_string_field(text, "version");
    ModMetadata {
        name: display_name.or_else(|| mod_id.clone()),
        mod_id,
        version,
        loader: Some(loader.to_string()),
    }
}

fn toml_string_field(text: &str, key: &str) -> Option<String> {
    let pattern = format!(r#"(?m)^\s*{key}\s*=\s*"([^"]*)""#);
    let regex = Regex::new(&pattern).ok()?;
    regex
        .captures(text)
        .and_then(|capture| capture.get(1))
        .map(|value| value.as_str().trim().to_string())
        .filter(|value| !value.is_empty())
}

fn read_sidecar(mods_dir: &Path, file_name: &str) -> Result<Option<ModSidecar>, String> {
    let path = sidecar_path(mods_dir, file_name)?;
    let metadata = match fs::symlink_metadata(&path) {
        Ok(value) => value,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("Cannot read mod sidecar metadata: {error}")),
    };
    if !metadata.file_type().is_file() || metadata.len() > 1024 * 1024 {
        return Ok(None);
    }
    let bytes =
        fs::read(&path).map_err(|error| format!("Cannot read mod sidecar metadata: {error}"))?;
    let sidecar = serde_json::from_slice(&bytes).unwrap_or_default();
    Ok(Some(sidecar))
}

fn write_sidecar(mods_dir: &Path, file_name: &str, sidecar: &ModSidecar) -> Result<(), String> {
    let path = sidecar_path(mods_dir, file_name)?;
    let bytes = serde_json::to_vec_pretty(sidecar)
        .map_err(|error| format!("Cannot serialize mod sidecar metadata: {error}"))?;
    let temp = net::temp_path_near(&path, "tmp");
    fs::write(&temp, bytes)
        .map_err(|error| format!("Cannot write mod sidecar metadata: {error}"))?;
    match fs::rename(&temp, &path) {
        Ok(()) => Ok(()),
        Err(first_error) => {
            let _ = fs::remove_file(&path);
            fs::rename(&temp, &path).map_err(|error| {
                let _ = fs::remove_file(&temp);
                format!("Cannot save mod sidecar metadata ({first_error}; {error})")
            })
        }
    }
}

fn sidecar_path(mods_dir: &Path, file_name: &str) -> Result<PathBuf, String> {
    validate_mod_file_name(file_name)?;
    let base_name = if file_name.to_ascii_lowercase().ends_with(".jar.disabled") {
        file_name
            .get(..file_name.len().saturating_sub(".disabled".len()))
            .ok_or_else(|| "Invalid disabled mod file name.".to_string())?
    } else {
        file_name
    };
    Ok(mods_dir.join(format!("{base_name}.flint.json")))
}

fn existing_mod_name(mods_dir: &Path, file_name: &str) -> Option<String> {
    let enabled = mods_dir.join(file_name);
    if enabled.is_file() {
        return Some(file_name.to_string());
    }
    let disabled = mods_dir.join(format!("{file_name}.disabled"));
    if disabled.is_file() {
        return Some(format!("{file_name}.disabled"));
    }
    None
}

fn validate_mod_file_name(file_name: &str) -> Result<(), String> {
    if file_name.is_empty()
        || file_name.len() > 220
        || file_name == "."
        || file_name == ".."
        || file_name.ends_with('.')
        || file_name.ends_with(' ')
        || file_name.chars().any(|character| {
            character.is_control()
                || matches!(
                    character,
                    '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|'
                )
        })
    {
        return Err("The mod file name is unsafe.".to_string());
    }
    if !is_mod_file_name(file_name) {
        return Err("Only .jar and .jar.disabled files are managed as mods.".to_string());
    }
    Ok(())
}

fn is_mod_file_name(file_name: &str) -> bool {
    let lower = file_name.to_ascii_lowercase();
    lower.ends_with(".jar") || lower.ends_with(".jar.disabled")
}

fn mod_display_stem(file_name: &str) -> String {
    let lower = file_name.to_ascii_lowercase();
    if lower.ends_with(".jar.disabled") {
        &file_name[..file_name.len() - ".jar.disabled".len()]
    } else if lower.ends_with(".jar") {
        &file_name[..file_name.len() - ".jar".len()]
    } else {
        file_name
    }
    .to_string()
}

fn ensure_regular_file(path: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("Cannot access mod file ({}): {error}", path.display()))?;
    if !metadata.file_type().is_file() {
        return Err("Refusing to operate on a symlink or non-file mod path.".to_string());
    }
    Ok(())
}

fn emit_mod_progress(
    sink: &dyn ModInstallProgressSink,
    project_name: &str,
    file_name: &str,
    stage: &str,
    completed: u64,
    total: u64,
    message: &str,
) {
    sink.report(ModInstallProgress {
        project_name: project_name.to_string(),
        file_name: file_name.to_string(),
        stage: stage.to_string(),
        completed,
        total,
        message: sanitize_progress_message(message),
    });
}
#[cfg(test)]
mod tests {
    use super::*;

    fn dependency(
        project_id: &str,
        version_id: &str,
        dependency_type: ModDependencyType,
    ) -> ModDependencyRef {
        ModDependencyRef {
            source: ModSource::Modrinth,
            project_id: project_id.to_string(),
            version_id: if version_id.is_empty() {
                None
            } else {
                Some(version_id.to_string())
            },
            name: format!("Dependency {project_id}"),
            dependency_type,
        }
    }

    fn version(id: &str, project_id: &str, dependencies: Vec<ModDependencyRef>) -> ModVersionInfo {
        ModVersionInfo {
            id: id.to_string(),
            source: ModSource::Modrinth,
            project_id: project_id.to_string(),
            name: format!("Project {project_id}"),
            version_number: "1.0.0".to_string(),
            file_name: format!("{project_id}-{id}.jar"),
            download_url: Some(format!("https://example.com/{project_id}-{id}.jar")),
            game_versions: vec!["1.20.1".to_string()],
            loaders: vec!["fabric".to_string()],
            release_type: "release".to_string(),
            published_at: None,
            dependencies,
        }
    }

    #[test]
    fn dependency_graph_detects_cycles_and_reports_optional_conflicts() {
        let root = version(
            "rootver",
            "root",
            vec![
                dependency("dep-a", "a1", ModDependencyType::Required),
                dependency("optional", "", ModDependencyType::Optional),
                dependency("bad", "", ModDependencyType::Incompatible),
            ],
        );
        let mut calls = 0;
        let plan = resolve_dependency_plan_with(root, |dep| {
            calls += 1;
            Ok(Some(version(
                "a1",
                &dep.project_id,
                vec![dependency("root", "rootver", ModDependencyType::Required)],
            )))
        })
        .expect("dependency plan");
        assert_eq!(calls, 1);
        assert_eq!(plan.required.len(), 1);
        assert_eq!(plan.optional.len(), 1);
        assert_eq!(plan.conflicts.len(), 1);
        assert!(plan
            .warnings
            .iter()
            .any(|warning| warning.contains("cycle")));
    }

    #[test]
    fn dependency_graph_enforces_depth_limit() {
        let root = version(
            "rootver",
            "root",
            vec![dependency("dep-0", "v0", ModDependencyType::Required)],
        );
        let mut next = 0usize;
        let result = resolve_dependency_plan_with(root, |_dep| {
            let current = next;
            next += 1;
            Ok(Some(version(
                &format!("v{current}"),
                &format!("dep-{current}"),
                vec![dependency(
                    &format!("dep-{}", current + 1),
                    &format!("v{}", current + 1),
                    ModDependencyType::Required,
                )],
            )))
        });
        assert!(result.is_err());
        assert!(result.err().unwrap_or_default().contains("maximum depth"));
    }

    #[test]
    fn dependency_graph_enforces_node_limit() {
        let dependencies = (0..(MAX_DEPENDENCY_NODES + 3))
            .map(|index| {
                dependency(
                    &format!("dep-{index}"),
                    &format!("v{index}"),
                    ModDependencyType::Required,
                )
            })
            .collect();
        let root = version("rootver", "root", dependencies);
        let result = resolve_dependency_plan_with(root, |dep| {
            Ok(Some(version(
                &format!("v{}", dep.project_id),
                &dep.project_id,
                Vec::new(),
            )))
        });
        assert!(result.is_err());
        assert!(result
            .err()
            .unwrap_or_default()
            .contains("maximum of 64 nodes"));
    }

    #[test]
    fn required_dependency_without_compatible_file_is_hard_error() {
        let root = version(
            "rootver",
            "root",
            vec![dependency("missing", "", ModDependencyType::Required)],
        );
        let result = resolve_dependency_plan_with(root, |_dependency| Ok(None));
        assert!(result.is_err());
        assert!(result
            .err()
            .unwrap_or_default()
            .contains("no compatible downloadable file"));
    }

    #[test]
    fn download_service_labels_keep_a_sanitized_mod_file_identity() {
        let label = mod_download_service_label("example+mod 1.0.jar").expect("service label");
        assert_eq!(label.as_str(), "example-mod 1.0.jar");
    }

    #[test]
    fn parses_forge_and_neoforge_metadata() {
        let text = r#"
            modLoader="javafml"
            loaderVersion="[47,)"
            [[mods]]
            modId="examplemod"
            version="1.2.3"
            displayName="Example Mod"
        "#;
        let metadata = parse_forge_mods_toml(text, "forge");
        assert_eq!(metadata.mod_id.as_deref(), Some("examplemod"));
        assert_eq!(metadata.name.as_deref(), Some("Example Mod"));
        assert_eq!(metadata.version.as_deref(), Some("1.2.3"));
        assert_eq!(metadata.loader.as_deref(), Some("forge"));
    }

    #[test]
    fn validates_mod_sidecar_path_for_disabled_jars() {
        let path = sidecar_path(Path::new("mods"), "example.jar.disabled").expect("sidecar path");
        assert_eq!(path, Path::new("mods").join("example.jar.flint.json"));
    }

    #[test]
    fn mod_file_validation_rejects_traversal_and_non_jar_files() {
        assert!(validate_mod_file_name("example-mod-1.2.3.jar").is_ok());
        assert!(validate_mod_file_name("example-mod-1.2.3.jar.disabled").is_ok());
        assert!(validate_mod_file_name("../escape.jar").is_err());
        assert!(validate_mod_file_name("folder\\escape.jar").is_err());
        assert!(validate_mod_file_name("example.zip").is_err());
    }

}

#[cfg(test)]
mod additional_tests {
    use super::*;
    use std::io::Write;
    use zip::{write::SimpleFileOptions, ZipWriter};

    fn write_test_jar(path: &Path, entries: &[(&str, &[u8])]) {
        let file = fs::File::create(path).expect("create test jar");
        let mut writer = ZipWriter::new(file);
        for (name, contents) in entries {
            writer
                .start_file(*name, SimpleFileOptions::default())
                .expect("start jar entry");
            writer.write_all(contents).expect("write jar entry");
        }
        writer.finish().expect("finish test jar");
    }

    fn temp_mods_root(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "flint-{label}-{}-{}",
            std::process::id(),
            net::nonce()
        ))
    }

    #[test]
    fn maps_neutral_provider_data_to_host_source_fields() {
        let project = mod_project_from_data(
            ModProjectData {
                id: "project".to_string(),
                slug: "project".to_string(),
                name: "Project".to_string(),
                summary: "Summary".to_string(),
                author: "Author".to_string(),
                downloads: 1,
                icon_url: None,
                categories: vec!["utility".to_string()],
                project_url: "https://example.com/project".to_string(),
            },
            ModSource::Curseforge,
        );
        assert_eq!(project.source, ModSource::Curseforge);

        let version = mod_version_from_data(
            ModVersionData {
                id: "version".to_string(),
                project_id: "project".to_string(),
                name: "Version".to_string(),
                version_number: "1.0.0".to_string(),
                file_name: "mod.jar".to_string(),
                download_url: None,
                game_versions: vec!["1.20.1".to_string()],
                loaders: vec!["fabric".to_string()],
                release_type: "release".to_string(),
                published_at: None,
                dependencies: vec![ModDependencyData {
                    project_id: "dependency".to_string(),
                    version_id: None,
                    name: "Dependency".to_string(),
                    dependency_type: ModDependencyKind::Required,
                }],
            },
            ModSource::Modrinth,
        );
        assert_eq!(version.source, ModSource::Modrinth);
        assert_eq!(version.dependencies[0].source, ModSource::Modrinth);
        assert_eq!(
            version.dependencies[0].dependency_type,
            ModDependencyType::Required
        );
        for dependency_type in [
            ModDependencyType::Required,
            ModDependencyType::Optional,
            ModDependencyType::Embedded,
            ModDependencyType::Incompatible,
        ] {
            assert_eq!(
                mod_dependency_kind_to_host(mod_dependency_kind_from_host(dependency_type)),
                dependency_type
            );
        }
    }

    #[test]
    fn source_binding_has_one_requirement_and_credential_kind_per_source() {
        let curseforge = source_binding(ModSource::Curseforge);
        assert_eq!(
            curseforge.requirement.selection_key,
            Some(CURSEFORGE_SELECTION_KEY)
        );
        assert_eq!(curseforge.credential, CredentialKind::CurseForgeApiKey);

        let modrinth = source_binding(ModSource::Modrinth);
        assert_eq!(
            modrinth.requirement.selection_key,
            Some(MODRINTH_SELECTION_KEY)
        );
        assert_eq!(modrinth.credential, CredentialKind::None);
    }

    #[test]
    fn parses_fabric_quilt_forge_and_legacy_jar_metadata() {
        let root = temp_mods_root("mod-metadata");
        fs::create_dir_all(&root).expect("temp root");

        let fabric = root.join("fabric.jar");
        write_test_jar(
            &fabric,
            &[(
                "fabric.mod.json",
                br#"{"id":"fabric_example","name":"Fabric Example","version":"1.2.3"}"#,
            )],
        );
        let metadata = parse_mod_jar_metadata(&fabric);
        assert_eq!(metadata.mod_id.as_deref(), Some("fabric_example"));
        assert_eq!(metadata.name.as_deref(), Some("Fabric Example"));
        assert_eq!(metadata.loader.as_deref(), Some("fabric"));

        let quilt = root.join("quilt.jar");
        write_test_jar(
            &quilt,
            &[(
                "quilt.mod.json",
                br#"{"quilt_loader":{"id":"quilt_example","version":"2.0.0","metadata":{"name":"Quilt Example"}}}"#,
            )],
        );
        let metadata = parse_mod_jar_metadata(&quilt);
        assert_eq!(metadata.mod_id.as_deref(), Some("quilt_example"));
        assert_eq!(metadata.loader.as_deref(), Some("quilt"));

        let forge = root.join("forge.jar");
        write_test_jar(
            &forge,
            &[(
                "META-INF/mods.toml",
                b"[[mods]]\nmodId=\"forge_example\"\ndisplayName=\"Forge Example\"\nversion=\"3.0.0\"\n",
            )],
        );
        let metadata = parse_mod_jar_metadata(&forge);
        assert_eq!(metadata.mod_id.as_deref(), Some("forge_example"));
        assert_eq!(metadata.loader.as_deref(), Some("forge"));

        let legacy = root.join("legacy.jar");
        write_test_jar(
            &legacy,
            &[(
                "mcmod.info",
                br#"[{"modid":"legacy_example","name":"Legacy Example","version":"4.0.0"}]"#,
            )],
        );
        let metadata = parse_mod_jar_metadata(&legacy);
        assert_eq!(metadata.mod_id.as_deref(), Some("legacy_example"));
        assert_eq!(metadata.loader.as_deref(), Some("forge"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_non_archive_provider_downloads() {
        let root = temp_mods_root("mod-download-validation");
        fs::create_dir_all(&root).expect("temp root");
        let path = root.join("bad.jar");
        fs::write(&path, b"<html>provider error</html>").expect("write invalid download");
        let error = validate_downloaded_mod_archive(&path).expect_err("reject non-archive");
        assert!(error.contains("not a valid JAR/ZIP archive"));

        let limited_jar = root.join("limited.jar");
        write_test_jar(
            &limited_jar,
            &[
                ("fabric.mod.json", b"{}"),
                ("nested/extra.bin", b"extra"),
            ],
        );
        let entry_error = validate_downloaded_mod_archive_with_limits(&limited_jar, 1)
            .expect_err("entry-count cap must be enforced while scanning");
        assert!(entry_error.contains("too many entries"));
        assert!(!root.join("fabric.mod.json").exists());
        assert!(!root.join("nested").exists());
        assert!(limited_jar.is_file());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn manages_enabled_disabled_and_deleted_mods_with_sidecars() {
        let root = temp_mods_root("mod-management");
        let mods_dir = root.join("mods");
        fs::create_dir_all(&mods_dir).expect("mods dir");
        let jar = mods_dir.join("example.jar");
        write_test_jar(
            &jar,
            &[(
                "fabric.mod.json",
                br#"{"id":"example_mod","name":"Example Mod","version":"1.0.0"}"#,
            )],
        );
        write_sidecar(
            &mods_dir,
            "example.jar",
            &ModSidecar {
                source: Some(ModSource::Modrinth),
                project_id: Some("project-id".to_string()),
                version_id: Some("version-id".to_string()),
                project_name: Some("Example Project".to_string()),
                loader: Some("fabric".to_string()),
            },
        )
        .expect("write sidecar");

        let game_dir = root.display().to_string();
        let listed = list_installed_mods_blocking(&game_dir).expect("list mods");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].name, "Example Mod");
        assert!(listed[0].enabled);
        assert_eq!(listed[0].source, Some(ModSource::Modrinth));

        let disabled =
            set_mod_enabled_blocking(&game_dir, "example.jar", false).expect("disable mod");
        assert!(!disabled.enabled);
        assert_eq!(disabled.file_name, "example.jar.disabled");
        assert!(mods_dir.join("example.jar.disabled").is_file());

        let enabled =
            set_mod_enabled_blocking(&game_dir, "example.jar.disabled", true).expect("enable mod");
        assert!(enabled.enabled);
        assert_eq!(enabled.file_name, "example.jar");

        delete_mod_blocking(&game_dir, "example.jar").expect("delete mod");
        assert!(!mods_dir.join("example.jar").exists());
        assert!(!sidecar_path(&mods_dir, "example.jar")
            .expect("sidecar path")
            .exists());
        let _ = fs::remove_dir_all(root);
    }
}

#[cfg(test)]
mod timeout_tests {
    use super::*;
    use flint_core::ports::{DownloadRequest, NetworkRequest};
    use flint_provider_api::{
        descriptor_for, ModDependencyVersionRequest, ModListVersionsRequest, ModProvider,
        ModProviderContext, ModSearchRequest, ModSearchResultData, ModVersionData, ModVersionRequest,
        Provider, ProviderDescriptor, ProviderRegistry, MOD_PROVIDER_API, MODRINTH_CAPABILITIES,
        MODRINTH_PROVIDER_ID,
    };
    use serde_json::Value;
    use std::sync::{Arc, Mutex};

    struct NoopNetwork;

    impl NetworkPort for NoopNetwork {
        fn read_json(&self, _request: &NetworkRequest) -> Result<Value, String> {
            Err("not used".to_string())
        }

        fn read_text(&self, _request: &NetworkRequest) -> Result<String, String> {
            Err("not used".to_string())
        }

        fn read_bytes(&self, _request: &NetworkRequest) -> Result<Vec<u8>, String> {
            Err("not used".to_string())
        }

        fn download_atomic(&self, _request: &DownloadRequest) -> Result<u64, String> {
            Err("not used".to_string())
        }
    }

    struct DownloadProbeNetwork {
        timeout: Mutex<Option<Duration>>,
    }

    impl NetworkPort for DownloadProbeNetwork {
        fn read_json(&self, _request: &NetworkRequest) -> Result<Value, String> {
            Err("not used".to_string())
        }

        fn read_text(&self, _request: &NetworkRequest) -> Result<String, String> {
            Err("not used".to_string())
        }

        fn read_bytes(&self, _request: &NetworkRequest) -> Result<Vec<u8>, String> {
            Err("not used".to_string())
        }

        fn download_atomic(&self, request: &DownloadRequest) -> Result<u64, String> {
            *self.timeout.lock().expect("download timeout lock") = Some(request.timeout);
            Err("timeout probe download".to_string())
        }
    }

    struct TimeoutProbeProvider {
        timeouts: Arc<Mutex<Vec<Duration>>>,
    }

    impl TimeoutProbeProvider {
        fn record(&self, context: &ModProviderContext<'_>) {
            self.timeouts
                .lock()
                .expect("timeout probe lock")
                .push(context.timeout());
        }
    }

    impl Provider for TimeoutProbeProvider {
        fn descriptor(&self) -> ProviderDescriptor {
            descriptor_for(MODRINTH_PROVIDER_ID, MODRINTH_CAPABILITIES)
        }
    }

    impl ModProvider for TimeoutProbeProvider {
        fn search(
            &self,
            context: &ModProviderContext<'_>,
            _request: &ModSearchRequest,
        ) -> Result<ModSearchResultData, String> {
            self.record(context);
            Err("timeout probe".to_string())
        }

        fn list_versions(
            &self,
            context: &ModProviderContext<'_>,
            _request: &ModListVersionsRequest,
        ) -> Result<Vec<ModVersionData>, String> {
            self.record(context);
            Err("timeout probe".to_string())
        }

        fn version_by_id(
            &self,
            context: &ModProviderContext<'_>,
            _request: &ModVersionRequest,
        ) -> Result<ModVersionData, String> {
            self.record(context);
            Err("timeout probe".to_string())
        }

        fn dependency_version(
            &self,
            context: &ModProviderContext<'_>,
            _request: &ModDependencyVersionRequest,
        ) -> Result<Option<ModVersionData>, String> {
            self.record(context);
            Err("timeout probe".to_string())
        }
    }

    fn probe_registry() -> (ProviderRegistry<dyn ModProvider>, Arc<Mutex<Vec<Duration>>>) {
        let timeouts = Arc::new(Mutex::new(Vec::new()));
        let providers: Vec<Box<dyn ModProvider>> = vec![Box::new(TimeoutProbeProvider {
            timeouts: Arc::clone(&timeouts),
        })];
        let registry =
            ProviderRegistry::new(providers, MOD_PROVIDER_API).expect("timeout probe registry");
        (registry, timeouts)
    }

    #[test]
    fn mod_loader_validation_is_exact_and_rejects_compatibility_aliases() {
        for loader in ["fabric", "forge", "neoforge", "quilt"] {
            assert_eq!(validate_mod_loader(loader), Ok::<&str, String>(loader));
        }
        for loader in [
            "",
            "vanilla",
            "neo_forge",
            "neo-forge",
            "lite_loader",
            "lite-loader",
            "optifine",
            "Fabric",
            " fabric ",
            "arbitrary",
        ] {
            assert_eq!(
                validate_mod_loader(loader).expect_err("invalid loader"),
                format!(
                    "Unsupported mod loader '{loader}'. Supported loaders are fabric, forge, neoforge, and quilt."
                )
            );
        }
    }

    #[test]
    fn invalid_loader_fails_search_before_provider_or_network_dispatch() {
        let (registry, timeouts) = probe_registry();
        let error = search_mods_blocking(
            &registry,
            ModSource::Modrinth,
            "probe",
            "1.20.1",
            "neo_forge",
            0,
            20,
        )
        .expect_err("alias must be rejected");
        assert_eq!(
            error,
            "Unsupported mod loader 'neo_forge'. Supported loaders are fabric, forge, neoforge, and quilt."
        );
        assert!(timeouts.lock().expect("timeout probe lock").is_empty());

        let sink = RecordingModSink {
            updates: Arc::new(Mutex::new(Vec::new())),
        };
        let expected = "Unsupported mod loader 'neo_forge'. Supported loaders are fabric, forge, neoforge, and quilt.";
        let list_error = list_mod_versions_blocking(
            &registry,
            ModSource::Modrinth,
            "project",
            "1.20.1",
            "neo_forge",
        )
        .err()
        .expect("list versions must reject the alias");
        assert_eq!(list_error, expected);
        let resolve_error = resolve_mod_dependencies_blocking(
            &registry,
            ModSource::Modrinth,
            "project",
            "",
            "1.20.1",
            "neo_forge",
        )
        .err()
        .expect("dependency resolution must reject the alias");
        assert_eq!(resolve_error, expected);
        let install_error = install_mod_blocking(
            &registry,
            &sink,
            "game",
            ModSource::Modrinth,
            "project",
            "",
            "1.20.1",
            "neo_forge",
            false,
        )
        .err()
        .expect("mod installation must reject the alias");
        assert_eq!(install_error, expected);
        assert!(timeouts.lock().expect("timeout probe lock").is_empty());
    }

    #[test]
    fn host_operation_timeouts_remain_45_45_60_600_and_enter_provider_context() {
        assert_eq!(SEARCH_MODS_TIMEOUT, Duration::from_secs(45));
        assert_eq!(LIST_MOD_VERSIONS_TIMEOUT, Duration::from_secs(45));
        assert_eq!(RESOLVE_MOD_DEPENDENCIES_TIMEOUT, Duration::from_secs(60));
        assert_eq!(INSTALL_MOD_TIMEOUT, Duration::from_secs(600));

        let (registry, timeouts) = probe_registry();
        assert!(search_mods_blocking(
            &registry,
            ModSource::Modrinth,
            "probe",
            "1.20.1",
            "fabric",
            0,
            20,
        )
        .is_err());
        assert!(list_mod_versions_blocking(
            &registry,
            ModSource::Modrinth,
            "project",
            "1.20.1",
            "fabric",
        )
        .is_err());
        assert!(resolve_mod_dependencies_blocking(
            &registry,
            ModSource::Modrinth,
            "project",
            "",
            "1.20.1",
            "fabric",
        )
        .is_err());
        assert_eq!(
            timeouts.lock().expect("timeout probe lock").clone(),
            vec![
                Duration::from_secs(45),
                Duration::from_secs(45),
                Duration::from_secs(60)
            ]
        );

        let network = NoopNetwork;
        let selected = select_for_source(
            &registry,
            ModSource::Modrinth,
            &network,
            INSTALL_MOD_TIMEOUT,
        )
        .expect("selected install provider");
        assert_eq!(selected.context.timeout(), Duration::from_secs(600));
    }
    struct RecordingModSink {
        updates: std::sync::Arc<std::sync::Mutex<Vec<ModInstallProgress>>>,
    }

    impl ModInstallProgressSink for RecordingModSink {
        fn report(&self, progress: ModInstallProgress) {
            self.updates.lock().expect("mod progress lock").push(progress);
        }
    }

    #[test]
    fn mod_progress_sink_is_object_safe_and_reports_exact_payload() {
        fn assert_send_sync<T: ?Sized + Send + Sync>() {}
        assert_send_sync::<dyn ModInstallProgressSink>();
        let sink = RecordingModSink {
            updates: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
        };
        let erased: &dyn ModInstallProgressSink = &sink;
        emit_mod_progress(
            erased,
            "Example Mod",
            "example.jar",
            "download",
            1,
            2,
            "Processed example.jar",
        );
        let updates = sink.updates.lock().expect("mod progress lock");
        assert_eq!(updates.len(), 1);
        assert_eq!(updates[0].project_name, "Example Mod");
        assert_eq!(updates[0].file_name, "example.jar");
        assert_eq!(updates[0].stage, "download");
        assert_eq!(updates[0].completed, 1);
        assert_eq!(updates[0].total, 2);
        assert_eq!(updates[0].message, "Processed example.jar");
        drop(updates);

        emit_mod_progress(
            erased,
            "Example Mod",
            "example.jar",
            "provider",
            2,
            2,
            "Provider detail\nhttps://example.com/private?token=url-secret\tapi_key=api-secret",
        );
        let updates = sink.updates.lock().expect("mod progress lock");
        assert_eq!(updates.len(), 2);
        assert_eq!(updates[0].stage, "download");
        assert_eq!(updates[1].stage, "provider");
        assert!(updates[1].message.contains("[redacted-url]"));
        for secret in ["url-secret", "api-secret", "https://"] {
            assert!(!updates[1].message.contains(secret));
        }
        assert!(!updates[1].message.chars().any(char::is_control));
        let payload = serde_json::to_value(&updates[1]).expect("mod progress payload");
        assert_eq!(payload["stage"].as_str(), Some("provider"));
        assert!(!payload["message"]
            .as_str()
            .unwrap_or_default()
            .contains("api-secret"));
    }

    #[test]
    fn install_download_uses_the_600s_operation_timeout() {
        let mods_dir = std::env::temp_dir().join(format!(
            "flint-timeout-probe-{}-{}",
            std::process::id(),
            net::nonce()
        ));
        let network = DownloadProbeNetwork {
            timeout: Mutex::new(None),
        };
        let version = ModVersionInfo {
            id: "version".to_string(),
            source: ModSource::Modrinth,
            project_id: "project".to_string(),
            name: "Probe".to_string(),
            version_number: "1.0.0".to_string(),
            file_name: "probe.jar".to_string(),
            download_url: Some("https://cdn.example/probe.jar".to_string()),
            game_versions: vec!["1.20.1".to_string()],
            loaders: vec!["fabric".to_string()],
            release_type: "release".to_string(),
            published_at: None,
            dependencies: Vec::new(),
        };

        assert!(install_single_mod(
            &network,
            &mods_dir,
            &version,
            "fabric",
            INSTALL_MOD_TIMEOUT,
        )
        .is_err());
        assert_eq!(
            *network.timeout.lock().expect("download timeout lock"),
            Some(Duration::from_secs(600))
        );
    }

}
