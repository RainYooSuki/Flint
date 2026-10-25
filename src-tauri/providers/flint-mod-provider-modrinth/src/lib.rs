//! Modrinth implementation of the source-neutral mod provider contract.

use flint_core::ports::network::sanitize_network_error;
use flint_core::ports::{
    NetworkPort, NetworkRequest, ServiceLabel,
};
use flint_provider_api::{
    descriptor_for, validate_mod_provider_id, ModDependencyData, ModDependencyKind,
    ModDependencyVersionRequest, ModListVersionsRequest, ModProjectData, ModProvider,
    ModProviderContext, ModSearchRequest, ModSearchResultData, ModVersionData, ModVersionRequest,
    Provider, ProviderDescriptor, MODRINTH_CAPABILITIES, MODRINTH_PROVIDER_ID,
};
use serde_json::Value;

const API_BASE: &str = "https://api.modrinth.com/v2";
const SERVICE: &str = "Modrinth";
const MAX_API_BODY: u64 = 16 * 1024 * 1024;

pub const DESCRIPTOR: ProviderDescriptor = descriptor_for(MODRINTH_PROVIDER_ID, MODRINTH_CAPABILITIES);

#[derive(Debug, Clone, Copy, Default)]
pub struct ModrinthModProvider;

impl ModrinthModProvider {
    pub const fn new() -> Self {
        Self
    }
}

impl Provider for ModrinthModProvider {
    fn descriptor(&self) -> ProviderDescriptor {
        DESCRIPTOR
    }
}

impl ModProvider for ModrinthModProvider {
    fn search(
        &self,
        context: &ModProviderContext<'_>,
        request: &ModSearchRequest,
    ) -> Result<ModSearchResultData, String> {
        let facets = if request.loader == "vanilla" {
            serde_json::json!([
                ["project_type:mod"],
                ["versions:{}", request.game_version.as_str()]
            ])
        } else {
            serde_json::json!([
                ["project_type:mod"],
                ["versions:{}", request.game_version.as_str()],
                ["categories:{}", request.loader.as_str()]
            ])
        }
        .to_string();
        let params = vec![
            ("query", request.query.clone()),
            ("facets", facets),
            ("offset", request.offset.to_string()),
            ("limit", request.limit.to_string()),
        ];
        let value = fetch_json(
            context,
            query_url(&format!("{API_BASE}/search"), &params),
        )?;
        let hits = value
            .get("hits")
            .and_then(Value::as_array)
            .ok_or_else(|| "Modrinth search response has no hits array.".to_string())?;
        let projects = hits
            .iter()
            .filter_map(modrinth_project_from_search)
            .collect();
        Ok(ModSearchResultData {
            total: u64_field(&value, "total_hits"),
            offset: request.offset,
            limit: request.limit,
            hits: projects,
        })
    }

