use crate::{
    models::{
        InstalledModInfo, ModDependencyPlan, ModDependencyRef, ModDependencyType,
        ModInstallProgress, ModInstallResult, ModProject, ModSearchPage, ModSource, ModVersionInfo,
    },
    net,
    paths::resolve_game_dir,
    settings,
};
use regex::Regex;
use reqwest::blocking::{Client, RequestBuilder};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::HashSet,
    fs,
    io::Read,
    path::{Path, PathBuf},
    time::Duration,
};
use tauri::{AppHandle, Emitter};
use zip::ZipArchive;

const MODRINTH_API: &str = "https://api.modrinth.com/v2";
const CURSEFORGE_API: &str = "https://api.curseforge.com/v1";
const MAX_API_BODY: u64 = 16 * 1024 * 1024;
const MAX_MOD_DOWNLOAD: u64 = 512 * 1024 * 1024;
const MAX_MOD_METADATA_ENTRY: u64 = 2 * 1024 * 1024;
const MAX_MOD_ARCHIVE_ENTRIES: usize = 20_000;
const MAX_DEPENDENCY_DEPTH: usize = 8;
const MAX_DEPENDENCY_NODES: usize = 64;
const CURSE_MINECRAFT_GAME_ID: u32 = 432;
const CURSE_MODS_CLASS_ID: u32 = 6;

pub fn search_mods_blocking(
    source: ModSource,
    query: &str,
    game_version: &str,
    loader: &str,
    offset: u32,
    limit: u32,
) -> Result<ModSearchPage, String> {
    validate_search_inputs(query, game_version, offset, limit)?;
    let loader = normalize_loader(loader);
    let client = net::http_client(Duration::from_secs(45))?;
    match source {
        ModSource::Modrinth => modrinth_search(
            &client,
            query.trim(),
            game_version.trim(),
            &loader,
            offset,
            limit,
        ),
        ModSource::Curseforge => {
            let api_key = require_curseforge_key()?;
            curseforge_search(
                &client,
                &api_key,
                query.trim(),
                game_version.trim(),
                &loader,
                offset,
                limit,
            )
        }
    }
}

pub fn list_mod_versions_blocking(
    source: ModSource,
    project_id: &str,
    game_version: &str,
    loader: &str,
) -> Result<Vec<ModVersionInfo>, String> {
    validate_provider_id(project_id)?;
    validate_filter_text(game_version, "game version")?;
    let loader = normalize_loader(loader);
    let client = net::http_client(Duration::from_secs(45))?;
    match source {
        ModSource::Modrinth => {
            modrinth_list_versions(&client, project_id, game_version.trim(), &loader)
        }
        ModSource::Curseforge => {
            let api_key = require_curseforge_key()?;
            curseforge_list_versions(&client, &api_key, project_id, game_version.trim(), &loader)
        }
    }
}

pub fn resolve_mod_dependencies_blocking(
    source: ModSource,
    project_id: &str,
    version_id: &str,
    game_version: &str,
    loader: &str,
) -> Result<ModDependencyPlan, String> {
    validate_provider_id(project_id)?;
    if !version_id.trim().is_empty() {
        validate_provider_id(version_id)?;
    }
    validate_filter_text(game_version, "game version")?;
    let loader = normalize_loader(loader);
    let client = net::http_client(Duration::from_secs(60))?;
    let api_key = if source == ModSource::Curseforge {
        Some(require_curseforge_key()?)
    } else {
        None
    };
    let root = load_root_version(
        &client,
        source,
        api_key.as_deref(),
        project_id,
        version_id,
        game_version.trim(),
        &loader,
    )?;
    build_dependency_plan(
        &client,
        source,
        api_key.as_deref(),
        root,
        game_version.trim(),
        &loader,
    )
}

