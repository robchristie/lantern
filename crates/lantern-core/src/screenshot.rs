use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::{
    cdp::{CdpError, CdpWebSocket, TargetInfo},
    redaction::{RedactionMode, sanitize_title, sanitize_url},
};

pub const SCREENSHOT_SCHEMA_VERSION: u8 = 1;
pub const SCREENSHOT_FORMAT: &str = "png";
pub const SCREENSHOT_REDACTION_CAVEAT: &str = "screenshot_contains_visible_page_pixels";

#[derive(Debug, Clone, PartialEq)]
pub struct CapturedScreenshot {
    pub page: ScreenshotPageSummary,
    pub format: &'static str,
    pub width: Option<u64>,
    pub height: Option<u64>,
    pub region: Option<ScreenshotRegion>,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ScreenshotCommandOutput {
    pub schema_version: u8,
    pub command: &'static str,
    pub ok: bool,
    pub page: ScreenshotPageSummary,
    pub screenshot: ScreenshotSummary,
}

impl ScreenshotCommandOutput {
    pub fn success(page: ScreenshotPageSummary, screenshot: ScreenshotSummary) -> Self {
        Self {
            schema_version: SCREENSHOT_SCHEMA_VERSION,
            command: "screenshot",
            ok: true,
            page,
            screenshot,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ScreenshotPageSummary {
    pub target_id: String,
    pub title: Option<String>,
    pub url_shape: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ScreenshotSummary {
    pub format: &'static str,
    pub width: Option<u64>,
    pub height: Option<u64>,
    pub region: Option<ScreenshotRegion>,
    pub byte_count: usize,
    pub path: String,
    pub overwritten: bool,
    pub redaction_caveat: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct ScreenshotRegion {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScreenshotError {
    TargetWebSocketMissing,
    Cdp(CdpError),
}

impl From<CdpError> for ScreenshotError {
    fn from(error: CdpError) -> Self {
        Self::Cdp(error)
    }
}

pub fn capture_visible_viewport_screenshot(
    target: &TargetInfo,
    mode: RedactionMode,
    region: Option<ScreenshotRegion>,
) -> Result<CapturedScreenshot, ScreenshotError> {
    let web_socket_debugger_url = target
        .web_socket_debugger_url
        .as_deref()
        .ok_or(ScreenshotError::TargetWebSocketMissing)?;

    let mut socket = CdpWebSocket::connect(web_socket_debugger_url)?;
    let viewport = viewport_metrics(&mut socket)?;
    let width = viewport
        .client_width
        .and_then(non_negative_finite_dimension);
    let height = viewport
        .client_height
        .and_then(non_negative_finite_dimension);
    let mut params = json!({
        "format": SCREENSHOT_FORMAT,
        "fromSurface": true,
        "captureBeyondViewport": false
    });
    if let Some(region) = region {
        params["clip"] = json!({
            "x": region.x + viewport.page_x.unwrap_or(0.0),
            "y": region.y + viewport.page_y.unwrap_or(0.0),
            "width": region.width,
            "height": region.height,
            "scale": 1.0
        });
    }

    let result = socket.call("Page.captureScreenshot", Some(params))?;
    let response: CdpCaptureScreenshotResponse =
        serde_json::from_value(result).map_err(|source| CdpError::ResponseInvalid {
            context: "failed to parse CDP Page.captureScreenshot response",
            source: source.to_string(),
        })?;
    let bytes = STANDARD
        .decode(response.data.as_bytes())
        .map_err(|source| CdpError::ResponseInvalid {
            context: "failed to decode CDP screenshot data",
            source: source.to_string(),
        })?;

    Ok(CapturedScreenshot {
        page: ScreenshotPageSummary {
            target_id: target.id.clone(),
            title: target
                .title
                .as_ref()
                .map(|title| sanitize_title(title, mode)),
            url_shape: target.url.as_ref().and_then(|url| sanitize_url(url, mode)),
        },
        format: SCREENSHOT_FORMAT,
        width,
        height,
        region,
        bytes,
    })
}

fn viewport_metrics(socket: &mut CdpWebSocket) -> Result<CdpViewport, ScreenshotError> {
    let result = socket.call("Page.getLayoutMetrics", None)?;
    let response: CdpLayoutMetricsResponse =
        serde_json::from_value(result).map_err(|source| CdpError::ResponseInvalid {
            context: "failed to parse CDP Page.getLayoutMetrics response",
            source: source.to_string(),
        })?;

    // CDP clip origins are page-relative; public crop coordinates are relative
    // to the visible viewport, including its scroll offset.
    Ok(response
        .css_visual_viewport
        .or(response.css_layout_viewport)
        .or(response.visual_viewport)
        .or(response.layout_viewport)
        .unwrap_or_default())
}

fn non_negative_finite_dimension(value: f64) -> Option<u64> {
    if value.is_finite() && value >= 0.0 {
        Some(value.round() as u64)
    } else {
        None
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CdpCaptureScreenshotResponse {
    data: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CdpLayoutMetricsResponse {
    layout_viewport: Option<CdpViewport>,
    visual_viewport: Option<CdpViewport>,
    css_layout_viewport: Option<CdpViewport>,
    css_visual_viewport: Option<CdpViewport>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CdpViewport {
    page_x: Option<f64>,
    page_y: Option<f64>,
    client_width: Option<f64>,
    client_height: Option<f64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_css_viewport_dimensions_before_legacy_dimensions() {
        let response: CdpLayoutMetricsResponse = serde_json::from_str(
            r#"{
                "layoutViewport": {"clientWidth": 800, "clientHeight": 600},
                "cssVisualViewport": {"clientWidth": 1280.4, "clientHeight": 720.2}
            }"#,
        )
        .expect("metrics response should parse");

        let viewport = response.css_visual_viewport.expect("css viewport");
        assert_eq!(
            viewport
                .client_width
                .and_then(non_negative_finite_dimension),
            Some(1280)
        );
        assert_eq!(
            viewport
                .client_height
                .and_then(non_negative_finite_dimension),
            Some(720)
        );
    }
}