    fn list_versions(
        &self,
        context: &ModProviderContext<'_>,
        request: &ModListVersionsRequest,
    ) -> Result<Vec<ModVersionData>, String> {
        modrinth_list_versions(
            context,
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
        modrinth_version_by_id(context, &request.version_id)
    }

    fn dependency_version(
        &self,
        context: &ModProviderContext<'_>,
        request: &ModDependencyVersionRequest,
    ) -> Result<Option<ModVersionData>, String> {
        if let Some(version_id) = request.dependency.version_id.as_deref() {
            return modrinth_version_by_id(context, version_id).map(Some);
        }
        if request.dependency.project_id.is_empty() {
            return Ok(None);
        }
        let versions = modrinth_list_versions(
            context,
            &request.dependency.project_id,
            &request.game_version,
            &request.loader,
        )?;
        Ok(versions.into_iter().next())
    }
}

fn fetch_json(context: &ModProviderContext<'_>, url: String) -> Result<Value, String> {
    context.check_control()?;
    let service = ServiceLabel::new(SERVICE)
        .map_err(|error| format!("Invalid Modrinth service label: {error}"))?;
    let request = NetworkRequest::new(url, service, MAX_API_BODY, context.timeout())
        .with_header("Accept", "application/json");
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

fn modrinth_project_from_search(value: &Value) -> Option<ModProjectData> {
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
        .map(str::to_string)
        .unwrap_or_else(|| fallback_url.clone());
    let project_url = optional_https_url(&project_url).unwrap_or(fallback_url);
    Some(ModProjectData {
        id,
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
    context: &ModProviderContext<'_>,
    project_id: &str,
    game_version: &str,
    loader: &str,
) -> Result<Vec<ModVersionData>, String> {
    validate_mod_provider_id(project_id)?;
    let game_versions = serde_json::to_string(&[game_version])
        .map_err(|error| format!("Cannot encode game-version filter: {error}"))?;
    let mut params = vec![("game_versions", game_versions)];
    if loader != "vanilla" {
        let loaders = serde_json::to_string(&[loader])
            .map_err(|error| format!("Cannot encode loader filter: {error}"))?;
        params.push(("loaders", loaders));
    }
    let value = fetch_json(
        context,
        query_url(&format!("{API_BASE}/project/{project_id}/version"), &params),
    )?;
    let versions = value
        .as_array()
        .ok_or_else(|| "Modrinth version response is not an array.".to_string())?;
    let mut result = versions
        .iter()
        .filter_map(|value| modrinth_version_from_value(value, project_id))
        .collect::<Vec<_>>();
    result.sort_by(|left, right| {
        right
            .published_at
            .as_deref()
            .unwrap_or_default()
            .cmp(left.published_at.as_deref().unwrap_or_default())
    });
    Ok(result)
}

fn modrinth_version_by_id(
    context: &ModProviderContext<'_>,
    version_id: &str,
) -> Result<ModVersionData, String> {
    validate_mod_provider_id(version_id)?;
    let value = fetch_json(context, format!("{API_BASE}/version/{version_id}"))?;
    modrinth_version_from_value(&value, "")
        .ok_or_else(|| "Modrinth returned an invalid version object.".to_string())
}

fn modrinth_version_from_value(
    value: &Value,
    fallback_project_id: &str,
) -> Option<ModVersionData> {
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
    Some(ModVersionData {
        id,
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
            .map(str::to_string),
        dependencies,
    })
}

fn modrinth_dependency_from_value(value: &Value) -> Option<ModDependencyData> {
    let dependency_type = match value
        .get("dependency_type")
        .and_then(Value::as_str)
        .unwrap_or("required")
    {
        "required" => ModDependencyKind::Required,
        "optional" => ModDependencyKind::Optional,
        "embedded" => ModDependencyKind::Embedded,
        "incompatible" => ModDependencyKind::Incompatible,
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
        .map(str::to_string)
        .or_else(|| {
            if project_id.is_empty() {
                None
            } else {
                Some(format!("Modrinth project {project_id}"))
            }
        })
        .unwrap_or_else(|| "Unknown Modrinth dependency".to_string());
    Some(ModDependencyData {
        project_id,
        version_id,
        name,
        dependency_type,
    })
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
    use flint_provider_api::Provider as _;
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
        let network = FakeNetwork::new(json!({"hits": [], "total_hits": 0}));
        let progress = NoopProgress;
        let logger = NoopLogger;
        let search_request = || ModSearchRequest {
            query: "sodium".to_string(),
            game_version: "1.20.1".to_string(),
            loader: "fabric".to_string(),
            offset: 0,
            limit: 20,
        };
        for seconds in [45, 45, 60, 600] {
            let context = ModProviderContext::new(
                &network,
                None,
                &progress,
                &logger,
                Duration::from_secs(seconds),
                Deadline::after(Duration::from_secs(30)),
                Cancellation::new(),
            );
            ModrinthModProvider::new()
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
    fn preserves_transport_and_body_limit_error_wording() {
        let progress = NoopProgress;
        let logger = NoopLogger;
        let request = || ModSearchRequest {
            query: "sodium".to_string(),
            game_version: "1.20.1".to_string(),
            loader: "fabric".to_string(),
            offset: 0,
            limit: 20,
        };
        let cases = [
            (
                "Modrinth request failed: connection reset while opening https://api.example.com/path?token=super-secret".to_string(),
                "Cannot contact Modrinth: connection reset while opening [redacted-url]".to_string(),
            ),
            (
                "Modrinth request failed: HTTP response is too large (999 bytes, limit 16777216).".to_string(),
                "HTTP response is too large (999 bytes, limit 16777216).".to_string(),
            ),
            (
                "Modrinth request failed: HTTP response exceeds the 16777216 byte limit.".to_string(),
                "HTTP response exceeds the 16777216 byte limit.".to_string(),
            ),
            (
                "Modrinth request failed: Cannot read HTTP response: connection reset".to_string(),
                "Cannot read HTTP response: connection reset".to_string(),
            ),
        ];
        for (raw_error, expected) in cases {
            let network = FakeNetwork::failing(raw_error);
            let context = ModProviderContext::new(
                &network,
                None,
                &progress,
                &logger,
                Duration::from_secs(45),
                Deadline::after(Duration::from_secs(30)),
                Cancellation::new(),
            );
            let error = ModrinthModProvider::new()
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
                "Modrinth request failed: HTTP status client error (404 Not Found)".to_string()
            ),
            "Modrinth did not find the requested project or version."
        );
        assert_eq!(
            map_transport_error(
                "Modrinth request failed: HTTP status server error (500 Internal Server Error)"
                    .to_string()
            ),
            "Modrinth request failed with HTTP 500."
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
        let descriptor = ModrinthModProvider::new().descriptor();
        assert_eq!(descriptor.id, MODRINTH_PROVIDER_ID);
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
            flint_provider_api::MODRINTH_SELECTION_KEY
        );
    }

    #[test]
    fn maps_modrinth_search_to_neutral_data() {
        let network = FakeNetwork::new(json!({
            "hits": [{
                "project_id": "AANobbMI",
                "slug": "sodium",
                "title": "Sodium",
                "description": "Rendering engine",
                "author": "Author",
                "downloads": 100,
                "icon_url": "https://media.example/icon.png",
                "categories": ["optimization"],
                "project_url": "https://modrinth.com/mod/sodium"
            }],
            "total_hits": 1
        }));
        let progress = NoopProgress;
        let logger = NoopLogger;
        let context = ModProviderContext::new(
            &network,
            None,
            &progress,
            &logger,
            Duration::from_secs(45),
            Deadline::after(Duration::from_secs(30)),
            Cancellation::new(),
        );
        let result = ModrinthModProvider::new()
            .search(
                &context,
                &ModSearchRequest {
                    query: "sodium".to_string(),
                    game_version: "1.20.1".to_string(),
                    loader: "fabric".to_string(),
                    offset: 0,
                    limit: 20,
                },
            )
            .expect("search result");
        assert_eq!(result.total, 1);
        assert_eq!(result.hits[0].slug, "sodium");
        let requests = network.requests.lock().unwrap();
        assert_eq!(requests.len(), 1);
        assert!(requests[0].url.contains("query=sodium"));
        assert!(requests[0].url.contains("facets="));
    }

    #[test]
    fn maps_modrinth_version_and_dependencies_to_neutral_data() {
        let version = modrinth_version_from_value(
            &json!({
                "id": "version-id",
                "project_id": "project-id",
                "name": "Version",
                "version_number": "1.0.0",
                "version_type": "release",
                "date_published": "2026-01-01T00:00:00Z",
                "game_versions": ["1.20.1"],
                "loaders": ["fabric"],
                "files": [{
                    "primary": true,
                    "filename": "mod.jar",
                    "url": "https://cdn.example/mod.jar"
                }],
                "dependencies": [{
                    "project_id": "dep-project",
                    "version_id": "dep-version",
                    "dependency_type": "required",
                    "file_name": "Dependency"
                }]
            }),
            "",
        )
        .expect("version mapping");
        assert_eq!(version.project_id, "project-id");
        assert_eq!(version.dependencies.len(), 1);
        assert_eq!(
            version.dependencies[0].dependency_type,
            ModDependencyKind::Required
        );
    }
}
