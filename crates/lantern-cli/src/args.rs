use crate::error::{
    BROWSER_GPU_DEVICE_INVALID_HINT, BROWSER_GRAPHICS_INVALID_HINT,
    BROWSER_GRAPHICS_INVALID_MESSAGE, BROWSER_HOST_GATEWAY_INVALID_HINT,
    BROWSER_PROFILE_MISSING_HINT, BROWSER_PROFILE_MISSING_MESSAGE, BROWSER_RUNTIME_INVALID_HINT,
    BROWSER_RUNTIME_INVALID_MESSAGE, BROWSER_USAGE_HINT, CliError, DOM_LIMIT_INVALID_HINT,
    DOM_LIMIT_INVALID_MESSAGE, INTERACTION_DELTA_INVALID_HINT, INTERACTION_DELTA_INVALID_MESSAGE,
    INTERACTION_DURATION_INVALID_HINT, INTERACTION_DURATION_INVALID_MESSAGE,
    SCREENSHOT_REGION_INVALID_HINT, SCREENSHOT_REGION_INVALID_MESSAGE, WAIT_TIMEOUT_INVALID_HINT,
    WAIT_TIMEOUT_INVALID_MESSAGE,
};
use crate::registry::{BrowserCommand, BrowserProfileCommand, Command};
use lantern_core::wait::WaitConditionName;
use lantern_storage::{BrowserGraphicsMode, RuntimeKind};
use std::path::PathBuf;

