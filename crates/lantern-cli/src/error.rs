use crate::output::CLI_SCHEMA_VERSION;
use lantern_core::cdp::CdpError;
use lantern_core::console::ConsoleReadError;
use lantern_core::dom::DomReadError;
use lantern_core::endpoint::EndpointResolutionError;
use lantern_core::flow::FlowError;
use lantern_core::interaction::InteractionError;
use lantern_core::layout::LayoutReadError;
use lantern_core::navigation::NavigationError;
use lantern_core::network::NetworkReadError;
use lantern_core::screenshot::ScreenshotError;
use lantern_core::wait::WaitError;
use serde::Serialize;

const ENDPOINT_MISSING_MESSAGE: &str = "No CDP endpoint configured.";

const ENDPOINT_MISSING_HINT: &str =
    "Pass --endpoint http://127.0.0.1:9222 or set LANTERN_CDP_ENDPOINT.";

const ENDPOINT_INVALID_MESSAGE: &str = "Invalid CDP endpoint.";

const ENDPOINT_INVALID_HINT: &str = "Use a local HTTP endpoint such as http://127.0.0.1:9222 without credentials, query, or fragment.";

const ENDPOINT_UNREACHABLE_MESSAGE: &str = "CDP endpoint could not be reached.";

const ENDPOINT_UNREACHABLE_HINT: &str =
    "Confirm Chromium is running with --remote-debugging-port and retry.";

const CDP_UNHEALTHY_MESSAGE: &str = "CDP endpoint did not behave like Chromium CDP.";

const CDP_UNHEALTHY_HINT: &str =
    "Check that the endpoint is Chromium's HTTP DevTools endpoint, not a page URL.";

pub(crate) const CDP_RESPONSE_INVALID_MESSAGE: &str = "CDP endpoint returned an invalid response.";

pub(crate) const CDP_RESPONSE_INVALID_HINT: &str =
    "Retry against a fresh Chromium DevTools endpoint or inspect /json/version and /json/list.";

pub(crate) const TARGET_NOT_FOUND_MESSAGE: &str = "No page target was available.";

pub(crate) const TARGET_NOT_FOUND_HINT: &str =
    "Open a page in the attached Chromium instance and retry.";

pub(crate) const TARGET_ID_NOT_FOUND_MESSAGE: &str =
    "No page target matched the requested target id.";

pub(crate) const TARGET_ID_NOT_FOUND_HINT: &str =
    "Run lantern targets and pass the exact id of a page target.";

pub(crate) const TARGET_AMBIGUOUS_MESSAGE: &str = "Multiple page targets matched.";

pub(crate) const TARGET_AMBIGUOUS_HINT: &str =
    "Close extra pages, leave exactly one attached page target, or pass --target-id.";

const TARGET_WEBSOCKET_MISSING_MESSAGE: &str =
    "Selected page target did not expose a WebSocket debugger URL.";

const TARGET_WEBSOCKET_MISSING_HINT: &str = "Refresh the target list, open a normal page target, or restart Chromium with remote debugging enabled.";

const URL_INVALID_MESSAGE: &str = "Invalid navigation URL.";

const URL_INVALID_HINT: &str =
    "Pass an absolute http:// or https:// URL, or the exact URL about:blank.";

const NAVIGATION_FAILED_MESSAGE: &str = "Chromium failed the requested page navigation.";

const NAVIGATION_FAILED_HINT: &str = "Check that the URL is reachable from Chromium, then retry.";

pub(crate) const WAIT_TIMEOUT_INVALID_MESSAGE: &str = "Invalid wait timeout.";

pub(crate) const WAIT_TIMEOUT_INVALID_HINT: &str = "Pass --timeout-ms from 1 through 30000; for quiet waits, --quiet-ms must also be in range and no larger than --timeout-ms.";

pub(crate) const INTERACTION_TIMEOUT_INVALID_MESSAGE: &str = "Invalid interaction timeout.";

pub(crate) const INTERACTION_TIMEOUT_INVALID_HINT: &str = "Pass --timeout-ms from 1 through 30000.";

