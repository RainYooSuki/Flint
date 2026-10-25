//! CurseForge implementation of the source-neutral mod provider contract.

use flint_core::ports::network::sanitize_network_error;
use flint_core::ports::{
    NetworkPort, NetworkRequest, ServiceLabel,
};
use flint_provider_api::{
    descriptor_for, validate_mod_provider_id, ModDependencyData, ModDependencyKind,
    ModDependencyVersionRequest, ModListVersionsRequest, ModProjectData, ModProvider,
    ModProviderContext, ModSearchRequest, ModSearchResultData, ModVersionData, ModVersionRequest,
    Provider, ProviderDescriptor, CURSEFORGE_CAPABILITIES, CURSEFORGE_PROVIDER_ID,
};
use serde_json::Value;
use std::collections::HashSet;

const API_BASE: &str = "https://api.curseforge.com/v1";
const SERVICE: &str = "CurseForge";
const MAX_API_BODY: u64 = 16 * 1024 * 1024;
const CURSE_MINECRAFT_GAME_ID: u32 = 432;
const CURSE_MODS_CLASS_ID: u32 = 6;

pub const DESCRIPTOR: ProviderDescriptor =
    descriptor_for(CURSEFORGE_PROVIDER_ID, CURSEFORGE_CAPABILITIES);

#[derive(Debug, Clone, Copy, Default)]
pub struct CurseForgeModProvider;

impl CurseForgeModProvider {
    pub const fn new() -> Self {
        Self
    }
}

impl Provider for CurseForgeModProvider {
    fn descriptor(&self) -> ProviderDescriptor {
        DESCRIPTOR
    }
}

impl ModProvider for CurseForgeModProvider {
    fn search(
        &self,
        context: &ModProviderContext<'_>,
        request: &ModSearchRequest,
    ) -> Result<ModSearchResultData, String> {
        let api_key = require_api_key(context, "this operation")?;
        let mut params = vec![
            ("gameId", CURSE_MINECRAFT_GAME_ID.to_string()),
            ("classId", CURSE_MODS_CLASS_ID.to_string()),
            ("index", request.offset.to_string()),
            ("pageSize", request.limit.to_string()),
            ("sortField", "2".to_string()),
            ("sortOrder", "desc".to_string()),
            ("gameVersion", request.game_version.clone()),
        ];
        if !request.query.is_empty() {
            params.push(("searchFilter", request.query.clone()));
        }
        if let Some(loader_type) = curseforge_loader_type(&request.loader) {
            params.push(("modLoaderType", loader_type.to_string()));
        }
        let value = fetch_json(
            context,
            api_key,
            query_url(&format!("{API_BASE}/mods/search"), &params),
        )?;
        let data = value
            .get("data")
            .and_then(Value::as_array)
            .ok_or_else(|| "CurseForge search response has no data array.".to_string())?;
        let hits = data
            .iter()
            .filter_map(curseforge_project_from_search)
            .collect();
        let total = value
            .get("pagination")
            .map(|pagination| u64_field(pagination, "totalCount"))
            .unwrap_or(0);
        Ok(ModSearchResultData {
            total,
            offset: request.offset,
            limit: request.limit,
            hits,
        })
    }

    fn list_versions(
        &self,
        context: &ModProviderContext<'_>,
        request: &ModListVersionsRequest,
    ) -> Result<Vec<ModVersionData>, String> {
        let api_key = require_api_key(context, "this operation")?;
        curseforge_list_versions(
            context,
            api_key,
            &request.project_id,
            &request.game_version,
            &request.loader,
        )
    }

    fn version_by_id(
        &self,
        context: &ModProviderContext<'_>,
        request: &ModVersionRequest,
    ) -> Result<ModVersionData, String> {
        validate_mod_provider_id(&request.version_id)?;
        let api_key = require_api_key(context, "this operation")?;
        curseforge_list_versions(
            context,
            api_key,
            &request.project_id,
            &request.game_version,
            &request.loader,
        )?
        .into_iter()
        .find(|version| {
            version.id.eq_ignore_ascii_case(&request.version_id)
                && version.download_url.is_some()
                && (request.game_version.is_empty()
                    || version.game_versions.is_empty()
                    || version
                        .game_versions
                        .iter()
                        .any(|candidate| candidate.eq_ignore_ascii_case(&request.game_version)))
                && (request.loader == "vanilla"
                    || version.loaders.is_empty()
                    || version
                        .loaders
                        .iter()
                        .any(|candidate| candidate.eq_ignore_ascii_case(&request.loader)))
        })
        .ok_or_else(|| {
            "No compatible downloadable file was found for the selected mod version.".to_string()
        })
    }