pub fn install_mod_blocking(
    app: &AppHandle,
    game_dir: &str,
    source: ModSource,
    project_id: &str,
    version_id: &str,
    game_version: &str,
    loader: &str,
    install_dependencies: bool,
) -> Result<ModInstallResult, String> {
    validate_provider_id(project_id)?;
    if !version_id.trim().is_empty() {
        validate_provider_id(version_id)?;
    }
    validate_filter_text(game_version, "game version")?;
    let loader = normalize_loader(loader);
    let game_root = resolve_game_dir(game_dir);
    let mods_dir = game_root.join("mods");
    net::ensure_directory(&mods_dir)?;
    let client = net::http_client(Duration::from_secs(600))?;
    let api_key = if source == ModSource::Curseforge {
        Some(require_curseforge_key()?)
    } else {
        None
    };
    let root_version = load_root_version(
        &client,
        source,
        api_key.as_deref(),
        project_id,
        version_id,
        game_version.trim(),
        &loader,
    )?;
    let mut warnings = Vec::new();
    let mut plan_required = Vec::new();
    if install_dependencies {
        let plan = build_dependency_plan(
            &client,
            source,
            api_key.as_deref(),
            root_version.clone(),
            game_version.trim(),
            &loader,
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
            app,
            &version.name,
            version.file_name.as_str(),
            "download",
            index as u64,
            total,
            &format!("Installing {}", version.file_name),
        );
        let (installed, was_skipped) = install_single_mod(&client, &mods_dir, version, &loader)?;
        if was_skipped {
            skipped.push(version.file_name.clone());
        }
        if index == 0 {
            root_result = Some(installed);
        } else if !was_skipped {
            dependency_results.push(installed);
        }
        emit_mod_progress(
            app,
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
        app,
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

fn normalize_loader(loader: &str) -> String {
    match loader.trim().to_ascii_lowercase().as_str() {
        "neo_forge" | "neo-forge" => "neoforge".to_string(),
        "lite_loader" | "lite-loader" => "liteloader".to_string(),
        "optifine" => "optifine".to_string(),
        other if !other.is_empty() => other.to_string(),
        _ => "vanilla".to_string(),
    }
}

fn validate_provider_id(value: &str) -> Result<(), String> {
    let value = value.trim();
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err("The provider project or version id is invalid.".to_string());
    }
    Ok(())
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

fn modrinth_search(
    client: &Client,
    query: &str,
    game_version: &str,
    loader: &str,
    offset: u32,
    limit: u32,
) -> Result<ModSearchPage, String> {
    let facets = if loader == "vanilla" {
        json!([["project_type:mod"], ["versions:{}", game_version]])
    } else {
        json!([
            ["project_type:mod"],
            ["versions:{}", game_version],
            ["categories:{}", loader]
        ])
    };
    let facets = facets.to_string();
    let request = client
        .get(format!("{MODRINTH_API}/search"))
        .header(reqwest::header::ACCEPT, "application/json")
        .query(&[
            ("query", query),
            ("facets", facets.as_str()),
            ("offset", &offset.to_string()),
            ("limit", &limit.to_string()),
        ]);
    let value = send_json(request, "Modrinth")?;
    let hits = value
        .get("hits")
        .and_then(Value::as_array)
        .ok_or_else(|| "Modrinth search response has no hits array.".to_string())?;
    let projects = hits
        .iter()
        .filter_map(modrinth_project_from_search)
        .collect();
    Ok(ModSearchPage {
        source: ModSource::Modrinth,
        total: u64_field(&value, "total_hits"),
        offset,
        limit,
        hits: projects,
    })
}

fn modrinth_project_from_search(value: &Value) -> Option<ModProject> {
    let id = value_id(value.get("project_id")?)?;
    let slug = value
        .get("slug")
        .and_then(Value::as_str)
        .unwrap_or(&id)
        .to_string();
    let fallback_url = format!("https://modrinth.com/mod/{slug}");
    let project_url = value
        .get("project_url")
        .and_then(Value::as_str)
        .map(|value| value.to_string())
        .unwrap_or_else(|| fallback_url.clone());
    let project_url = optional_https_url(&project_url).unwrap_or(fallback_url);
    Some(ModProject {
        id,
        source: ModSource::Modrinth,
        slug,
        name: value
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or("Untitled mod")
            .to_string(),
        summary: value
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        author: value
            .get("author")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        downloads: u64_field(value, "downloads"),
        icon_url: value
            .get("icon_url")
            .and_then(Value::as_str)
            .and_then(optional_https_url),
        categories: string_array(value.get("categories")),
        project_url,
    })
}

fn modrinth_list_versions(
    client: &Client,
    project_id: &str,
    game_version: &str,
    loader: &str,
) -> Result<Vec<ModVersionInfo>, String> {
    validate_provider_id(project_id)?;
    let game_versions = serde_json::to_string(&[game_version])
        .map_err(|error| format!("Cannot encode game-version filter: {error}"))?;
    let loaders = if loader == "vanilla" {
        None
    } else {
        Some(
            serde_json::to_string(&[loader])
                .map_err(|error| format!("Cannot encode loader filter: {error}"))?,
        )
    };
    let mut request = client
        .get(format!("{MODRINTH_API}/project/{project_id}/version"))
        .header(reqwest::header::ACCEPT, "application/json")
        .query(&[("game_versions", game_versions.as_str())]);
    if let Some(loaders) = loaders.as_deref() {
        request = request.query(&[("loaders", loaders)]);
    }
    let value = send_json(request, "Modrinth")?;
    let versions = value
        .as_array()
        .ok_or_else(|| "Modrinth version response is not an array.".to_string())?;
    let mut result: Vec<ModVersionInfo> = versions
        .iter()
        .filter_map(|value| modrinth_version_from_value(value, project_id))
        .collect();
    result.sort_by(|left, right| {
        right
            .published_at
            .as_deref()
            .unwrap_or_default()
            .cmp(left.published_at.as_deref().unwrap_or_default())
    });
    Ok(result)
}

fn modrinth_version_by_id(client: &Client, version_id: &str) -> Result<ModVersionInfo, String> {
    validate_provider_id(version_id)?;
    let request = client
        .get(format!("{MODRINTH_API}/version/{version_id}"))
        .header(reqwest::header::ACCEPT, "application/json");
    let value = send_json(request, "Modrinth")?;
    modrinth_version_from_value(&value, "")
        .ok_or_else(|| "Modrinth returned an invalid version object.".to_string())
}

fn modrinth_version_from_value(value: &Value, fallback_project_id: &str) -> Option<ModVersionInfo> {
    let id = value_id(value.get("id")?)?;
    let project_id = value
        .get("project_id")
        .and_then(value_id)
        .unwrap_or_else(|| fallback_project_id.to_string());
    let files = value.get("files").and_then(Value::as_array)?;
    let selected_file = files
        .iter()
        .find(|file| {
            file.get("primary")
                .and_then(Value::as_bool)
                .unwrap_or(false)
                && file
                    .get("filename")
                    .and_then(Value::as_str)
                    .map(|name| name.to_ascii_lowercase().ends_with(".jar"))
                    .unwrap_or(false)
        })
        .or_else(|| {
            files.iter().find(|file| {
                file.get("filename")
                    .and_then(Value::as_str)
                    .map(|name| name.to_ascii_lowercase().ends_with(".jar"))
                    .unwrap_or(false)
            })
        })?;
    let file_name = selected_file
        .get("filename")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let download_url = selected_file
        .get("url")
        .and_then(Value::as_str)
        .and_then(optional_https_url);
    let dependencies = value
        .get("dependencies")
        .and_then(Value::as_array)
        .map(|dependencies| {
            dependencies
                .iter()
                .filter_map(modrinth_dependency_from_value)
                .collect()
        })
        .unwrap_or_default();
    Some(ModVersionInfo {
        id,
        source: ModSource::Modrinth,
        project_id,
        name: value
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("Unnamed mod version")
            .to_string(),
        version_number: value
            .get("version_number")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        file_name,
        download_url,
        game_versions: string_array(value.get("game_versions")),
        loaders: string_array(value.get("loaders")),
        release_type: value
            .get("version_type")
            .and_then(Value::as_str)
            .unwrap_or("release")
            .to_string(),
        published_at: value
            .get("date_published")
            .and_then(Value::as_str)
            .map(|value| value.to_string()),
        dependencies,
    })
}

fn modrinth_dependency_from_value(value: &Value) -> Option<ModDependencyRef> {
    let dependency_type = match value
        .get("dependency_type")
        .and_then(Value::as_str)
        .unwrap_or("required")
    {
        "required" => ModDependencyType::Required,
        "optional" => ModDependencyType::Optional,
        "embedded" => ModDependencyType::Embedded,
        "incompatible" => ModDependencyType::Incompatible,
        _ => return None,
    };
    let project_id = value
        .get("project_id")
        .and_then(value_id)
        .unwrap_or_default();
    let version_id = value.get("version_id").and_then(value_id);
    let name = value
        .get("file_name")
        .and_then(Value::as_str)
        .map(|value| value.to_string())
        .or_else(|| {
            if project_id.is_empty() {
                None
            } else {
                Some(format!("Modrinth project {project_id}"))
            }
        })
        .unwrap_or_else(|| "Unknown Modrinth dependency".to_string());
    Some(ModDependencyRef {
        source: ModSource::Modrinth,
        project_id,
        version_id,
        name,
        dependency_type,
    })
}
fn curseforge_search(
    client: &Client,
    api_key: &str,
    query: &str,
    game_version: &str,
    loader: &str,
    offset: u32,
    limit: u32,
) -> Result<ModSearchPage, String> {
    let mut request = client
        .get(format!("{CURSEFORGE_API}/mods/search"))
        .header("x-api-key", api_key)
        .header(reqwest::header::ACCEPT, "application/json")
        .query(&[
            ("gameId", CURSE_MINECRAFT_GAME_ID.to_string()),
            ("classId", CURSE_MODS_CLASS_ID.to_string()),
            ("index", offset.to_string()),
            ("pageSize", limit.to_string()),
            ("sortField", "2".to_string()),
            ("sortOrder", "desc".to_string()),
            ("gameVersion", game_version.to_string()),
        ]);
    if !query.is_empty() {
        request = request.query(&[("searchFilter", query.to_string())]);
    }
    if let Some(loader_type) = curseforge_loader_type(loader) {
        request = request.query(&[("modLoaderType", loader_type.to_string())]);
    }
    let value = send_json(request, "CurseForge")?;
    let data = value
        .get("data")
        .and_then(Value::as_array)
        .ok_or_else(|| "CurseForge search response has no data array.".to_string())?;
    let hits = data
        .iter()
        .filter_map(curseforge_project_from_search)
        .collect();
    let pagination = value.get("pagination").cloned().unwrap_or(Value::Null);
    Ok(ModSearchPage {
        source: ModSource::Curseforge,
        total: u64_field(&pagination, "totalCount"),
        offset,
        limit,
        hits,
    })
}

fn curseforge_project_from_search(value: &Value) -> Option<ModProject> {
    let id = value_id(value.get("id")?)?;
    let slug = value
        .get("slug")
        .and_then(Value::as_str)
        .unwrap_or(&id)
        .to_string();
    let fallback_url = format!("https://www.curseforge.com/minecraft/mc-mods/{slug}");
    let project_url = value
        .get("links")
        .and_then(|links| links.get("websiteUrl"))
        .and_then(Value::as_str)
        .map(|value| value.to_string())
        .unwrap_or_else(|| fallback_url.clone());
    let project_url = optional_https_url(&project_url).unwrap_or(fallback_url);
    let authors = value
        .get("authors")
        .and_then(Value::as_array)
        .map(|authors| {
            authors
                .iter()
                .filter_map(|author| author.get("name").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default();
    let categories = value
        .get("categories")
        .and_then(Value::as_array)
        .map(|categories| {
            categories
                .iter()
                .filter_map(|category| {
                    category
                        .get("name")
                        .and_then(Value::as_str)
                        .or_else(|| category.as_str())
                        .map(|value| value.to_string())
                })
                .collect()
        })
        .unwrap_or_default();
    Some(ModProject {
        id,
        source: ModSource::Curseforge,
        slug,
        name: value
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("Untitled mod")
            .to_string(),
        summary: value
            .get("summary")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        author: authors,
        downloads: u64_field(value, "downloadCount"),
        icon_url: value
            .get("logo")
            .and_then(|logo| logo.get("thumbnailUrl").or_else(|| logo.get("url")))
            .and_then(Value::as_str)
            .and_then(optional_https_url),
        categories,
        project_url,
    })
}

fn curseforge_list_versions(
    client: &Client,
    api_key: &str,
    project_id: &str,
    game_version: &str,
    loader: &str,
) -> Result<Vec<ModVersionInfo>, String> {
    validate_provider_id(project_id)?;
    let mut result = Vec::new();
    let mut seen = HashSet::new();
    for page in 0..10u32 {
        let mut request = client
            .get(format!("{CURSEFORGE_API}/mods/{project_id}/files"))
            .header("x-api-key", api_key)
            .header(reqwest::header::ACCEPT, "application/json")
            .query(&[
                ("gameVersion", game_version.to_string()),
                ("index", (page * 50).to_string()),
                ("pageSize", "50".to_string()),
            ]);
        if let Some(loader_type) = curseforge_loader_type(loader) {
            request = request.query(&[("modLoaderType", loader_type.to_string())]);
        }
        let value = send_json(request, "CurseForge")?;
        let data = value
            .get("data")
            .and_then(Value::as_array)
            .ok_or_else(|| "CurseForge file response has no data array.".to_string())?;
        for version in data
            .iter()
            .filter_map(|value| curseforge_version_from_value(value, project_id, loader))
        {
            if seen.insert(version.id.clone()) {
                result.push(version);
            }
        }
        if data.len() < 50 {
            break;
        }
    }
    result.sort_by(|left, right| {
        right
            .published_at
            .as_deref()
            .unwrap_or_default()
            .cmp(left.published_at.as_deref().unwrap_or_default())
    });
    Ok(result)
}

fn curseforge_version_from_value(
    value: &Value,
    fallback_project_id: &str,
    requested_loader: &str,
) -> Option<ModVersionInfo> {
    let id = value_id(value.get("id")?)?;
    let project_id = value
        .get("modId")
        .and_then(value_id)
        .unwrap_or_else(|| fallback_project_id.to_string());
    let file_name = value
        .get("fileName")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    if file_name.is_empty() {
        return None;
    }
    let game_versions = string_array(value.get("gameVersions"));
    let mut loaders: Vec<String> = game_versions
        .iter()
        .filter_map(|value| curseforge_loader_label(value))
        .collect();
    loaders.sort();
    loaders.dedup();
    if loaders.is_empty() && requested_loader != "vanilla" {
        loaders.push(requested_loader.to_string());
    }
    let dependencies = value
        .get("dependencies")
        .and_then(Value::as_array)
        .map(|dependencies| {
            dependencies
                .iter()
                .filter_map(curseforge_dependency_from_value)
                .collect()
        })
        .unwrap_or_default();
    Some(ModVersionInfo {
        id,
        source: ModSource::Curseforge,
        project_id,
        name: value
            .get("displayName")
            .and_then(Value::as_str)
            .unwrap_or(&file_name)
            .to_string(),
        version_number: value
            .get("displayName")
            .and_then(Value::as_str)
            .unwrap_or(&file_name)
            .to_string(),
        file_name,
        download_url: value
            .get("downloadUrl")
            .and_then(Value::as_str)
            .and_then(optional_https_url),
        game_versions,
        loaders,
        release_type: curseforge_release_type(
            value
                .get("releaseType")
                .and_then(Value::as_u64)
                .unwrap_or(1),
        ),
        published_at: value
            .get("fileDate")
            .and_then(Value::as_str)
            .map(|value| value.to_string()),
        dependencies,
    })
}

fn curseforge_dependency_from_value(value: &Value) -> Option<ModDependencyRef> {
    let project_id = value_id(value.get("modId")?)?;
    let relation_type = value
        .get("relationType")
        .and_then(Value::as_u64)
        .unwrap_or(3);
    let dependency_type = match relation_type {
        3 => ModDependencyType::Required,
        1 => ModDependencyType::Embedded,
        5 => ModDependencyType::Incompatible,
        2 | 4 | 6 => ModDependencyType::Optional,
        _ => ModDependencyType::Optional,
    };
    Some(ModDependencyRef {
        source: ModSource::Curseforge,
        project_id: project_id.clone(),
        version_id: None,
        name: format!("CurseForge project {project_id}"),
        dependency_type,
    })
}

fn curseforge_loader_type(loader: &str) -> Option<u32> {
    match loader {
        "forge" => Some(1),
        "liteloader" => Some(3),
        "fabric" => Some(4),
        "quilt" => Some(5),
        "neoforge" => Some(6),
        _ => None,
    }
}

fn curseforge_loader_label(value: &str) -> Option<String> {
    let lower = value.to_ascii_lowercase();
    for loader in ["neoforge", "forge", "fabric", "quilt", "liteloader"] {
        if lower.contains(loader) {
            return Some(loader.to_string());
        }
    }
    None
}

fn curseforge_release_type(value: u64) -> String {
    match value {
        2 => "beta",
        3 => "alpha",
        _ => "release",
    }
    .to_string()
}

fn load_root_version(
    client: &Client,
    source: ModSource,
    api_key: Option<&str>,
    project_id: &str,
    version_id: &str,
    game_version: &str,
    loader: &str,
) -> Result<ModVersionInfo, String> {
    let version_id = version_id.trim();
    let versions = match source {
        ModSource::Modrinth => {
            if version_id.is_empty() {
                modrinth_list_versions(client, project_id, game_version, loader)?
            } else {
                vec![modrinth_version_by_id(client, version_id)?]
            }
        }
        ModSource::Curseforge => {
            let api_key = api_key
                .ok_or_else(|| "CurseForge API key is required for this operation.".to_string())?;
            let versions =
                curseforge_list_versions(client, api_key, project_id, game_version, loader)?;
            if version_id.is_empty() {
                versions
            } else {
                versions
                    .into_iter()
                    .filter(|version| version.id.eq_ignore_ascii_case(version_id))
                    .collect()
            }
        }
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
                && (loader == "vanilla"
                    || version.loaders.is_empty()
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
    client: &Client,
    source: ModSource,
    api_key: Option<&str>,
    root: ModVersionInfo,
    game_version: &str,
    loader: &str,
) -> Result<ModDependencyPlan, String> {
    if root.download_url.is_none() {
        return Err("The selected mod version has no safe HTTPS download URL.".to_string());
    }
    match source {
        ModSource::Modrinth => {
            let fetch = |dependency: &ModDependencyRef| -> Result<Option<ModVersionInfo>, String> {
                if let Some(version_id) = dependency.version_id.as_deref() {
                    return modrinth_version_by_id(client, version_id).map(Some);
                }
                if dependency.project_id.is_empty() {
                    return Ok(None);
                }
                let versions =
                    modrinth_list_versions(client, &dependency.project_id, game_version, loader)?;
                Ok(versions.into_iter().next())
            };
            resolve_dependency_plan_with(root, |dependency| fetch(dependency))
        }
        ModSource::Curseforge => {
            let api_key = api_key.ok_or_else(|| {
                "CurseForge API key is required for dependency resolution.".to_string()
            })?;
            let fetch = |dependency: &ModDependencyRef| -> Result<Option<ModVersionInfo>, String> {
                if dependency.project_id.is_empty() {
                    return Ok(None);
                }
                let versions = curseforge_list_versions(
                    client,
                    api_key,
                    &dependency.project_id,
                    game_version,
                    loader,
                )?;
                Ok(versions.into_iter().next())
            };
            resolve_dependency_plan_with(root, |dependency| fetch(dependency))
        }
    }
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
fn install_single_mod(
    client: &Client,
    mods_dir: &Path,
    version: &ModVersionInfo,
    requested_loader: &str,
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
    net::download_atomic(
        client,
        download_url,
        &destination,
        None,
        MAX_MOD_DOWNLOAD,
        &version.file_name,
    )?;
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
    if installed.loader.is_none() && requested_loader != "vanilla" {
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
    let file = fs::File::open(path)
        .map_err(|error| format!("Cannot open downloaded mod archive: {error}"))?;
    let archive = ZipArchive::new(file)
        .map_err(|_| "The provider download is not a valid JAR/ZIP archive.".to_string())?;
    if archive.len() > MAX_MOD_ARCHIVE_ENTRIES {
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

fn send_json(request: RequestBuilder, service: &str) -> Result<Value, String> {
    let response = request
        .send()
        .map_err(|error| format!("Cannot contact {service}: {error}"))?;
    let status = response.status();
    let bytes = net::read_body_limited(response, MAX_API_BODY)?;
    if !status.is_success() {
        if service == "CurseForge" && matches!(status.as_u16(), 401 | 403) {
            return Err(
                "CurseForge rejected the configured API key. Check the key in Settings."
                    .to_string(),
            );
        }
        if status.as_u16() == 404 {
            return Err(format!(
                "{service} did not find the requested project or version."
            ));
        }
        return Err(format!(
            "{service} request failed with HTTP {}.",
            status.as_u16()
        ));
    }
    serde_json::from_slice(&bytes)
        .map_err(|error| format!("{service} returned invalid JSON: {error}"))
}

fn value_id(value: &Value) -> Option<String> {
    value
        .as_str()
        .map(|value| value.to_string())
        .or_else(|| value.as_u64().map(|value| value.to_string()))
        .or_else(|| value.as_i64().map(|value| value.to_string()))
}

fn u64_field(value: &Value, field: &str) -> u64 {
    value.get(field).and_then(Value::as_u64).unwrap_or(0)
}

fn string_array(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .map(|value| value.to_string())
                .collect()
        })
        .unwrap_or_default()
}

fn optional_https_url(value: &str) -> Option<String> {
    if value.starts_with("https://") {
        Some(value.to_string())
    } else {
        None
    }
}

fn emit_mod_progress(
    app: &AppHandle,
    project_name: &str,
    file_name: &str,
    stage: &str,
    completed: u64,
    total: u64,
    message: &str,
) {
    let _ = app.emit(
        "flint://mod-install-progress",
        ModInstallProgress {
            project_name: project_name.to_string(),
            file_name: file_name.to_string(),
            stage: stage.to_string(),
            completed,
            total,
            message: message.to_string(),
        },
    );
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

    #[test]
    fn provider_urls_must_be_https() {
        assert_eq!(
            optional_https_url("https://cdn.example.com/file.jar").as_deref(),
            Some("https://cdn.example.com/file.jar")
        );
        assert!(optional_https_url("http://cdn.example.com/file.jar").is_none());
    }

    #[test]
    fn parses_modrinth_version_shape() {
        let value = serde_json::json!({
            "id": "version-id",
            "project_id": "project-id",
            "name": "Example Version",
            "version_number": "1.0.0",
            "version_type": "release",
            "date_published": "2025-01-01T00:00:00Z",
            "game_versions": ["1.20.1"],
            "loaders": ["fabric"],
            "files": [{
                "filename": "example.jar",
                "url": "https://cdn.modrinth.com/example.jar",
                "primary": true
            }],
            "dependencies": [{
                "project_id": "dependency",
                "version_id": null,
                "file_name": "dependency.jar",
                "dependency_type": "required"
            }]
        });
        let version = modrinth_version_from_value(&value, "").expect("modrinth version");
        assert_eq!(version.id, "version-id");
        assert_eq!(version.project_id, "project-id");
        assert_eq!(version.file_name, "example.jar");
        assert_eq!(version.dependencies.len(), 1);
        assert_eq!(
            version.dependencies[0].dependency_type,
            ModDependencyType::Required
        );
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
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn maps_curseforge_dependency_relation_types() {
        let required = curseforge_dependency_from_value(&serde_json::json!({
            "modId": 123,
            "relationType": 3
        }))
        .expect("required dependency");
        assert_eq!(required.dependency_type, ModDependencyType::Required);

        let incompatible = curseforge_dependency_from_value(&serde_json::json!({
            "modId": 124,
            "relationType": 5
        }))
        .expect("incompatible dependency");
        assert_eq!(
            incompatible.dependency_type,
            ModDependencyType::Incompatible
        );

        let embedded = curseforge_dependency_from_value(&serde_json::json!({
            "modId": 125,
            "relationType": 1
        }))
        .expect("embedded dependency");
        assert_eq!(embedded.dependency_type, ModDependencyType::Embedded);
        assert_eq!(curseforge_release_type(3), "alpha");
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