pub(crate) const TYPE_INPUT_INVALID_MESSAGE: &str = "Interaction input could not be used.";

pub(crate) const TYPE_INPUT_INVALID_HINT: &str =
    "Check local input access, type, permissions, UTF-8 encoding, and the 64 KiB limit.";

pub(crate) const TYPE_INPUT_TOO_LARGE_MESSAGE: &str = "Interaction input exceeds the size limit.";

pub(crate) const TYPE_INPUT_TOO_LARGE_HINT: &str = "Use interaction input no larger than 64 KiB.";

pub(crate) const INTERACTION_DURATION_INVALID_MESSAGE: &str = "Invalid interaction duration.";

pub(crate) const INTERACTION_DURATION_INVALID_HINT: &str =
    "Pass --duration-ms from 0 through 30000.";

pub(crate) const INTERACTION_DELTA_INVALID_MESSAGE: &str = "Invalid interaction delta.";

pub(crate) const INTERACTION_DELTA_INVALID_HINT: &str =
    "Pass finite --dx/--dy values, and for wheel at least one non-zero delta.";

pub(crate) const SCREENSHOT_OUTPUT_EXISTS_MESSAGE: &str = "Screenshot output path already exists.";

pub(crate) const SCREENSHOT_OUTPUT_EXISTS_HINT: &str =
    "Pass --overwrite to replace the file, or choose a different --output path.";

pub(crate) const SCREENSHOT_WRITE_FAILED_MESSAGE: &str = "Screenshot could not be written.";

pub(crate) const SCREENSHOT_WRITE_FAILED_HINT: &str =
    "Check that the parent directory exists and the output path is writable.";

pub(crate) const SCREENSHOT_REGION_INVALID_MESSAGE: &str = "Invalid screenshot region.";

pub(crate) const SCREENSHOT_REGION_INVALID_HINT: &str = "Pass all of --region-x, --region-y, --region-width, and --region-height with non-negative x/y and positive width/height.";

pub(crate) const DOM_LIMIT_INVALID_MESSAGE: &str = "Invalid DOM summary limit.";

pub(crate) const DOM_LIMIT_INVALID_HINT: &str =
    "Pass --depth from 1 through 12 and --max-nodes from 1 through 500.";

pub(crate) const BROWSER_USAGE_HINT: &str =
    "Run lantern browser <start|list|status|endpoint|stop|prune|profile>.";

pub(crate) const BROWSER_ID_MISSING_MESSAGE: &str = "Missing browser instance id.";

pub(crate) const BROWSER_ID_MISSING_HINT: &str =
    "Run lantern browser list, then pass the id to status, endpoint, or stop.";

pub(crate) const BROWSER_RUNTIME_INVALID_MESSAGE: &str = "Invalid browser runtime.";

pub(crate) const BROWSER_RUNTIME_INVALID_HINT: &str = "Use --runtime podman or --runtime docker.";

pub(crate) const BROWSER_GRAPHICS_INVALID_MESSAGE: &str = "Invalid browser graphics mode.";

pub(crate) const BROWSER_GRAPHICS_INVALID_HINT: &str =
    "Use --graphics disabled, --graphics swiftshader, --graphics gpu, or --graphics webgpu.";

pub(crate) const BROWSER_GPU_DEVICE_INVALID_MESSAGE: &str = "Invalid browser GPU device.";

pub(crate) const BROWSER_GPU_DEVICE_INVALID_HINT: &str = "Pass one explicit container-runtime device selector such as nvidia.com/gpu=0 or /dev/dri/renderD128.";

pub(crate) const BROWSER_GPU_DEVICE_MODE_MESSAGE: &str =
    "Browser GPU device requires a hardware graphics mode.";

pub(crate) const BROWSER_GPU_DEVICE_MODE_HINT: &str =
    "Use --gpu-device only with --graphics gpu or --graphics webgpu.";

pub(crate) const BROWSER_WEBGPU_DEVICE_REQUIRED_MESSAGE: &str =
    "Hardware WebGPU requires an explicit GPU device.";