    fn dependency_version(
        &self,
        context: &ModProviderContext<'_>,
        request: &ModDependencyVersionRequest,
    ) -> Result<Option<ModVersionData>, String> {
        if request.dependency.project_id.is_empty() {
            return Ok(None);
        }
        let api_key = require_api_key(context, "dependency resolution")?;
        let versions = curseforge_list_versions(
            context,
            api_key,
            &request.dependency.project_id,
            &request.game_version,
            &request.loader,
        )?;
        Ok(versions.into_iter().next())
    }
}

fn require_api_key<'a>(
    context: &'a ModProviderContext<'_>,
    operation: &str,
) -> Result<&'a str, String> {
    context
        .credential()
        .ok_or_else(|| format!("CurseForge API key is required for {operation}."))
}

fn fetch_json(
    context: &ModProviderContext<'_>,
    api_key: &str,
    url: String,
) -> Result<Value, String> {
    context.check_control()?;
    let service = ServiceLabel::new(SERVICE)
        .map_err(|error| format!("Invalid CurseForge service label: {error}"))?;
    let request = NetworkRequest::new(url, service, MAX_API_BODY, context.timeout())
        .with_header("Accept", "application/json")
        .with_header("x-api-key", api_key);
    let network: &dyn NetworkPort = context.network();
    let bytes = network
        .read_bytes(&request)
        .map_err(map_transport_error)?;
    serde_json::from_slice(&bytes)
        .map_err(|error| format!("{SERVICE} returned invalid JSON: {error}"))
}

fn map_transport_error(error: String) -> String {
    let sanitized = sanitize_network_error(&error);
    let failed_prefix = format!("{SERVICE} request failed: ");
    let rejected_prefix = format!("{SERVICE} request rejected: ");
    let detail = sanitized
        .strip_prefix(failed_prefix.as_str())
        .or_else(|| sanitized.strip_prefix(rejected_prefix.as_str()))
        .unwrap_or(&sanitized);
    if is_body_error(detail) {
        return detail.to_string();
    }
    let Some(status) = http_status(detail) else {
        return format!("Cannot contact {SERVICE}: {detail}");
    };
    if matches!(status, 401 | 403) {
        return "CurseForge rejected the configured API key. Check the key in Settings."
            .to_string();
    }
    if status == 404 {
        return format!("{SERVICE} did not find the requested project or version.");
    }
    format!("{SERVICE} request failed with HTTP {status}.")
}

fn is_body_error(error: &str) -> bool {
    error.starts_with("HTTP response is too large (")
        || error.starts_with("HTTP response exceeds the ")
        || error.starts_with("Cannot read HTTP response:")
}

