//! Bounded network and atomic-download port contracts.

use reqwest::blocking::Client;
use serde_json::Value;
use std::{
    fmt,
    path::PathBuf,
    time::Duration,
};

use crate::net;

/// A short, sanitized label used for diagnostics without exposing raw URLs or payloads.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ServiceLabel(String);

impl ServiceLabel {
    pub fn new(value: impl Into<String>) -> Result<Self, String> {
        let label = value.into();
        let trimmed = label.trim();
        let valid_character = |byte: u8| {
            byte.is_ascii_alphanumeric() || matches!(byte, b' ' | b'-' | b'_' | b'.')
        };
        if trimmed.is_empty() || trimmed.len() > 80 || !trimmed.bytes().all(valid_character) {
            return Err(
                "Service label must be 1-80 ASCII letters, digits, spaces, '.', '-', or '_'."
                    .to_string(),
            );
        }
        Ok(Self(trimmed.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ServiceLabel {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

fn redact_url_like(value: &str) -> String {
    let tokens = value
        .split_whitespace()
        .map(|token| {
            let lower = token.to_ascii_lowercase();
            if lower.contains("http://") || lower.contains("https://") {
                "[redacted-url]".to_string()
            } else {
                token.to_string()
            }
        })
        .collect::<Vec<_>>();
    tokens.join(" ")
}

/// Redacts URL-like tokens from network diagnostics before they reach user-facing errors.
pub fn sanitize_network_error(error: impl fmt::Display) -> String {
    redact_url_like(&error.to_string())
}

fn sanitize_reqwest_error(error: reqwest::Error) -> String {
    sanitize_network_error(error.without_url())
}

#[derive(Clone)]
pub struct NetworkRequest {
    pub url: String,
    pub service: ServiceLabel,
    pub max_bytes: u64,
    pub timeout: Duration,
    /// Headers are scoped to this request and are redacted from Debug output and sanitized errors.
    pub headers: Vec<(String, String)>,
}

impl NetworkRequest {
    pub fn new(
        url: impl Into<String>,
        service: ServiceLabel,
        max_bytes: u64,
        timeout: Duration,
    ) -> Self {
        Self {
            url: url.into(),
            service,
            max_bytes,
            timeout,
            headers: Vec::new(),
        }
    }

    /// Adds a request-scoped header. Providers use this for credentials and
    /// content negotiation without exposing them in URLs or diagnostics.
    pub fn with_header(
        mut self,
        name: impl Into<String>,
        value: impl Into<String>,
    ) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }
}

impl fmt::Debug for NetworkRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let headers = format!("[{} redacted header(s)]", self.headers.len());
        formatter
            .debug_struct("NetworkRequest")
            .field("url", &"[redacted-url]")
            .field("service", &self.service)
            .field("max_bytes", &self.max_bytes)
            .field("timeout", &self.timeout)
            .field("headers", &headers)
            .finish()
    }
}

#[derive(Debug, Clone)]
pub struct DownloadRequest {
    pub url: String,
    pub destination: PathBuf,
    pub expected_sha1: Option<String>,
    pub service: ServiceLabel,
    pub max_bytes: u64,
    pub timeout: Duration,
}

impl DownloadRequest {
    pub fn new(
        url: impl Into<String>,
        destination: impl Into<PathBuf>,
        expected_sha1: Option<String>,
        service: ServiceLabel,
        max_bytes: u64,
        timeout: Duration,
    ) -> Self {
        Self {
            url: url.into(),
            destination: destination.into(),
            expected_sha1,
            service,
            max_bytes,
            timeout,
        }
    }
}

/// Object-safe network contract for bounded reads and atomic downloads.
pub trait NetworkPort: Send + Sync {
    fn read_json(&self, request: &NetworkRequest) -> Result<Value, String>;
    fn read_text(&self, request: &NetworkRequest) -> Result<String, String>;
    fn read_bytes(&self, request: &NetworkRequest) -> Result<Vec<u8>, String>;
    fn download_atomic(&self, request: &DownloadRequest) -> Result<u64, String>;
}

/// Concrete blocking Reqwest adapter for [`NetworkPort`].
pub struct ReqwestNetwork {
    client: Client,
}

impl ReqwestNetwork {
    pub fn new(timeout: Duration) -> Result<Self, String> {
        net::http_client(timeout)
            .map(|client| Self { client })
            .map_err(sanitize_network_error)
    }

    pub fn from_client(client: Client) -> Self {
        Self { client }
    }
}

impl NetworkPort for ReqwestNetwork {
    fn read_json(&self, request: &NetworkRequest) -> Result<Value, String> {
        let bytes = self.read_bytes(request)?;
        serde_json::from_slice(&bytes).map_err(|error| {
            format!(
                "{} response is not valid JSON: {}",
                request.service,
                sanitize_network_error(error)
            )
        })
    }

    fn read_text(&self, request: &NetworkRequest) -> Result<String, String> {
        let bytes = self.read_bytes(request)?;
        String::from_utf8(bytes).map_err(|error| {
            format!(
                "{} response is not valid UTF-8: {}",
                request.service,
                sanitize_network_error(error)
            )
        })
    }

    fn read_bytes(&self, request: &NetworkRequest) -> Result<Vec<u8>, String> {
        net::validate_https_url(&request.url)
            .map_err(|error| {
                format!(
                    "{} request rejected: {}",
                    request.service,
                    sanitize_network_error(error)
                )
            })?;
        let mut builder = self
            .client
            .get(request.url.clone())
            .timeout(request.timeout);
        for (name, value) in &request.headers {
            builder = builder.header(name.as_str(), value.as_str());
        }
        let response = builder
            .send()
            .and_then(|response| response.error_for_status())
            .map_err(|error| {
                format!(
                    "{} request failed: {}",
                    request.service,
                    sanitize_reqwest_error(error)
                )
            })?;
        net::read_body_limited(response, request.max_bytes).map_err(|error| {
            format!(
                "{} request failed: {}",
                request.service,
                sanitize_network_error(error)
            )
        })
    }

    fn download_atomic(&self, request: &DownloadRequest) -> Result<u64, String> {
        let client = net::http_client(request.timeout).map_err(|error| {
            format!(
                "{} download setup failed: {}",
                request.service,
                sanitize_network_error(error)
            )
        })?;
        net::download_atomic(
            &client,
            &request.url,
            &request.destination,
            request.expected_sha1.as_deref(),
            request.max_bytes,
            request.service.as_str(),
        )
        .map_err(|error| {
            format!(
                "{}: {}",
                request.service,
                sanitize_network_error(error)
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn service_labels_are_short_and_sanitized() {
        assert_eq!(ServiceLabel::new(" Example Service ").unwrap().as_str(), "Example Service");
        assert!(ServiceLabel::new("bad/service").is_err());
        assert!(ServiceLabel::new("bad\nservice").is_err());
        assert!(ServiceLabel::new("").is_err());
    }

    #[test]
    fn network_requests_accept_headers_without_network_access() {
        let request = NetworkRequest::new(
            "https://example.com/api",
            ServiceLabel::new("Example").unwrap(),
            1024,
            Duration::from_secs(1),
        )
        .with_header("x-api-key", "test-key")
        .with_header("Accept", "application/json");
        assert_eq!(
            request.headers,
            vec![
                ("x-api-key".to_string(), "test-key".to_string()),
                ("Accept".to_string(), "application/json".to_string()),
            ]
        );
    }

    #[test]
    fn network_request_debug_redacts_url_queries_and_credentials() {
        let request = NetworkRequest::new(
            "https://api.example.com/mods/search?token=url-secret&api_key=query-secret",
            ServiceLabel::new("Example").unwrap(),
            1024,
            Duration::from_secs(45),
        )
        .with_header("x-api-key", "credential-secret")
        .with_header("Authorization", "Bearer bearer-secret");
        let debug = format!("{request:?}");
        assert!(debug.contains("Example"));
        assert!(debug.contains("2 redacted header(s)"));
        assert!(!debug.contains("api.example.com"));
        assert!(!debug.contains("url-secret"));
        assert!(!debug.contains("query-secret"));
        assert!(!debug.contains("x-api-key"));
        assert!(!debug.contains("credential-secret"));
        assert!(!debug.contains("Authorization"));
        assert!(!debug.contains("bearer-secret"));
    }

    #[test]
    fn network_errors_redact_urls_and_query_secrets() {
        let sanitized = sanitize_network_error(
            "request failed for url (https://api.example.com/path?token=super-secret&key=abc): connection reset",
        );
        assert!(sanitized.contains("[redacted-url]"));
        assert!(!sanitized.contains("api.example.com"));
        assert!(!sanitized.contains("super-secret"));
        assert!(sanitized.contains("connection reset"));

        let uppercase = sanitize_network_error("HTTP://example.com?token=secret timed out");
        assert!(!uppercase.contains("example.com"));
        assert!(!uppercase.contains("secret"));
    }

    #[test]
    fn network_port_is_object_safe_and_send_sync() {
        fn assert_send_sync<T: ?Sized + Send + Sync>() {}
        assert_send_sync::<dyn NetworkPort>();
        assert_send_sync::<ReqwestNetwork>();
    }
}