pub(crate) const BROWSER_WEBGPU_DEVICE_REQUIRED_HINT: &str =
    "Pass --gpu-device with a runtime selector such as nvidia.com/gpu=0.";

pub(crate) const BROWSER_RUNTIME_UNAVAILABLE_MESSAGE: &str =
    "No supported container runtime was found.";

pub(crate) const BROWSER_RUNTIME_UNAVAILABLE_HINT: &str =
    "Install podman or docker, or pass --runtime to select an installed runtime.";

pub(crate) const BROWSER_RUNTIME_FAILED_MESSAGE: &str = "Container runtime command failed.";

pub(crate) const BROWSER_RUNTIME_FAILED_HINT: &str =
    "Check that the browser image is built and that the selected runtime is available.";

pub(crate) const BROWSER_HOST_GATEWAY_INVALID_MESSAGE: &str =
    "Invalid browser host-gateway hostname.";

pub(crate) const BROWSER_HOST_GATEWAY_INVALID_HINT: &str =
    "Pass one DNS hostname, such as app.example.test; IP addresses and URLs are not accepted.";

pub(crate) const BROWSER_PORT_MISSING_MESSAGE: &str = "Managed browser CDP port was not published.";

pub(crate) const BROWSER_PORT_MISSING_HINT: &str =
    "Inspect the container port mapping and confirm CDP was published to host loopback.";

pub(crate) const BROWSER_NOT_READY_MESSAGE: &str = "Managed browser did not become ready.";

pub(crate) const BROWSER_NOT_READY_HINT: &str =
    "Check the container logs, then stop or prune the failed managed browser instance.";

pub(crate) const BROWSER_STATE_FAILED_MESSAGE: &str = "Managed browser state could not be updated.";

pub(crate) const BROWSER_STATE_FAILED_HINT: &str = "Check repository-local disposable state and the operator-owned Lantern state-home permissions, then retry.";

pub(crate) const BROWSER_PROFILE_MISSING_MESSAGE: &str = "Missing browser profile name.";

pub(crate) const BROWSER_PROFILE_MISSING_HINT: &str =
    "Pass a profile name made from ASCII letters, numbers, hyphen, or underscore.";

pub(crate) const BROWSER_PROFILE_NOT_FOUND_MESSAGE: &str = "Managed browser profile was not found.";

pub(crate) const BROWSER_PROFILE_NOT_FOUND_HINT: &str =
    "Run lantern browser profile list, or create the named profile first.";

pub(crate) const BROWSER_PROFILE_EXISTS_MESSAGE: &str = "Managed browser profile already exists.";

pub(crate) const BROWSER_PROFILE_EXISTS_HINT: &str =
    "Reuse it with lantern browser start --profile NAME, or choose another name.";

pub(crate) const BROWSER_PROFILE_IN_USE_MESSAGE: &str =
    "Managed browser profile is already in use.";

pub(crate) const BROWSER_PROFILE_IN_USE_HINT: &str =
    "Stop the attached managed browser before starting or deleting this profile.";

pub(crate) const BROWSER_PROFILE_DELETE_CONFIRM_MESSAGE: &str =
    "Profile deletion requires confirmation.";

pub(crate) const BROWSER_PROFILE_DELETE_CONFIRM_HINT: &str =
    "Review the dedicated profile name, then repeat with --yes.";

pub(crate) const BROWSER_STATE_HOME_INVALID_MESSAGE: &str = "Lantern state home is invalid.";

pub(crate) const BROWSER_STATE_HOME_INVALID_HINT: &str = "Set LANTERN_STATE_HOME or XDG_STATE_HOME to an absolute operator-owned directory, or configure HOME.";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CliError {
    pub(crate) exit_code: u8,
    pub(crate) json: bool,
    pub(crate) code: &'static str,
    pub(crate) message: &'static str,
    pub(crate) hint: &'static str,
}

impl CliError {
    pub(crate) fn usage(json: bool, message: &'static str, hint: &'static str) -> Self {
        Self {
            exit_code: 2,
            json,
            code: "usage",
            message,
            hint,
        }
    }