pub(crate) fn print_help() {
    println!(
        "Usage: lantern <doctor|targets|page|dom|open|wait|console|network|layout|screenshot|click|type|key|hover|wheel|drag|flow|action-flow> [--endpoint <URL>] [--json] [--no-redact] [--target-id <ID>]
       lantern capabilities [--json]
       lantern browser <start|list|status|endpoint|stop|prune> [--json]
       lantern browser profile <create|list|status|delete> [NAME] [--yes] [--json]
       lantern open <URL> [--endpoint <URL>] [--json] [--no-redact] [--target-id <ID>]
       lantern wait <ready|url|selector|text|quiet> --timeout-ms <MS> [condition flags]
       lantern action-flow --selector <CSS> --timeout-ms <MS> (--expect-selector <CSS> [--expect-text <TEXT>] | --expect-url <URL>) [--output <PATH>] [--strict]
       lantern flow --timeout-ms <MS> [--quiet-ms <MS>] [--open <URL>]
       lantern screenshot --output <PATH> [--overwrite]
       lantern click --selector <CSS> --timeout-ms <MS>
       lantern type --selector <CSS> (--text <TEXT> | --text-file <PATH>) --timeout-ms <MS>
       lantern key --selector <CSS> --key <KEY> --timeout-ms <MS>
       lantern hover --selector <CSS> --timeout-ms <MS>
       lantern wheel --selector <CSS> [--dx <PX>] [--dy <PX>] --timeout-ms <MS>
       lantern drag --selector <CSS> --dx <PX> --dy <PX> --duration-ms <MS> --timeout-ms <MS>

Shared flags:
  --endpoint <URL>  Local Chromium CDP HTTP endpoint
  --json            Emit JSON results on stdout; pre-result errors on stderr
  --no-redact       Disable redaction for command output metadata
  --strict          Interactions: require acknowledged input; action-flow: require passed verdict
  --target-id <ID>  Exact CDP page target id for selected-page commands

Navigation and wait flags:
  --open <URL>      Optional navigation URL for flow
  --timeout-ms <MS> Explicit timeout from 1 through 30000
  --state <STATE>   ready state: loading, interactive, or complete
  --url-shape <URL> Expected URL shape for wait url
  --selector <CSS>  CSS selector for wait selector/text and interactions
  --text <TEXT>     Text substring for wait text, or inserted text for type
  --text-file <PATH>
                     Owner-private UTF-8 input for type; never reported
  --key <KEY>       Single key value for key
  --dx <PX>         Horizontal wheel or drag delta
  --dy <PX>         Vertical wheel or drag delta
  --duration-ms <MS> Drag duration from 0 through 30000
  --quiet-ms <MS>   Quiet period for wait quiet or flow

Screenshot flags:
  --output <PATH>   Local PNG path: required for screenshot, optional for action-flow
  --overwrite       Replace an existing output file
  --region-x <PX>   Screenshot crop x coordinate; --crop-x is an alias
  --region-y <PX>   Screenshot crop y coordinate; --crop-y is an alias
  --region-width <PX> Screenshot crop width; --crop-width is an alias
  --region-height <PX> Screenshot crop height; --crop-height is an alias

Browser lifecycle flags:
  --runtime <NAME>  Container runtime for browser start: podman or docker
  --image <IMAGE>   Container image for browser start
  --id <ID>         Optional disposable browser id; persistent ids are derived
  --wait-ms <MS>    Browser start readiness timeout
  --host-gateway <HOST>
                     Map one DNS hostname to the runtime host gateway
  --profile <NAME>  Existing persistent profile selected only for browser start
  --yes             Confirm explicit browser profile deletion
  --graphics <MODE> Browser graphics mode: disabled, swiftshader, gpu, or webgpu
  --gpu-device <DEV> Explicit runtime device for gpu/webgpu; required by webgpu

  capabilities       Print package/build identity, commands and output schema versions as JSON

  -h, --help        Print help
  -V, --version     Print version"
    );
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Invocation {
    pub(crate) command: Option<Command>,
    pub(crate) endpoint: Option<String>,
    pub(crate) json: bool,
    pub(crate) no_redact: bool,
    pub(crate) strict: bool,
    pub(crate) expect_selector: Option<String>,
    pub(crate) expect_text: Option<String>,
    pub(crate) expect_url: Option<String>,
    pub(crate) target_id: Option<String>,
    pub(crate) open_url: Option<String>,
    pub(crate) wait_kind: Option<WaitConditionName>,
    pub(crate) timeout_ms: Option<u64>,
    pub(crate) wait_state: Option<String>,
    pub(crate) wait_url_shape: Option<String>,
    pub(crate) wait_selector: Option<String>,
    pub(crate) wait_text: Option<String>,
    pub(crate) type_text_file: Option<PathBuf>,
    pub(crate) key: Option<String>,
    pub(crate) quiet_ms: Option<u64>,
    pub(crate) screenshot_output: Option<String>,
    pub(crate) screenshot_overwrite: bool,
    pub(crate) region_x: Option<f64>,
    pub(crate) region_y: Option<f64>,
    pub(crate) region_width: Option<f64>,
    pub(crate) region_height: Option<f64>,
    pub(crate) dom_depth: Option<usize>,
    pub(crate) dom_max_nodes: Option<usize>,
    pub(crate) delta_x: Option<f64>,
    pub(crate) delta_y: Option<f64>,
    pub(crate) duration_ms: Option<u64>,
    pub(crate) browser_command: Option<BrowserCommand>,
    pub(crate) browser_id: Option<String>,
    pub(crate) browser_runtime: Option<RuntimeKind>,
    pub(crate) browser_image: Option<String>,
    pub(crate) browser_wait_ms: Option<u64>,
    pub(crate) browser_host_gateway: Option<String>,
    pub(crate) browser_profile_command: Option<BrowserProfileCommand>,
    pub(crate) browser_profile_name: Option<String>,
    pub(crate) browser_confirm: bool,
    pub(crate) browser_graphics: Option<BrowserGraphicsMode>,
    pub(crate) browser_gpu_device: Option<String>,
    pub(crate) help: bool,
    pub(crate) version: bool,
}

impl Invocation {
    pub(crate) fn has_wait_only_flags(&self) -> bool {
        self.wait_kind.is_some()
            || self.wait_state.is_some()
            || self.wait_url_shape.is_some()
            || self.quiet_ms.is_some()
    }

    pub(crate) fn has_screenshot_flags(&self) -> bool {
        self.screenshot_output.is_some()
            || self.screenshot_overwrite
            || self.region_x.is_some()
            || self.region_y.is_some()
            || self.region_width.is_some()
            || self.region_height.is_some()
    }

    pub(crate) fn has_dom_flags(&self) -> bool {
        self.dom_depth.is_some() || self.dom_max_nodes.is_some()
    }

    pub(crate) fn has_browser_lifecycle_flags(&self) -> bool {
        self.browser_id.is_some()
            || self.browser_runtime.is_some()
            || self.browser_image.is_some()
            || self.browser_wait_ms.is_some()
            || self.browser_host_gateway.is_some()
            || self.browser_profile_command.is_some()
            || self.browser_profile_name.is_some()
            || self.browser_confirm
            || self.browser_graphics.is_some()
            || self.browser_gpu_device.is_some()
    }

    pub(crate) fn parse(args: impl IntoIterator<Item = String>) -> Result<Self, CliError> {
        let mut invocation = Self {
            command: None,
            endpoint: None,
            json: false,
            no_redact: false,
            strict: false,
            expect_selector: None,
            expect_text: None,
            expect_url: None,
            target_id: None,
            open_url: None,
            wait_kind: None,
            timeout_ms: None,
            wait_state: None,
            wait_url_shape: None,
            wait_selector: None,
            wait_text: None,
            type_text_file: None,
            key: None,
            quiet_ms: None,
            screenshot_output: None,
            screenshot_overwrite: false,
            region_x: None,
            region_y: None,
            region_width: None,
            region_height: None,
            dom_depth: None,
            dom_max_nodes: None,
            delta_x: None,
            delta_y: None,
            duration_ms: None,
            browser_command: None,
            browser_id: None,
            browser_runtime: None,
            browser_image: None,
            browser_wait_ms: None,
            browser_host_gateway: None,
            browser_profile_command: None,
            browser_profile_name: None,
            browser_confirm: false,
            browser_graphics: None,
            browser_gpu_device: None,
            help: false,
            version: false,
        };

        let mut args = args.into_iter();
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "-h" | "--help" => invocation.help = true,
                "-V" | "--version" => invocation.version = true,
                "--json" => invocation.json = true,
                "--no-redact" => invocation.no_redact = true,
                "--strict" => invocation.strict = true,
                flag @ ("--expect-selector" | "--expect-text" | "--expect-url") => {
                    let value = args.next().ok_or_else(|| {
                        CliError::usage(
                            invocation.json,
                            "Missing postcondition flag value.",
                            "Pass a value after --expect-selector, --expect-text or --expect-url.",
                        )
                    })?;
                    match flag {
                        "--expect-selector" => invocation.expect_selector = Some(value),
                        "--expect-text" => invocation.expect_text = Some(value),
                        _ => invocation.expect_url = Some(value),
                    }
                }
                "--endpoint" => {
                    let Some(endpoint) = args.next() else {
                        return Err(CliError::usage(
                            invocation.json,
                            "Missing value for --endpoint.",
                            "Pass --endpoint http://127.0.0.1:9222.",
                        ));
                    };
                    invocation.endpoint = Some(endpoint);
                }
                "--target-id" => {
                    let Some(target_id) = args.next() else {
                        return Err(CliError::usage(
                            invocation.json,
                            "Missing value for --target-id.",
                            "Pass --target-id with the exact id from lantern targets.",
                        ));
                    };
                    invocation.target_id = Some(target_id);
                }
                "--timeout-ms" => {
                    let Some(timeout_ms) = args.next() else {
                        return Err(CliError::usage(
                            invocation.json,
                            "Missing value for --timeout-ms.",
                            "Pass --timeout-ms from 1 through 30000.",
                        ));
                    };
                    invocation.timeout_ms = Some(parse_wait_millis(&timeout_ms, invocation.json)?);
                }
                "--state" => {
                    let Some(state) = args.next() else {
                        return Err(CliError::usage(
                            invocation.json,
                            "Missing value for --state.",
                            "Use --state loading, --state interactive, or --state complete.",
                        ));
                    };
                    invocation.wait_state = Some(state);
                }
                "--url-shape" => {
                    let Some(url_shape) = args.next() else {
                        return Err(CliError::usage(
                            invocation.json,
                            "Missing value for --url-shape.",
                            "Pass the URL shape to wait for.",
                        ));
                    };
                    invocation.wait_url_shape = Some(url_shape);
                }
                "--selector" => {
                    let Some(selector) = args.next() else {
                        return Err(CliError::usage(
                            invocation.json,
                            "Missing value for --selector.",
                            "Pass a CSS selector.",
                        ));
                    };
                    invocation.wait_selector = Some(selector);
                }
                "--text" => {
                    let Some(text) = args.next() else {
                        return Err(CliError::usage(
                            invocation.json,
                            "Missing value for --text.",
                            "Pass text to wait for.",
                        ));
                    };
                    invocation.wait_text = Some(text);
                }
                "--text-file" => {
                    let Some(path) = args.next() else {
                        return Err(CliError::usage(
                            invocation.json,
                            "Missing value for --text-file.",
                            "Pass the owner-private UTF-8 file path.",
                        ));
                    };
                    invocation.type_text_file = Some(PathBuf::from(path));
                }
                "--key" => {
                    let Some(key) = args.next() else {
                        return Err(CliError::usage(
                            invocation.json,
                            "Missing value for --key.",
                            "Pass a single key value.",
                        ));
                    };
                    invocation.key = Some(key);
                }
                "--dx" | "--delta-x" => {
                    let Some(delta) = args.next() else {
                        return Err(CliError::usage(
                            invocation.json,
                            "Missing value for --dx.",
                            INTERACTION_DELTA_INVALID_HINT,
                        ));
                    };
                    invocation.delta_x = Some(parse_finite_number(
                        &delta,
                        invocation.json,
                        "interaction_delta_invalid",
                        INTERACTION_DELTA_INVALID_MESSAGE,
                        INTERACTION_DELTA_INVALID_HINT,
                    )?);
                }
                "--dy" | "--delta-y" => {
                    let Some(delta) = args.next() else {
                        return Err(CliError::usage(
                            invocation.json,
                            "Missing value for --dy.",
                            INTERACTION_DELTA_INVALID_HINT,
                        ));
                    };
                    invocation.delta_y = Some(parse_finite_number(
                        &delta,
                        invocation.json,
                        "interaction_delta_invalid",
                        INTERACTION_DELTA_INVALID_MESSAGE,
                        INTERACTION_DELTA_INVALID_HINT,
                    )?);
                }
                "--duration-ms" => {
                    let Some(duration_ms) = args.next() else {
                        return Err(CliError::usage(
                            invocation.json,
                            "Missing value for --duration-ms.",
                            INTERACTION_DURATION_INVALID_HINT,
                        ));
                    };
                    invocation.duration_ms =
                        Some(parse_duration_millis(&duration_ms, invocation.json)?);
                }
                "--open" => {
                    let Some(url) = args.next() else {
                        return Err(CliError::usage(
                            invocation.json,
                            "Missing value for --open.",
                            "Pass an absolute http:// or https:// URL, or about:blank.",
                        ));
                    };
                    invocation.open_url = Some(url);
                }
                "--quiet-ms" => {
                    let Some(quiet_ms) = args.next() else {
                        return Err(CliError::usage(
                            invocation.json,
                            "Missing value for --quiet-ms.",
                            "Pass --quiet-ms from 1 through 30000.",
                        ));
                    };
                    invocation.quiet_ms = Some(parse_wait_millis(&quiet_ms, invocation.json)?);
                }
                "--depth" => {
                    let Some(depth) = args.next() else {
                        return Err(CliError::usage(
                            invocation.json,
                            "Missing value for --depth.",
                            DOM_LIMIT_INVALID_HINT,
                        ));
                    };
                    invocation.dom_depth = Some(parse_dom_limit(&depth, invocation.json)?);
                }
                "--max-nodes" => {
                    let Some(max_nodes) = args.next() else {
                        return Err(CliError::usage(
                            invocation.json,
                            "Missing value for --max-nodes.",
                            DOM_LIMIT_INVALID_HINT,
                        ));
                    };
                    invocation.dom_max_nodes = Some(parse_dom_limit(&max_nodes, invocation.json)?);
                }
                "--output" => {
                    let Some(output) = args.next() else {
                        return Err(CliError::usage(
                            invocation.json,
                            "Missing value for --output.",
                            "Pass a local screenshot output path.",
                        ));
                    };
                    invocation.screenshot_output = Some(output);
                }
                "--region-x" | "--crop-x" => {
                    let Some(value) = args.next() else {
                        return Err(CliError::usage(
                            invocation.json,
                            "Missing value for --region-x.",
                            SCREENSHOT_REGION_INVALID_HINT,
                        ));
                    };
                    invocation.region_x = Some(parse_finite_number(
                        &value,
                        invocation.json,
                        "screenshot_region_invalid",
                        SCREENSHOT_REGION_INVALID_MESSAGE,
                        SCREENSHOT_REGION_INVALID_HINT,
                    )?);
                }
                "--region-y" | "--crop-y" => {
                    let Some(value) = args.next() else {
                        return Err(CliError::usage(
                            invocation.json,
                            "Missing value for --region-y.",
                            SCREENSHOT_REGION_INVALID_HINT,
                        ));
                    };
                    invocation.region_y = Some(parse_finite_number(
                        &value,
                        invocation.json,
                        "screenshot_region_invalid",
                        SCREENSHOT_REGION_INVALID_MESSAGE,
                        SCREENSHOT_REGION_INVALID_HINT,
                    )?);
                }
                "--region-width" | "--crop-width" => {
                    let Some(value) = args.next() else {
                        return Err(CliError::usage(
                            invocation.json,
                            "Missing value for --region-width.",
                            SCREENSHOT_REGION_INVALID_HINT,
                        ));
                    };
                    invocation.region_width = Some(parse_finite_number(
                        &value,
                        invocation.json,
                        "screenshot_region_invalid",
                        SCREENSHOT_REGION_INVALID_MESSAGE,
                        SCREENSHOT_REGION_INVALID_HINT,
                    )?);
                }
                "--region-height" | "--crop-height" => {
                    let Some(value) = args.next() else {
                        return Err(CliError::usage(
                            invocation.json,
                            "Missing value for --region-height.",
                            SCREENSHOT_REGION_INVALID_HINT,
                        ));
                    };
                    invocation.region_height = Some(parse_finite_number(
                        &value,
                        invocation.json,
                        "screenshot_region_invalid",
                        SCREENSHOT_REGION_INVALID_MESSAGE,
                        SCREENSHOT_REGION_INVALID_HINT,
                    )?);
                }
                "--overwrite" => invocation.screenshot_overwrite = true,
                "--runtime" => {
                    let Some(runtime) = args.next() else {
                        return Err(CliError::usage(
                            invocation.json,
                            "Missing value for --runtime.",
                            BROWSER_RUNTIME_INVALID_HINT,
                        ));
                    };
                    invocation.browser_runtime =
                        Some(RuntimeKind::parse(&runtime).ok_or_else(|| {
                            CliError::usage(
                                invocation.json,
                                BROWSER_RUNTIME_INVALID_MESSAGE,
                                BROWSER_RUNTIME_INVALID_HINT,
                            )
                        })?);
                }
                "--image" => {
                    let Some(image) = args.next() else {
                        return Err(CliError::usage(
                            invocation.json,
                            "Missing value for --image.",
                            "Pass a container image tag.",
                        ));
                    };
                    invocation.browser_image = Some(image);
                }
                "--id" => {
                    let Some(id) = args.next() else {
                        return Err(CliError::usage(
                            invocation.json,
                            "Missing value for --id.",
                            "Pass a browser instance id.",
                        ));
                    };
                    invocation.browser_id = Some(id);
                }
                "--wait-ms" => {
                    let Some(wait_ms) = args.next() else {
                        return Err(CliError::usage(
                            invocation.json,
                            "Missing value for --wait-ms.",
                            "Pass a readiness timeout in milliseconds.",
                        ));
                    };
                    invocation.browser_wait_ms =
                        Some(parse_wait_millis(&wait_ms, invocation.json)?);
                }
                "--host-gateway" => {
                    let Some(hostname) = args.next() else {
                        return Err(CliError::usage(
                            invocation.json,
                            "Missing value for --host-gateway.",
                            BROWSER_HOST_GATEWAY_INVALID_HINT,
                        ));
                    };
                    invocation.browser_host_gateway = Some(hostname.to_ascii_lowercase());
                }
                "--profile" => {
                    let Some(name) = args.next() else {
                        return Err(CliError::usage(
                            invocation.json,
                            BROWSER_PROFILE_MISSING_MESSAGE,
                            BROWSER_PROFILE_MISSING_HINT,
                        ));
                    };
                    invocation.browser_profile_name = Some(name);
                }
                "--yes" => invocation.browser_confirm = true,
                "--graphics" => {
                    let Some(graphics) = args.next() else {
                        return Err(CliError::usage(
                            invocation.json,
                            "Missing value for --graphics.",
                            BROWSER_GRAPHICS_INVALID_HINT,
                        ));
                    };
                    invocation.browser_graphics =
                        Some(BrowserGraphicsMode::parse(&graphics).ok_or_else(|| {
                            CliError::usage(
                                invocation.json,
                                BROWSER_GRAPHICS_INVALID_MESSAGE,
                                BROWSER_GRAPHICS_INVALID_HINT,
                            )
                        })?);
                }
                "--gpu-device" => {
                    let Some(device) = args.next() else {
                        return Err(CliError::usage(
                            invocation.json,
                            "Missing value for --gpu-device.",
                            BROWSER_GPU_DEVICE_INVALID_HINT,
                        ));
                    };
                    invocation.browser_gpu_device = Some(device);
                }
                name if Command::from_name(name).is_some() => {
                    if invocation.command.is_some() {
                        return Err(CliError::usage(
                            invocation.json,
                            "Multiple commands provided.",
                            "Run one command at a time.",
                        ));
                    }
                    invocation.command = Command::from_name(name);
                }
                unknown if unknown.starts_with('-') => {
                    return Err(CliError::usage(
                        invocation.json,
                        "Unknown flag.",
                        "Run lantern --help for supported flags.",
                    ));
                }
                _ if invocation.command == Some(Command::Open) && invocation.open_url.is_none() => {
                    invocation.open_url = Some(arg);
                }
                _ if invocation.command == Some(Command::Open) => {
                    return Err(CliError::usage(
                        invocation.json,
                        "Multiple URLs provided for open.",
                        "Run lantern open with exactly one URL.",
                    ));
                }
                _ if invocation.command == Some(Command::Wait)
                    && invocation.wait_kind.is_none() =>
                {
                    invocation.wait_kind = Some(parse_wait_condition(&arg, invocation.json)?);
                }
                _ if invocation.command == Some(Command::Wait) => {
                    return Err(CliError::usage(
                        invocation.json,
                        "Multiple wait conditions provided.",
                        "Run lantern wait with exactly one condition.",
                    ));
                }
                _ if invocation.command == Some(Command::Browser)
                    && invocation.browser_command.is_none() =>
                {
                    invocation.browser_command =
                        Some(parse_browser_command(&arg, invocation.json)?);
                }
                _ if invocation.command == Some(Command::Browser)
                    && invocation.browser_command == Some(BrowserCommand::Profile)
                    && invocation.browser_profile_command.is_none() =>
                {
                    invocation.browser_profile_command =
                        Some(parse_browser_profile_command(&arg, invocation.json)?);
                }
                _ if invocation.command == Some(Command::Browser)
                    && invocation.browser_command == Some(BrowserCommand::Profile)
                    && invocation.browser_profile_name.is_none() =>
                {
                    invocation.browser_profile_name = Some(arg);
                }
                _ if invocation.command == Some(Command::Browser)
                    && invocation.browser_id.is_none() =>
                {
                    invocation.browser_id = Some(arg);
                }
                _ if invocation.command == Some(Command::Browser) => {
                    return Err(CliError::usage(
                        invocation.json,
                        "Multiple browser instance ids provided.",
                        "Pass at most one id to lantern browser status, endpoint, or stop.",
                    ));
                }
                _ => {
                    return Err(CliError::usage(
                        invocation.json,
                        "Unknown command.",
                        "Run lantern --help for supported commands.",
                    ));
                }
            }
        }

        Ok(invocation)
    }
}