fn http_status(error: &str) -> Option<u16> {
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

fn query_url(base: &str, params: &[(&str, String)]) -> String {
    let query = params
        .iter()
        .map(|(key, value)| format!("{}={}", encode_query(key), encode_query(value)))
        .collect::<Vec<_>>()
        .join("&");
    if query.is_empty() {
        base.to_string()
    } else {
        format!("{base}?{query}")
    }
}

fn encode_query(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.bytes() {
        match byte {
            b' ' => encoded.push('+'),
            b'*' | b'-' | b'.' | b'_' => encoded.push(byte as char),
            byte if byte.is_ascii_alphanumeric() => encoded.push(byte as char),
            byte => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
}

fn curseforge_list_versions(
    context: &ModProviderContext<'_>,
    api_key: &str,
    project_id: &str,
    game_version: &str,
    loader: &str,
) -> Result<Vec<ModVersionData>, String> {
    validate_mod_provider_id(project_id)?;
    let mut result = Vec::new();
    let mut seen = HashSet::new();
    for page in 0..10u32 {
        let mut params = vec![
            ("gameVersion", game_version.to_string()),
            ("index", (page * 50).to_string()),
            ("pageSize", "50".to_string()),
        ];
        if let Some(loader_type) = curseforge_loader_type(loader) {
            params.push(("modLoaderType", loader_type.to_string()));
        }
        let value = fetch_json(
            context,
            api_key,
            query_url(&format!("{API_BASE}/mods/{project_id}/files"), &params),
        )?;
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

fn curseforge_project_from_search(value: &Value) -> Option<ModProjectData> {
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
        .map(str::to_string)
        .unwrap_or_else(|| fallback_url.clone());
    let project_url = optional_https_url(&project_url).unwrap_or(fallback_url);
    let author = value
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
                        .map(str::to_string)
                })
                .collect()
        })
        .unwrap_or_default();
    Some(ModProjectData {
        id,
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
        author,
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

fn curseforge_version_from_value(
    value: &Value,
    fallback_project_id: &str,
    requested_loader: &str,
) -> Option<ModVersionData> {
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
    let mut loaders = game_versions
        .iter()
        .filter_map(|value| curseforge_loader_label(value))
        .collect::<Vec<_>>();
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
    Some(ModVersionData {
        id,
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
            .map(str::to_string),
        dependencies,
    })
}

fn curseforge_dependency_from_value(value: &Value) -> Option<ModDependencyData> {
    let project_id = value_id(value.get("modId")?)?;
    let relation_type = value
        .get("relationType")
        .and_then(Value::as_u64)
        .unwrap_or(3);
    let dependency_type = match relation_type {
        3 => ModDependencyKind::Required,
        1 => ModDependencyKind::Embedded,
        5 => ModDependencyKind::Incompatible,
        _ => ModDependencyKind::Optional,
    };
    Some(ModDependencyData {
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

fn value_id(value: &Value) -> Option<String> {
    value
        .as_str()
        .map(str::to_string)
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
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn optional_https_url(value: &str) -> Option<String> {
    value.starts_with("https://").then(|| value.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use flint_core::ports::{Cancellation, Deadline, DownloadRequest, NoopLogger, NoopProgress};
    use flint_provider_api::ScopedCredential;
    use serde_json::json;
    use std::{sync::Mutex, time::Duration};

    struct FakeNetwork {
        response: Vec<u8>,
        error: Option<String>,
        requests: Mutex<Vec<NetworkRequest>>,
    }

    impl FakeNetwork {
        fn new(response: Value) -> Self {
            Self {
                response: serde_json::to_vec(&response).unwrap(),
                error: None,
                requests: Mutex::new(Vec::new()),
            }
        }

        fn failing(error: impl Into<String>) -> Self {
            Self {
                response: Vec::new(),
                error: Some(error.into()),
                requests: Mutex::new(Vec::new()),
            }
        }

    }

    impl NetworkPort for FakeNetwork {
        fn read_json(&self, _request: &NetworkRequest) -> Result<Value, String> {
            serde_json::from_slice(&self.response)
                .map_err(|error| error.to_string())
        }

        fn read_text(&self, _request: &NetworkRequest) -> Result<String, String> {
            String::from_utf8(self.response.clone()).map_err(|error| error.to_string())
        }

        fn read_bytes(&self, request: &NetworkRequest) -> Result<Vec<u8>, String> {
            self.requests.lock().unwrap().push(request.clone());
            match &self.error {
                Some(error) => Err(error.clone()),
                None => Ok(self.response.clone()),
            }
        }

        fn download_atomic(&self, _request: &DownloadRequest) -> Result<u64, String> {
            Err("download not used in fake network".to_string())
        }
    }


    #[test]
    fn propagates_each_host_operation_timeout_to_requests() {
        let network = FakeNetwork::new(json!({"data": [], "pagination": {"totalCount": 0}}));
        let progress = NoopProgress;
        let logger = NoopLogger;
        let search_request = || ModSearchRequest {
            query: "jei".to_string(),
            game_version: "1.20.1".to_string(),
            loader: "fabric".to_string(),
            offset: 0,
            limit: 20,
        };
        for seconds in [45, 45, 60, 600] {
            let context = ModProviderContext::new(
                &network,
                Some(ScopedCredential::new("test-key")),
                &progress,
                &logger,
                Duration::from_secs(seconds),
                Deadline::after(Duration::from_secs(30)),
                Cancellation::new(),
            );
            CurseForgeModProvider::new()
                .search(&context, &search_request())
                .expect("search result");
        }
        let requests = network.requests.lock().unwrap();
        assert_eq!(requests.len(), 4);
        assert_eq!(
            requests.iter().map(|request| request.timeout).collect::<Vec<_>>(),
            vec![
                Duration::from_secs(45),
                Duration::from_secs(45),
                Duration::from_secs(60),
                Duration::from_secs(600)
            ]
        );
    }

    #[test]
    fn deadline_and_pre_cancelled_cancellation_fail_before_network_io() {
        let request = || ModSearchRequest {
            query: "jei".to_string(),
            game_version: "1.20.1".to_string(),
            loader: "fabric".to_string(),
            offset: 0,
            limit: 20,
        };
        let progress = NoopProgress;
        let logger = NoopLogger;

        let deadline_network = FakeNetwork::new(json!({"data": [], "pagination": {"totalCount": 0}}));
        let deadline_context = ModProviderContext::new(
            &deadline_network,
            Some(ScopedCredential::new("test-key")),
            &progress,
            &logger,
            Duration::from_secs(45),
            Deadline::after(Duration::ZERO),
            Cancellation::new(),
        );
        let error = CurseForgeModProvider::new()
            .search(&deadline_context, &request())
            .expect_err("deadline must fail");
        assert_eq!(error, "Operation deadline exceeded.");
        assert!(deadline_network.requests.lock().unwrap().is_empty());

        let cancelled_network = FakeNetwork::new(json!({"data": [], "pagination": {"totalCount": 0}}));
        let cancellation = Cancellation::new();
        cancellation.cancel();
        let cancelled_context = ModProviderContext::new(
            &cancelled_network,
            Some(ScopedCredential::new("test-key")),
            &progress,
            &logger,
            Duration::from_secs(45),
            Deadline::after(Duration::from_secs(30)),
            cancellation,
        );
        let error = CurseForgeModProvider::new()
            .search(&cancelled_context, &request())
            .expect_err("cancellation must fail");
        assert_eq!(error, "Operation was cancelled.");
        assert!(cancelled_network.requests.lock().unwrap().is_empty());
    }

    #[test]
    fn preserves_transport_and_body_limit_error_wording() {
        let progress = NoopProgress;
        let logger = NoopLogger;
        let request = || ModSearchRequest {
            query: "jei".to_string(),
            game_version: "1.20.1".to_string(),
            loader: "fabric".to_string(),
            offset: 0,
            limit: 20,
        };
        let cases = [
            (
                "CurseForge request failed: connection reset while opening https://api.example.com/path?token=super-secret".to_string(),
                "Cannot contact CurseForge: connection reset while opening [redacted-url]".to_string(),
            ),
            (
                "CurseForge request failed: HTTP response is too large (999 bytes, limit 16777216).".to_string(),
                "HTTP response is too large (999 bytes, limit 16777216).".to_string(),
            ),
            (
                "CurseForge request failed: HTTP response exceeds the 16777216 byte limit.".to_string(),
                "HTTP response exceeds the 16777216 byte limit.".to_string(),
            ),
            (
                "CurseForge request failed: Cannot read HTTP response: connection reset".to_string(),
                "Cannot read HTTP response: connection reset".to_string(),
            ),
        ];
        for (raw_error, expected) in cases {
            let network = FakeNetwork::failing(raw_error);
            let context = ModProviderContext::new(
                &network,
                Some(ScopedCredential::new("test-key")),
                &progress,
                &logger,
                Duration::from_secs(45),
                Deadline::after(Duration::from_secs(30)),
                Cancellation::new(),
            );
            let error = CurseForgeModProvider::new()
                .search(&context, &request())
                .expect_err("transport/body error");
            assert_eq!(error, expected);
            assert!(!error.contains("super-secret"));
            assert!(!error.contains("api.example.com"));
        }
    }

    #[test]
    fn maps_http_status_to_existing_error_semantics() {
        assert_eq!(
            map_transport_error(
                "CurseForge request failed: HTTP status client error (404 Not Found)".to_string()
            ),
            "CurseForge did not find the requested project or version."
        );
        assert_eq!(
            map_transport_error(
                "CurseForge request failed: HTTP status server error (500 Internal Server Error)"
                    .to_string()
            ),
            "CurseForge request failed with HTTP 500."
        );
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
    fn descriptor_declares_the_versioned_mod_catalog_contract() {
        let descriptor = CurseForgeModProvider::new().descriptor();
        assert_eq!(descriptor.id, CURSEFORGE_PROVIDER_ID);
        assert_eq!(descriptor.provider_version, "1.0.0");
        assert_eq!(descriptor.provider_api, flint_provider_api::MOD_PROVIDER_API);
        assert_eq!(descriptor.capabilities.len(), 1);
        assert_eq!(
            descriptor.capabilities[0].id,
            flint_provider_api::MOD_CATALOG_CAPABILITY_ID
        );
        assert_eq!(
            descriptor.capabilities[0].api_version,
            flint_provider_api::MOD_CATALOG_API
        );
        assert_eq!(descriptor.capabilities[0].selection_keys.len(), 1);
        assert_eq!(
            descriptor.capabilities[0].selection_keys[0],
            flint_provider_api::CURSEFORGE_SELECTION_KEY
        );
    }

    #[test]
    fn maps_curseforge_search_to_neutral_data_and_sends_credential_header() {
        let network = FakeNetwork::new(json!({
            "data": [{
                "id": 238222,
                "slug": "jei",
                "name": "Just Enough Items",
                "summary": "Recipe viewer",
                "downloadCount": 42,
                "authors": [{"name": "Author"}],
                "categories": [{"name": "Utility"}],
                "links": {"websiteUrl": "https://www.curseforge.com/minecraft/mc-mods/jei"},
                "logo": {"thumbnailUrl": "https://media.example/icon.png"}
            }],
            "pagination": {"totalCount": 1}
        }));
        let progress = NoopProgress;
        let logger = NoopLogger;
        let context = ModProviderContext::new(
            &network,
            Some(ScopedCredential::new("test-key")),
            &progress,
            &logger,
            Duration::from_secs(45),
            Deadline::after(Duration::from_secs(30)),
            Cancellation::new(),
        );
        let result = CurseForgeModProvider::new()
            .search(
                &context,
                &ModSearchRequest {
                    query: "jei".to_string(),
                    game_version: "1.20.1".to_string(),
                    loader: "fabric".to_string(),
                    offset: 0,
                    limit: 20,
                },
            )
            .expect("search result");
        assert_eq!(result.total, 1);
        assert_eq!(result.hits[0].slug, "jei");
        assert_eq!(result.hits[0].icon_url.as_deref(), Some("https://media.example/icon.png"));
        let requests = network.requests.lock().unwrap();
        assert_eq!(requests.len(), 1);
        assert!(requests[0].url.contains("searchFilter=jei"));
        assert!(requests[0].headers.contains(&(
            "x-api-key".to_string(),
            "test-key".to_string()
        )));

    }

    #[test]
    fn maps_curseforge_dependency_relation_types() {
        let required = curseforge_dependency_from_value(&json!({
            "modId": 1,
            "relationType": 3
        }))
        .expect("required");
        assert_eq!(required.dependency_type, ModDependencyKind::Required);
        let incompatible = curseforge_dependency_from_value(&json!({
            "modId": 2,
            "relationType": 5
        }))
        .expect("incompatible");
        assert_eq!(
            incompatible.dependency_type,
            ModDependencyKind::Incompatible
        );
    }

    #[test]
    fn maps_authentication_status_to_the_existing_key_error() {
        assert_eq!(
            map_transport_error(
                "CurseForge request failed: HTTP status client error (401 Unauthorized)"
                    .to_string()
            ),
            "CurseForge rejected the configured API key. Check the key in Settings."
        );
    }
}