    pub(crate) fn runtime(
        json: bool,
        code: &'static str,
        message: &'static str,
        hint: &'static str,
    ) -> Self {
        Self {
            exit_code: 1,
            json,
            code,
            message,
            hint,
        }
    }

    pub(crate) fn with_code(mut self, code: &'static str) -> Self {
        self.code = code;
        self
    }

    pub(crate) fn with_json(mut self, json: bool) -> Self {
        self.json = json;
        self
    }

    pub(crate) fn from_endpoint(error: EndpointResolutionError, json: bool) -> Self {
        match error {
            EndpointResolutionError::Missing => Self {
                exit_code: 2,
                json,
                code: "endpoint_missing",
                message: ENDPOINT_MISSING_MESSAGE,
                hint: ENDPOINT_MISSING_HINT,
            },
            EndpointResolutionError::Invalid { .. } => Self {
                exit_code: 2,
                json,
                code: "endpoint_invalid",
                message: ENDPOINT_INVALID_MESSAGE,
                hint: ENDPOINT_INVALID_HINT,
            },
        }
    }

    pub(crate) fn from_cdp(error: CdpError, json: bool) -> Self {
        match error {
            CdpError::CommandUncertain { .. } => Self::runtime(
                json,
                "cdp_command_uncertain",
                "CDP command completion is uncertain.",
                "The command may have executed. Inspect browser state before deciding whether another action is needed; do not automatically replay it.",
            ),
            CdpError::EndpointUrlInvalid { .. } | CdpError::WebSocketUrlInvalid { .. } => Self {
                exit_code: 2,
                json,
                code: "endpoint_invalid",
                message: ENDPOINT_INVALID_MESSAGE,
                hint: ENDPOINT_INVALID_HINT,
            },
            CdpError::Unreachable { .. } => Self::runtime(
                json,
                "endpoint_unreachable",
                ENDPOINT_UNREACHABLE_MESSAGE,
                ENDPOINT_UNREACHABLE_HINT,
            ),
            CdpError::HttpStatus { .. }
            | CdpError::WebSocketTransport { .. }
            | CdpError::Command { .. } => Self::runtime(
                json,
                "cdp_unhealthy",
                CDP_UNHEALTHY_MESSAGE,
                CDP_UNHEALTHY_HINT,
            ),
            CdpError::ResponseInvalid { .. } => Self::runtime(
                json,
                "cdp_response_invalid",
                CDP_RESPONSE_INVALID_MESSAGE,
                CDP_RESPONSE_INVALID_HINT,
            ),
        }
    }

    pub(crate) fn from_dom_read(error: DomReadError, json: bool) -> Self {
        match error {
            DomReadError::TargetWebSocketMissing => Self::runtime(
                json,
                "target_websocket_missing",
                TARGET_WEBSOCKET_MISSING_MESSAGE,
                TARGET_WEBSOCKET_MISSING_HINT,
            ),
            DomReadError::Cdp(CdpError::WebSocketTransport { .. }) => Self::runtime(
                json,
                "endpoint_unreachable",
                ENDPOINT_UNREACHABLE_MESSAGE,
                ENDPOINT_UNREACHABLE_HINT,
            ),
            DomReadError::Cdp(CdpError::Command { .. } | CdpError::ResponseInvalid { .. }) => {
                Self::runtime(
                    json,
                    "cdp_response_invalid",
                    CDP_RESPONSE_INVALID_MESSAGE,
                    CDP_RESPONSE_INVALID_HINT,
                )
            }
            DomReadError::Cdp(CdpError::WebSocketUrlInvalid { .. }) => Self::runtime(
                json,
                "cdp_unhealthy",
                CDP_UNHEALTHY_MESSAGE,
                CDP_UNHEALTHY_HINT,
            ),
            DomReadError::Cdp(error) => Self::from_cdp(error, json),
        }
    }