fn parse_dom_limit(value: &str, json: bool) -> Result<usize, CliError> {
    value.parse::<usize>().map_err(|_| CliError {
        exit_code: 2,
        json,
        code: "dom_limit_invalid",
        message: DOM_LIMIT_INVALID_MESSAGE,
        hint: DOM_LIMIT_INVALID_HINT,
    })
}

fn parse_wait_millis(value: &str, json: bool) -> Result<u64, CliError> {
    value.parse::<u64>().map_err(|_| CliError {
        exit_code: 2,
        json,
        code: "wait_timeout_invalid",
        message: WAIT_TIMEOUT_INVALID_MESSAGE,
        hint: WAIT_TIMEOUT_INVALID_HINT,
    })
}

fn parse_duration_millis(value: &str, json: bool) -> Result<u64, CliError> {
    value.parse::<u64>().map_err(|_| CliError {
        exit_code: 2,
        json,
        code: "interaction_duration_invalid",
        message: INTERACTION_DURATION_INVALID_MESSAGE,
        hint: INTERACTION_DURATION_INVALID_HINT,
    })
}

fn parse_finite_number(
    value: &str,
    json: bool,
    code: &'static str,
    message: &'static str,
    hint: &'static str,
) -> Result<f64, CliError> {
    let value = value.parse::<f64>().map_err(|_| CliError {
        exit_code: 2,
        json,
        code,
        message,
        hint,
    })?;
    if value.is_finite() {
        Ok(value)
    } else {
        Err(CliError {
            exit_code: 2,
            json,
            code,
            message,
            hint,
        })
    }
}

fn parse_browser_command(value: &str, json: bool) -> Result<BrowserCommand, CliError> {
    BrowserCommand::from_name(value)
        .ok_or_else(|| CliError::usage(json, "Unknown browser subcommand.", BROWSER_USAGE_HINT))
}

fn parse_browser_profile_command(
    value: &str,
    json: bool,
) -> Result<BrowserProfileCommand, CliError> {
    BrowserProfileCommand::from_name(value).ok_or_else(|| {
        CliError::usage(
            json,
            "Unknown browser profile subcommand.",
            "Run lantern browser profile <create|list|status|delete>.",
        )
    })
}

fn parse_wait_condition(value: &str, json: bool) -> Result<WaitConditionName, CliError> {
    crate::registry::wait_condition_from_name(value).ok_or_else(|| {
        CliError::usage(
            json,
            "Unknown wait condition.",
            "Use ready, url, selector, text, or quiet.",
        )
    })
}

#[cfg(test)]
mod tests;
