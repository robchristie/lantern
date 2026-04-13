use serde::{Deserialize, Serialize};
use serde_json::json;
use url::Url;

use crate::{
    cdp::{CdpError, CdpWebSocket, TargetInfo},
    redaction::{RedactionMode, sanitize_title, sanitize_url},
};

pub const NAVIGATION_SCHEMA_VERSION: u8 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NavigationCommandOutput {
    pub schema_version: u8,
    pub command: &'static str,
    pub ok: bool,
    pub page: NavigationPageSummary,
    pub navigation: NavigationSummary,
}

impl NavigationCommandOutput {
    pub fn success(page: NavigationPageSummary, navigation: NavigationSummary) -> Self {
        Self {
            schema_version: NAVIGATION_SCHEMA_VERSION,
            command: "open",
            ok: true,
            page,
            navigation,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NavigationPageSummary {
    pub target_id: String,
    pub title: Option<String>,
    pub url_shape: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NavigationSummary {
    pub requested_url_shape: String,
    pub final_url_shape: Option<String>,
    pub loading_state: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NavigationError {
    UrlInvalid,
    TargetWebSocketMissing,
    NavigationFailed,
    Cdp(CdpError),
}

impl From<CdpError> for NavigationError {
    fn from(error: CdpError) -> Self {
        Self::Cdp(error)
    }
}

pub fn navigate_page(
    target: &TargetInfo,
    requested_url: &str,
    mode: RedactionMode,
) -> Result<NavigationCommandOutput, NavigationError> {
    let requested = NavigationUrl::parse(requested_url)?;
    let web_socket_debugger_url = target
        .web_socket_debugger_url
        .as_deref()
        .ok_or(NavigationError::TargetWebSocketMissing)?;

    let mut socket = CdpWebSocket::connect(web_socket_debugger_url)?;
    let result = socket.call(
        "Page.navigate",
        Some(json!({
            "url": requested.as_str()
        })),
    )?;
    let response: CdpNavigateResponse =
        serde_json::from_value(result).map_err(|source| CdpError::ResponseInvalid {
            context: "failed to parse CDP Page.navigate response",
            source: source.to_string(),
        })?;

    if response.error_text.is_some() {
        return Err(NavigationError::NavigationFailed);
    }

    let final_url_shape = navigation_history_current_url(&mut socket, mode)?;
    let loading_state = document_ready_state(&mut socket).unwrap_or(None);
    let page = NavigationPageSummary {
        target_id: target.id.clone(),
        title: target
            .title
            .as_ref()
            .map(|title| sanitize_title(title, mode)),
        url_shape: target.url.as_ref().and_then(|url| sanitize_url(url, mode)),
    };
    let navigation = NavigationSummary {
        requested_url_shape: requested.shape(mode),
        final_url_shape,
        loading_state,
    };

    Ok(NavigationCommandOutput::success(page, navigation))
}

pub fn validate_navigation_url(requested_url: &str) -> Result<(), NavigationError> {
    NavigationUrl::parse(requested_url).map(|_| ())
}

fn navigation_history_current_url(
    socket: &mut CdpWebSocket,
    mode: RedactionMode,
) -> Result<Option<String>, NavigationError> {
    let result = socket.call("Page.getNavigationHistory", None)?;
    let response: CdpNavigationHistoryResponse =
        serde_json::from_value(result).map_err(|source| CdpError::ResponseInvalid {
            context: "failed to parse CDP Page.getNavigationHistory response",
            source: source.to_string(),
        })?;

    let Some(entry) = response.entries.get(response.current_index as usize) else {
        return Ok(None);
    };

    Ok(navigation_url_shape(&entry.url, mode))
}

fn document_ready_state(socket: &mut CdpWebSocket) -> Result<Option<String>, NavigationError> {
    let result = socket.call(
        "Runtime.evaluate",
        Some(json!({
            "expression": "document.readyState",
            "returnByValue": true,
            "timeout": 1000
        })),
    )?;
    let response: CdpRuntimeEvaluateResponse =
        serde_json::from_value(result).map_err(|source| CdpError::ResponseInvalid {
            context: "failed to parse CDP Runtime.evaluate response",
            source: source.to_string(),
        })?;

    Ok(response.result.value)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CdpNavigateResponse {
    error_text: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CdpNavigationHistoryResponse {
    current_index: i64,
    entries: Vec<CdpNavigationHistoryEntry>,
}

#[derive(Debug, Deserialize)]
struct CdpNavigationHistoryEntry {
    url: String,
}

#[derive(Debug, Deserialize)]
struct CdpRuntimeEvaluateResponse {
    result: CdpRuntimeRemoteObject,
}

#[derive(Debug, Deserialize)]
struct CdpRuntimeRemoteObject {
    value: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NavigationUrl {
    raw: String,
}

impl NavigationUrl {
    fn parse(value: &str) -> Result<Self, NavigationError> {
        if value == "about:blank" {
            return Ok(Self {
                raw: value.to_owned(),
            });
        }

        let url = Url::parse(value).map_err(|_| NavigationError::UrlInvalid)?;
        if !matches!(url.scheme(), "http" | "https") {
            return Err(NavigationError::UrlInvalid);
        }
        if url.host().is_none() {
            return Err(NavigationError::UrlInvalid);
        }

        Ok(Self {
            raw: value.to_owned(),
        })
    }

    fn as_str(&self) -> &str {
        &self.raw
    }

    fn shape(&self, mode: RedactionMode) -> String {
        navigation_url_shape(&self.raw, mode).unwrap_or_else(|| self.raw.clone())
    }
}

fn navigation_url_shape(value: &str, mode: RedactionMode) -> Option<String> {
    if value == "about:blank" {
        return Some(value.to_owned());
    }

    sanitize_url(value, mode)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_http_https_and_about_blank_navigation_urls() {
        assert!(NavigationUrl::parse("http://localhost:5173/").is_ok());
        assert!(NavigationUrl::parse("https://example.test/path").is_ok());
        assert!(NavigationUrl::parse("about:blank").is_ok());
    }

    #[test]
    fn rejects_unsupported_navigation_urls() {
        for value in [
            "/relative",
            "about:srcdoc",
            "file:///tmp/index.html",
            "data:text/html,hello",
            "javascript:alert(1)",
            "chrome://version",
        ] {
            assert_eq!(
                NavigationUrl::parse(value),
                Err(NavigationError::UrlInvalid)
            );
        }
    }

    #[test]
    fn redacts_navigation_url_shapes() {
        let url = NavigationUrl::parse(
            "https://user:pass@example.test/reset/4a7f9c0e2d1b4c6a8e9f0123456789ab?token=secret#frag",
        )
        .expect("url should parse");

        assert_eq!(
            url.shape(RedactionMode::Redacted),
            "https://example.test/reset/:redacted"
        );
    }
}