    pub(crate) fn from_navigation(error: NavigationError, json: bool) -> Self {
        match error {
            NavigationError::UrlInvalid => Self {
                exit_code: 2,
                json,
                code: "url_invalid",
                message: URL_INVALID_MESSAGE,
                hint: URL_INVALID_HINT,
            },
            NavigationError::TargetWebSocketMissing => Self::runtime(
                json,
                "target_websocket_missing",
                TARGET_WEBSOCKET_MISSING_MESSAGE,
                TARGET_WEBSOCKET_MISSING_HINT,
            ),
            NavigationError::NavigationFailed => Self::runtime(
                json,
                "navigation_failed",
                NAVIGATION_FAILED_MESSAGE,
                NAVIGATION_FAILED_HINT,
            ),
            NavigationError::Cdp(CdpError::WebSocketTransport { .. }) => Self::runtime(
                json,
                "endpoint_unreachable",
                ENDPOINT_UNREACHABLE_MESSAGE,
                ENDPOINT_UNREACHABLE_HINT,
            ),
            NavigationError::Cdp(CdpError::Command { .. }) => Self::runtime(
                json,
                "navigation_failed",
                NAVIGATION_FAILED_MESSAGE,
                NAVIGATION_FAILED_HINT,
            ),
            NavigationError::Cdp(CdpError::ResponseInvalid { .. }) => Self::runtime(
                json,
                "cdp_response_invalid",
                CDP_RESPONSE_INVALID_MESSAGE,
                CDP_RESPONSE_INVALID_HINT,
            ),
            NavigationError::Cdp(CdpError::WebSocketUrlInvalid { .. }) => Self::runtime(
                json,
                "cdp_unhealthy",
                CDP_UNHEALTHY_MESSAGE,
                CDP_UNHEALTHY_HINT,
            ),
            NavigationError::Cdp(error) => Self::from_cdp(error, json),
        }
    }

    pub(crate) fn from_console_read(error: ConsoleReadError, json: bool) -> Self {
        match error {
            ConsoleReadError::TargetWebSocketMissing => Self::runtime(
                json,
                "target_websocket_missing",
                TARGET_WEBSOCKET_MISSING_MESSAGE,
                TARGET_WEBSOCKET_MISSING_HINT,
            ),
            ConsoleReadError::Cdp(CdpError::WebSocketTransport { .. }) => Self::runtime(
                json,
                "endpoint_unreachable",
                ENDPOINT_UNREACHABLE_MESSAGE,
                ENDPOINT_UNREACHABLE_HINT,
            ),
            ConsoleReadError::Cdp(CdpError::Command { .. } | CdpError::ResponseInvalid { .. }) => {
                Self::runtime(
                    json,
                    "cdp_response_invalid",
                    CDP_RESPONSE_INVALID_MESSAGE,
                    CDP_RESPONSE_INVALID_HINT,
                )
            }
            ConsoleReadError::Cdp(CdpError::WebSocketUrlInvalid { .. }) => Self::runtime(
                json,
                "cdp_unhealthy",
                CDP_UNHEALTHY_MESSAGE,
                CDP_UNHEALTHY_HINT,
            ),
            ConsoleReadError::Cdp(error) => Self::from_cdp(error, json),
        }
    }

    pub(crate) fn from_network_read(error: NetworkReadError, json: bool) -> Self {
        match error {
            NetworkReadError::TargetWebSocketMissing => Self::runtime(
                json,
                "target_websocket_missing",
                TARGET_WEBSOCKET_MISSING_MESSAGE,
                TARGET_WEBSOCKET_MISSING_HINT,
            ),
            NetworkReadError::Cdp(CdpError::WebSocketTransport { .. }) => Self::runtime(
                json,
                "endpoint_unreachable",
                ENDPOINT_UNREACHABLE_MESSAGE,
                ENDPOINT_UNREACHABLE_HINT,
            ),
            NetworkReadError::Cdp(CdpError::Command { .. } | CdpError::ResponseInvalid { .. }) => {
                Self::runtime(
                    json,
                    "cdp_response_invalid",
                    CDP_RESPONSE_INVALID_MESSAGE,
                    CDP_RESPONSE_INVALID_HINT,
                )
            }
            NetworkReadError::Cdp(CdpError::WebSocketUrlInvalid { .. }) => Self::runtime(
                json,
                "cdp_unhealthy",
                CDP_UNHEALTHY_MESSAGE,
                CDP_UNHEALTHY_HINT,
            ),
            NetworkReadError::Cdp(error) => Self::from_cdp(error, json),
        }
    }

    pub(crate) fn from_wait(error: WaitError, json: bool) -> Self {
        match error {
            WaitError::TargetWebSocketMissing => Self::runtime(
                json,
                "target_websocket_missing",
                TARGET_WEBSOCKET_MISSING_MESSAGE,
                TARGET_WEBSOCKET_MISSING_HINT,
            ),
            WaitError::Cdp(CdpError::WebSocketTransport { .. }) => Self::runtime(
                json,
                "endpoint_unreachable",
                ENDPOINT_UNREACHABLE_MESSAGE,
                ENDPOINT_UNREACHABLE_HINT,
            ),
            WaitError::Cdp(CdpError::Command { .. } | CdpError::ResponseInvalid { .. }) => {
                Self::runtime(
                    json,
                    "cdp_response_invalid",
                    CDP_RESPONSE_INVALID_MESSAGE,
                    CDP_RESPONSE_INVALID_HINT,
                )
            }
            WaitError::Cdp(CdpError::WebSocketUrlInvalid { .. }) => Self::runtime(
                json,
                "cdp_unhealthy",
                CDP_UNHEALTHY_MESSAGE,
                CDP_UNHEALTHY_HINT,
            ),
            WaitError::Cdp(error) => Self::from_cdp(error, json),
        }
    }

    pub(crate) fn from_flow(error: FlowError, json: bool) -> Self {
        match error {
            FlowError::TargetWebSocketMissing => Self::runtime(
                json,
                "target_websocket_missing",
                TARGET_WEBSOCKET_MISSING_MESSAGE,
                TARGET_WEBSOCKET_MISSING_HINT,
            ),
            FlowError::Cdp(CdpError::WebSocketTransport { .. }) => Self::runtime(
                json,
                "endpoint_unreachable",
                ENDPOINT_UNREACHABLE_MESSAGE,
                ENDPOINT_UNREACHABLE_HINT,
            ),
            FlowError::NavigationFailed(_) => Self::runtime(
                json,
                "navigation_failed",
                NAVIGATION_FAILED_MESSAGE,
                NAVIGATION_FAILED_HINT,
            ),
            FlowError::Cdp(CdpError::Command { .. }) => Self::runtime(
                json,
                "cdp_response_invalid",
                CDP_RESPONSE_INVALID_MESSAGE,
                CDP_RESPONSE_INVALID_HINT,
            ),
            FlowError::Cdp(CdpError::ResponseInvalid { .. }) => Self::runtime(
                json,
                "cdp_response_invalid",
                CDP_RESPONSE_INVALID_MESSAGE,
                CDP_RESPONSE_INVALID_HINT,
            ),
            FlowError::Cdp(CdpError::WebSocketUrlInvalid { .. }) => Self::runtime(
                json,
                "cdp_unhealthy",
                CDP_UNHEALTHY_MESSAGE,
                CDP_UNHEALTHY_HINT,
            ),
            FlowError::Cdp(error) => Self::from_cdp(error, json),
        }
    }

    pub(crate) fn from_layout_read(error: LayoutReadError, json: bool) -> Self {
        match error {
            LayoutReadError::TargetWebSocketMissing => Self::runtime(
                json,
                "target_websocket_missing",
                TARGET_WEBSOCKET_MISSING_MESSAGE,
                TARGET_WEBSOCKET_MISSING_HINT,
            ),
            LayoutReadError::Cdp(CdpError::Command { .. } | CdpError::ResponseInvalid { .. }) => {
                Self::runtime(
                    json,
                    "cdp_response_invalid",
                    CDP_RESPONSE_INVALID_MESSAGE,
                    CDP_RESPONSE_INVALID_HINT,
                )
            }
            LayoutReadError::Cdp(_) => Self::runtime(
                json,
                "cdp_unhealthy",
                CDP_UNHEALTHY_MESSAGE,
                CDP_UNHEALTHY_HINT,
            ),
        }
    }

    pub(crate) fn from_screenshot(error: ScreenshotError, json: bool) -> Self {
        match error {
            ScreenshotError::TargetWebSocketMissing => Self::runtime(
                json,
                "target_websocket_missing",
                TARGET_WEBSOCKET_MISSING_MESSAGE,
                TARGET_WEBSOCKET_MISSING_HINT,
            ),
            ScreenshotError::Cdp(CdpError::WebSocketTransport { .. }) => Self::runtime(
                json,
                "endpoint_unreachable",
                ENDPOINT_UNREACHABLE_MESSAGE,
                ENDPOINT_UNREACHABLE_HINT,
            ),
            ScreenshotError::Cdp(CdpError::Command { .. } | CdpError::ResponseInvalid { .. }) => {
                Self::runtime(
                    json,
                    "cdp_response_invalid",
                    CDP_RESPONSE_INVALID_MESSAGE,
                    CDP_RESPONSE_INVALID_HINT,
                )
            }
            ScreenshotError::Cdp(CdpError::WebSocketUrlInvalid { .. }) => Self::runtime(
                json,
                "cdp_unhealthy",
                CDP_UNHEALTHY_MESSAGE,
                CDP_UNHEALTHY_HINT,
            ),
            ScreenshotError::Cdp(error) => Self::from_cdp(error, json),
        }
    }

    pub(crate) fn from_interaction(error: InteractionError, json: bool) -> Self {
        match error {
            InteractionError::TargetWebSocketMissing => Self::runtime(
                json,
                "target_websocket_missing",
                TARGET_WEBSOCKET_MISSING_MESSAGE,
                TARGET_WEBSOCKET_MISSING_HINT,
            ),
            InteractionError::Cdp(CdpError::WebSocketTransport { .. }) => Self::runtime(
                json,
                "interaction_transport_failed",
                "Interaction transport failed or its deadline expired.",
                "Earlier input steps may have executed. Inspect browser state before deciding whether another action is needed.",
            ),
            InteractionError::Cdp(CdpError::Command { .. } | CdpError::ResponseInvalid { .. }) => {
                Self::runtime(
                    json,
                    "cdp_response_invalid",
                    CDP_RESPONSE_INVALID_MESSAGE,
                    CDP_RESPONSE_INVALID_HINT,
                )
            }
            InteractionError::Cdp(CdpError::WebSocketUrlInvalid { .. }) => Self::runtime(
                json,
                "cdp_unhealthy",
                CDP_UNHEALTHY_MESSAGE,
                CDP_UNHEALTHY_HINT,
            ),
            InteractionError::Cdp(error) => Self::from_cdp(error, json),
        }
    }

    pub(crate) fn write_stderr(&self) {
        if self.json {
            let error = ErrorOutput {
                schema_version: CLI_SCHEMA_VERSION,
                ok: false,
                error: ErrorBody {
                    code: self.code,
                    message: self.message,
                    hint: self.hint,
                },
            };
            eprintln!(
                "{}",
                serde_json::to_string(&error).expect("static error output should serialize")
            );
        } else {
            eprintln!("error[{}]: {}", self.code, self.message);
            eprintln!("hint: {}", self.hint);
        }
    }
}

#[derive(Debug, Serialize)]
struct ErrorOutput {
    schema_version: u8,
    ok: bool,
    error: ErrorBody,
}

#[derive(Debug, Serialize)]
struct ErrorBody {
    code: &'static str,
    message: &'static str,
    hint: &'static str,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_errors_use_contract_codes() {
        let missing = CliError::from_endpoint(EndpointResolutionError::Missing, true);

        assert_eq!(missing.exit_code, 2);
        assert_eq!(missing.code, "endpoint_missing");
        assert!(missing.json);
    }
}
