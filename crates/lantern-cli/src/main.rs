use std::{env, fs, io::ErrorKind, path::Path, process::ExitCode, time::Duration};

use lantern_core::{
    cdp::{BrowserVersion, CdpClient, CdpError, TargetInfo},
    console::{ConsoleCommandOutput, ConsoleEntry, ConsoleReadError, read_console_errors},
    dom::{
        DOM_DEFAULT_DEPTH, DOM_DEFAULT_MAX_NODES, DomCommandOutput, DomNodeSummary, DomReadError,
        DomSummaryOptions, read_dom_summary,
    },
    endpoint::{EndpointResolutionError, ResolvedEndpoint, resolve_endpoint},
    interaction::{
        INTERACTION_MAX_TIMEOUT_MS, InteractionCommandOutput, InteractionError, click_element,
        type_text,
    },
    navigation::{
        NavigationCommandOutput, NavigationError, navigate_page, validate_navigation_url,
    },
    network::{NetworkCommandOutput, NetworkEntry, NetworkReadError, read_network_failures},
    redaction::{RedactionMode, sanitize_title, sanitize_url},
    screenshot::{
        SCREENSHOT_REDACTION_CAVEAT, ScreenshotCommandOutput, ScreenshotError, ScreenshotSummary,
        capture_visible_viewport_screenshot,
    },
    wait::{
        ReadyState, WAIT_MAX_TIMEOUT_MS, WaitCommandOutput, WaitCondition, WaitConditionName,
        WaitError, wait_for_condition,
    },
};
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
const CDP_RESPONSE_INVALID_MESSAGE: &str = "CDP endpoint returned an invalid response.";
const CDP_RESPONSE_INVALID_HINT: &str =
    "Retry against a fresh Chromium DevTools endpoint or inspect /json/version and /json/list.";
const TARGET_NOT_FOUND_MESSAGE: &str = "No page target was available.";
const TARGET_NOT_FOUND_HINT: &str = "Open a page in the attached Chromium instance and retry.";
const TARGET_ID_NOT_FOUND_MESSAGE: &str = "No page target matched the requested target id.";
const TARGET_ID_NOT_FOUND_HINT: &str =
    "Run lantern targets and pass the exact id of a page target.";
const TARGET_AMBIGUOUS_MESSAGE: &str = "Multiple page targets matched.";
const TARGET_AMBIGUOUS_HINT: &str =
    "Close extra pages, leave exactly one attached page target, or pass --target-id.";
const TARGET_WEBSOCKET_MISSING_MESSAGE: &str =
    "Selected page target did not expose a WebSocket debugger URL.";
const TARGET_WEBSOCKET_MISSING_HINT: &str = "Refresh the target list, open a normal page target, or restart Chromium with remote debugging enabled.";
const URL_INVALID_MESSAGE: &str = "Invalid navigation URL.";
const URL_INVALID_HINT: &str =
    "Pass an absolute http:// or https:// URL, or the exact URL about:blank.";
const NAVIGATION_FAILED_MESSAGE: &str = "Chromium failed the requested page navigation.";
const NAVIGATION_FAILED_HINT: &str = "Check that the URL is reachable from Chromium, then retry.";
const WAIT_TIMEOUT_INVALID_MESSAGE: &str = "Invalid wait timeout.";
const WAIT_TIMEOUT_INVALID_HINT: &str = "Pass --timeout-ms from 1 through 30000; for quiet waits, --quiet-ms must also be in range and no larger than --timeout-ms.";
const INTERACTION_TIMEOUT_INVALID_MESSAGE: &str = "Invalid interaction timeout.";
const INTERACTION_TIMEOUT_INVALID_HINT: &str = "Pass --timeout-ms from 1 through 30000.";
const SCREENSHOT_OUTPUT_EXISTS_MESSAGE: &str = "Screenshot output path already exists.";
const SCREENSHOT_OUTPUT_EXISTS_HINT: &str =
    "Pass --overwrite to replace the file, or choose a different --output path.";
const SCREENSHOT_WRITE_FAILED_MESSAGE: &str = "Screenshot could not be written.";
const SCREENSHOT_WRITE_FAILED_HINT: &str =
    "Check that the parent directory exists and the output path is writable.";
const DOM_LIMIT_INVALID_MESSAGE: &str = "Invalid DOM summary limit.";
const DOM_LIMIT_INVALID_HINT: &str =
    "Pass --depth from 1 through 12 and --max-nodes from 1 through 500.";

fn main() -> ExitCode {
    match run(env::args().skip(1), env::var("LANTERN_CDP_ENDPOINT").ok()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            error.write_stderr();
            ExitCode::from(error.exit_code)
        }
    }
}

fn run(
    args: impl IntoIterator<Item = String>,
    env_endpoint: Option<String>,
) -> Result<(), CliError> {
    let invocation = Invocation::parse(args)?;

    if invocation.help {
        print_help();
        return Ok(());
    }

    if invocation.version {
        println!("lantern {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    let command = invocation.command.ok_or_else(|| {
        CliError::usage(
            invocation.json,
            "No command provided.",
            "Run lantern --help for usage.",
        )
    })?;

    if command == Command::Open && invocation.open_url.is_none() {
        return Err(CliError::usage(
            invocation.json,
            "Missing URL for open.",
            "Run lantern open <URL> with an absolute http:// or https:// URL, or about:blank.",
        ));
    }

    if invocation.target_id.is_some()
        && !matches!(
            command,
            Command::Page
                | Command::Dom
                | Command::Open
                | Command::Wait
                | Command::Console
                | Command::Network
                | Command::Screenshot
                | Command::Click
                | Command::Type
        )
    {
        return Err(CliError::usage(
            invocation.json,
            "--target-id is only supported by page, dom, open, wait, console, network, screenshot, click, and type.",
            "Run a selected-page command with --target-id <CDP_TARGET_ID>.",
        ));
    }

    if command != Command::Wait && invocation.has_wait_only_flags() {
        return Err(CliError::usage(
            invocation.json,
            "Wait flags are only supported by wait.",
            "Run lantern wait <ready|url|selector|text|quiet> with wait-specific flags.",
        ));
    }

    if !matches!(command, Command::Wait | Command::Click | Command::Type)
        && invocation.timeout_ms.is_some()
    {
        return Err(CliError::usage(
            invocation.json,
            "--timeout-ms is only supported by wait, click, and type.",
            "Run a bounded command with --timeout-ms <MS>.",
        ));
    }

    if !matches!(command, Command::Wait | Command::Click | Command::Type)
        && invocation.wait_selector.is_some()
    {
        return Err(CliError::usage(
            invocation.json,
            "--selector is only supported by wait, click, and type.",
            "Run a selected-element command with --selector <CSS_SELECTOR>.",
        ));
    }

    if !matches!(command, Command::Wait | Command::Type) && invocation.wait_text.is_some() {
        return Err(CliError::usage(
            invocation.json,
            "--text is only supported by wait text and type.",
            "Run lantern wait text or lantern type with --text <TEXT>.",
        ));
    }

    if command == Command::Wait && invocation.wait_kind.is_none() {
        return Err(CliError::usage(
            invocation.json,
            "Missing wait condition.",
            "Run lantern wait <ready|url|selector|text|quiet> with --timeout-ms.",
        ));
    }

    if command == Command::Wait {
        validate_wait_flag_shape(&invocation)?;
    }

    if command != Command::Dom && invocation.has_dom_flags() {
        return Err(CliError::usage(
            invocation.json,
            "DOM limit flags are only supported by dom.",
            "Run lantern dom with --depth <N> or --max-nodes <N>.",
        ));
    }

    if command != Command::Screenshot && invocation.has_screenshot_flags() {
        return Err(CliError::usage(
            invocation.json,
            "Screenshot flags are only supported by screenshot.",
            "Run lantern screenshot --output <PATH>.",
        ));
    }

    if command == Command::Screenshot && invocation.screenshot_output.is_none() {
        return Err(CliError::usage(
            invocation.json,
            "Missing --output for screenshot.",
            "Run lantern screenshot --output <PATH>.",
        ));
    }

    if matches!(command, Command::Click | Command::Type) && invocation.wait_selector.is_none() {
        return Err(CliError::usage(
            invocation.json,
            "Missing --selector for interaction.",
            "Run lantern click --selector <CSS_SELECTOR> --timeout-ms <MS> or lantern type --selector <CSS_SELECTOR> --text <TEXT> --timeout-ms <MS>.",
        ));
    }

    if matches!(command, Command::Click | Command::Type) && invocation.timeout_ms.is_none() {
        return Err(CliError::usage(
            invocation.json,
            "Missing --timeout-ms for interaction.",
            "Run lantern click or lantern type with --timeout-ms <MS> from 1 through 30000.",
        ));
    }

    if command == Command::Type && invocation.wait_text.is_none() {
        return Err(CliError::usage(
            invocation.json,
            "Missing --text for type.",
            "Run lantern type --selector <CSS_SELECTOR> --text <TEXT> --timeout-ms <MS>.",
        ));
    }

    if command == Command::Click && invocation.wait_text.is_some() {
        return Err(CliError::usage(
            invocation.json,
            "--text is not supported by click.",
            "Run lantern click with --selector <CSS_SELECTOR> --timeout-ms <MS>.",
        ));
    }

    if command == Command::Dom {
        build_dom_summary_options(
            invocation.dom_depth,
            invocation.dom_max_nodes,
            invocation.json,
        )?;
    }

    let endpoint = resolve_endpoint(invocation.endpoint.as_deref(), env_endpoint.as_deref())
        .map_err(|error| CliError::from_endpoint(error, invocation.json))?;

    run_command(
        command,
        endpoint,
        invocation.json,
        invocation.no_redact,
        invocation.target_id,
        invocation.open_url,
        invocation.wait_kind,
        invocation.timeout_ms,
        invocation.wait_state,
        invocation.wait_url_shape,
        invocation.wait_selector,
        invocation.wait_text,
        invocation.quiet_ms,
        invocation.screenshot_output,
        invocation.screenshot_overwrite,
        invocation.dom_depth,
        invocation.dom_max_nodes,
    )
}

fn validate_wait_flag_shape(invocation: &Invocation) -> Result<(), CliError> {
    let wait_kind = invocation
        .wait_kind
        .expect("wait kind checked before wait flag shape");
    let unsupported = match wait_kind {
        WaitConditionName::Ready => {
            invocation.wait_url_shape.is_some()
                || invocation.wait_selector.is_some()
                || invocation.wait_text.is_some()
                || invocation.quiet_ms.is_some()
        }
        WaitConditionName::Url => {
            invocation.wait_state.is_some()
                || invocation.wait_selector.is_some()
                || invocation.wait_text.is_some()
                || invocation.quiet_ms.is_some()
        }
        WaitConditionName::Selector => {
            invocation.wait_state.is_some()
                || invocation.wait_url_shape.is_some()
                || invocation.wait_text.is_some()
                || invocation.quiet_ms.is_some()
        }
        WaitConditionName::Text => {
            invocation.wait_state.is_some()
                || invocation.wait_url_shape.is_some()
                || invocation.quiet_ms.is_some()
        }
        WaitConditionName::Quiet => {
            invocation.wait_state.is_some()
                || invocation.wait_url_shape.is_some()
                || invocation.wait_selector.is_some()
                || invocation.wait_text.is_some()
        }
    };

    if unsupported {
        return Err(CliError::usage(
            invocation.json,
            wait_irrelevant_flag_message(wait_kind),
            wait_condition_usage_hint(wait_kind),
        ));
    }

    Ok(())
}

fn wait_irrelevant_flag_message(wait_kind: WaitConditionName) -> &'static str {
    match wait_kind {
        WaitConditionName::Ready => "Unsupported flag for wait ready.",
        WaitConditionName::Url => "Unsupported flag for wait url.",
        WaitConditionName::Selector => "Unsupported flag for wait selector.",
        WaitConditionName::Text => "Unsupported flag for wait text.",
        WaitConditionName::Quiet => "Unsupported flag for wait quiet.",
    }
}

fn wait_condition_usage_hint(wait_kind: WaitConditionName) -> &'static str {
    match wait_kind {
        WaitConditionName::Ready => {
            "Run lantern wait ready [--state <loading|interactive|complete>] --timeout-ms <MS>."
        }
        WaitConditionName::Url => "Run lantern wait url --url-shape <URL_SHAPE> --timeout-ms <MS>.",
        WaitConditionName::Selector => {
            "Run lantern wait selector --selector <CSS_SELECTOR> --timeout-ms <MS>."
        }
        WaitConditionName::Text => {
            "Run lantern wait text --selector <CSS_SELECTOR> --text <TEXT> --timeout-ms <MS>."
        }
        WaitConditionName::Quiet => "Run lantern wait quiet --quiet-ms <MS> --timeout-ms <MS>.",
    }
}

fn run_command(
    command: Command,
    endpoint: ResolvedEndpoint,
    json: bool,
    no_redact: bool,
    target_id: Option<String>,
    open_url: Option<String>,
    wait_kind: Option<WaitConditionName>,
    timeout_ms: Option<u64>,
    wait_state: Option<String>,
    wait_url_shape: Option<String>,
    wait_selector: Option<String>,
    wait_text: Option<String>,
    quiet_ms: Option<u64>,
    screenshot_output: Option<String>,
    screenshot_overwrite: bool,
    dom_depth: Option<usize>,
    dom_max_nodes: Option<usize>,
) -> Result<(), CliError> {
    let client = CdpClient::new(endpoint.clone());

    match command {
        Command::Doctor => {
            let version = client
                .browser_version()
                .map_err(|error| CliError::from_cdp(error, json))?;
            write_doctor(endpoint, version, json)?;
        }
        Command::Targets => {
            let targets = client
                .targets()
                .map_err(|error| CliError::from_cdp(error, json))?;
            write_targets(targets, json, no_redact)?;
        }
        Command::Page => {
            let targets = client
                .targets()
                .map_err(|error| CliError::from_cdp(error, json))?;
            let page = select_page_target(targets, target_id.as_deref())
                .map_err(|error| error.with_json(json))?;
            write_page(page, json, no_redact)?;
        }
        Command::Dom => {
            let targets = client
                .targets()
                .map_err(|error| CliError::from_cdp(error, json))?;
            let page = select_page_target(targets, target_id.as_deref())
                .map_err(|error| error.with_json(json))?;
            let options = build_dom_summary_options(dom_depth, dom_max_nodes, json)?;
            let output = read_dom_summary(&page, RedactionMode::from_no_redact(no_redact), options)
                .map_err(|error| CliError::from_dom_read(error, json))?;
            write_dom(output, json)?;
        }
        Command::Open => {
            let requested_url = open_url
                .as_deref()
                .expect("open URL checked before endpoint");
            validate_navigation_url(requested_url)
                .map_err(|error| CliError::from_navigation(error, json))?;
            let targets = client
                .targets()
                .map_err(|error| CliError::from_cdp(error, json))?;
            let page = select_page_target(targets, target_id.as_deref())
                .map_err(|error| error.with_json(json))?;
            let output = navigate_page(
                &page,
                requested_url,
                RedactionMode::from_no_redact(no_redact),
            )
            .map_err(|error| CliError::from_navigation(error, json))?;
            write_open(output, json)?;
        }
        Command::Wait => {
            let condition = build_wait_condition(
                wait_kind.expect("wait kind checked before endpoint"),
                wait_state,
                wait_url_shape,
                wait_selector,
                wait_text,
                quiet_ms,
                timeout_ms,
                json,
            )?;
            let timeout_ms = timeout_ms.expect("wait timeout checked with condition");
            let targets = client
                .targets()
                .map_err(|error| CliError::from_cdp(error, json))?;
            let page = select_page_target(targets, target_id.as_deref())
                .map_err(|error| error.with_json(json))?;
            let output = wait_for_condition(
                &page,
                condition,
                Duration::from_millis(timeout_ms),
                RedactionMode::from_no_redact(no_redact),
            )
            .map_err(|error| CliError::from_wait(error, json))?;
            write_wait(output, json)?;
        }
        Command::Console => {
            let targets = client
                .targets()
                .map_err(|error| CliError::from_cdp(error, json))?;
            let page = select_page_target(targets, target_id.as_deref())
                .map_err(|error| error.with_json(json))?;
            let output = read_console_errors(&page, RedactionMode::from_no_redact(no_redact))
                .map_err(|error| CliError::from_console_read(error, json))?;
            write_console(output, json)?;
        }
        Command::Network => {
            let targets = client
                .targets()
                .map_err(|error| CliError::from_cdp(error, json))?;
            let page = select_page_target(targets, target_id.as_deref())
                .map_err(|error| error.with_json(json))?;
            let output = read_network_failures(&page, RedactionMode::from_no_redact(no_redact))
                .map_err(|error| CliError::from_network_read(error, json))?;
            write_network(output, json)?;
        }
        Command::Screenshot => {
            let output_path = screenshot_output
                .as_deref()
                .expect("screenshot output checked before endpoint");
            validate_screenshot_output_path(output_path, screenshot_overwrite, json)?;
            let targets = client
                .targets()
                .map_err(|error| CliError::from_cdp(error, json))?;
            let page = select_page_target(targets, target_id.as_deref())
                .map_err(|error| error.with_json(json))?;
            let capture = capture_visible_viewport_screenshot(
                &page,
                RedactionMode::from_no_redact(no_redact),
            )
            .map_err(|error| CliError::from_screenshot(error, json))?;
            let overwritten =
                write_screenshot_file(output_path, &capture.bytes, screenshot_overwrite, json)?;
            let output = ScreenshotCommandOutput::success(
                capture.page,
                ScreenshotSummary {
                    format: capture.format,
                    width: capture.width,
                    height: capture.height,
                    byte_count: capture.bytes.len(),
                    path: output_path.to_owned(),
                    overwritten,
                    redaction_caveat: SCREENSHOT_REDACTION_CAVEAT,
                },
            );
            write_screenshot(output, json)?;
        }
        Command::Click => {
            let selector = wait_selector
                .as_deref()
                .expect("interaction selector checked before endpoint");
            let timeout_ms = timeout_ms.expect("interaction timeout checked before endpoint");
            validate_interaction_timeout(timeout_ms, json)?;
            let targets = client
                .targets()
                .map_err(|error| CliError::from_cdp(error, json))?;
            let page = select_page_target(targets, target_id.as_deref())
                .map_err(|error| error.with_json(json))?;
            let output = click_element(
                &page,
                selector,
                Duration::from_millis(timeout_ms),
                RedactionMode::from_no_redact(no_redact),
            )
            .map_err(|error| CliError::from_interaction(error, json))?;
            write_interaction(output, json)?;
        }
        Command::Type => {
            let selector = wait_selector
                .as_deref()
                .expect("interaction selector checked before endpoint");
            let text = wait_text
                .as_deref()
                .expect("type text checked before endpoint");
            let timeout_ms = timeout_ms.expect("interaction timeout checked before endpoint");
            validate_interaction_timeout(timeout_ms, json)?;
            let targets = client
                .targets()
                .map_err(|error| CliError::from_cdp(error, json))?;
            let page = select_page_target(targets, target_id.as_deref())
                .map_err(|error| error.with_json(json))?;
            let output = type_text(
                &page,
                selector,
                text,
                Duration::from_millis(timeout_ms),
                RedactionMode::from_no_redact(no_redact),
            )
            .map_err(|error| CliError::from_interaction(error, json))?;
            write_interaction(output, json)?;
        }
    }

    Ok(())
}

fn build_dom_summary_options(
    dom_depth: Option<usize>,
    dom_max_nodes: Option<usize>,
    json: bool,
) -> Result<DomSummaryOptions, CliError> {
    let depth = dom_depth.unwrap_or(DOM_DEFAULT_DEPTH);
    let max_nodes = dom_max_nodes.unwrap_or(DOM_DEFAULT_MAX_NODES);
    DomSummaryOptions::new(depth, max_nodes).ok_or_else(|| CliError {
        exit_code: 2,
        json,
        code: "dom_limit_invalid",
        message: DOM_LIMIT_INVALID_MESSAGE,
        hint: DOM_LIMIT_INVALID_HINT,
    })
}

fn build_wait_condition(
    wait_kind: WaitConditionName,
    wait_state: Option<String>,
    wait_url_shape: Option<String>,
    wait_selector: Option<String>,
    wait_text: Option<String>,
    quiet_ms: Option<u64>,
    timeout_ms: Option<u64>,
    json: bool,
) -> Result<WaitCondition, CliError> {
    let timeout_ms = timeout_ms.ok_or_else(|| {
        CliError::usage(
            json,
            "Missing --timeout-ms for wait.",
            "Run lantern wait <condition> --timeout-ms <MS>.",
        )
    })?;
    validate_wait_timeout(timeout_ms, None, json)?;

    match wait_kind {
        WaitConditionName::Ready => {
            let state = match wait_state.as_deref() {
                Some(value) => ReadyState::parse(value).ok_or_else(|| {
                    CliError::usage(
                        json,
                        "Invalid ready state.",
                        "Use --state loading, --state interactive, or --state complete.",
                    )
                })?,
                None => ReadyState::Complete,
            };
            Ok(WaitCondition::Ready { state })
        }
        WaitConditionName::Url => {
            let expected_url_shape = wait_url_shape.ok_or_else(|| {
                CliError::usage(
                    json,
                    "Missing --url-shape for wait url.",
                    "Run lantern wait url --url-shape <URL_SHAPE> --timeout-ms <MS>.",
                )
            })?;
            Ok(WaitCondition::Url { expected_url_shape })
        }
        WaitConditionName::Selector => {
            let selector = wait_selector.ok_or_else(|| {
                CliError::usage(
                    json,
                    "Missing --selector for wait selector.",
                    "Run lantern wait selector --selector <CSS_SELECTOR> --timeout-ms <MS>.",
                )
            })?;
            Ok(WaitCondition::Selector { selector })
        }
        WaitConditionName::Text => {
            let selector = wait_selector.ok_or_else(|| {
                CliError::usage(
                    json,
                    "Missing --selector for wait text.",
                    "Run lantern wait text --selector <CSS_SELECTOR> --text <TEXT> --timeout-ms <MS>.",
                )
            })?;
            let text = wait_text.ok_or_else(|| {
                CliError::usage(
                    json,
                    "Missing --text for wait text.",
                    "Run lantern wait text --selector <CSS_SELECTOR> --text <TEXT> --timeout-ms <MS>.",
                )
            })?;
            Ok(WaitCondition::Text { selector, text })
        }
        WaitConditionName::Quiet => {
            let quiet_ms = quiet_ms.ok_or_else(|| {
                CliError::usage(
                    json,
                    "Missing --quiet-ms for wait quiet.",
                    "Run lantern wait quiet --quiet-ms <MS> --timeout-ms <MS>.",
                )
            })?;
            validate_wait_timeout(timeout_ms, Some(quiet_ms), json)?;
            Ok(WaitCondition::Quiet { quiet_ms })
        }
    }
}

fn validate_wait_timeout(
    timeout_ms: u64,
    quiet_ms: Option<u64>,
    json: bool,
) -> Result<(), CliError> {
    let timeout_ok = (1..=WAIT_MAX_TIMEOUT_MS).contains(&timeout_ms);
    let quiet_ok = quiet_ms
        .map(|quiet_ms| (1..=WAIT_MAX_TIMEOUT_MS).contains(&quiet_ms) && quiet_ms <= timeout_ms)
        .unwrap_or(true);

    if timeout_ok && quiet_ok {
        return Ok(());
    }

    Err(CliError {
        exit_code: 2,
        json,
        code: "wait_timeout_invalid",
        message: WAIT_TIMEOUT_INVALID_MESSAGE,
        hint: WAIT_TIMEOUT_INVALID_HINT,
    })
}

fn validate_interaction_timeout(timeout_ms: u64, json: bool) -> Result<(), CliError> {
    if (1..=INTERACTION_MAX_TIMEOUT_MS).contains(&timeout_ms) {
        return Ok(());
    }

    Err(CliError {
        exit_code: 2,
        json,
        code: "interaction_timeout_invalid",
        message: INTERACTION_TIMEOUT_INVALID_MESSAGE,
        hint: INTERACTION_TIMEOUT_INVALID_HINT,
    })
}

fn write_doctor(
    endpoint: ResolvedEndpoint,
    version: BrowserVersion,
    json: bool,
) -> Result<(), CliError> {
    if json {
        write_json(&DoctorOutput {
            schema_version: 1,
            command: "doctor",
            ok: true,
            endpoint: EndpointOutput {
                source: endpoint.source.as_str(),
                display: endpoint.display,
            },
            browser: BrowserOutput {
                product: version.browser,
                protocol_version: version.protocol_version,
            },
            websocket: WebSocketOutput {
                available: version.web_socket_debugger_url.is_some(),
            },
        })?;
        return Ok(());
    }

    let product = version.browser.as_deref().unwrap_or("browser=null");
    let protocol = version.protocol_version.as_deref().unwrap_or("null");
    let websocket = if version.web_socket_debugger_url.is_some() {
        "available"
    } else {
        "unavailable"
    };

    println!(
        "ok: {product} protocol={protocol} endpoint={} websocket={websocket}",
        endpoint.display
    );
    Ok(())
}

fn write_targets(targets: Vec<TargetInfo>, json: bool, no_redact: bool) -> Result<(), CliError> {
    let targets = ordered_targets(targets);
    let output_targets: Vec<TargetOutput> = targets
        .into_iter()
        .map(|target| target_output(target, no_redact))
        .collect();

    if json {
        write_json(&TargetsOutput {
            schema_version: 1,
            command: "targets",
            ok: true,
            targets: output_targets,
        })?;
        return Ok(());
    }

    for target in output_targets {
        let attached = match target.attached {
            Some(true) => "attached",
            Some(false) => "detached",
            None => "attached=null",
        };
        let title = target.title.as_deref().unwrap_or("null");
        let url_shape = target.url_shape.as_deref().unwrap_or("null");

        println!(
            "{} {} {attached} title=\"{}\" url={url_shape}",
            short_target_id(&target.id),
            target.kind,
            escape_human(title)
        );
    }

    Ok(())
}

fn write_page(target: TargetInfo, json: bool, no_redact: bool) -> Result<(), CliError> {
    let target = target_output(target, no_redact);
    let page = PageOutput {
        target_id: target.id,
        title: target.title,
        url_shape: target.url_shape,
        loading_state: None,
    };

    if json {
        write_json(&PageCommandOutput {
            schema_version: 1,
            command: "page",
            ok: true,
            page,
        })?;
        return Ok(());
    }

    println!(
        "page: {} title=\"{}\" url={} loading={}",
        short_target_id(&page.target_id),
        escape_human(page.title.as_deref().unwrap_or("null")),
        page.url_shape.as_deref().unwrap_or("null"),
        page.loading_state.as_deref().unwrap_or("null")
    );
    Ok(())
}

fn write_dom(output: DomCommandOutput, json: bool) -> Result<(), CliError> {
    if json {
        write_json(&output)?;
        return Ok(());
    }

    println!(
        "dom: {} title=\"{}\" url={} nodes={} depth={} truncated={}",
        short_target_id(&output.page.target_id),
        escape_human(output.page.title.as_deref().unwrap_or("null")),
        output.page.url_shape.as_deref().unwrap_or("null"),
        output.dom.node_count,
        output.dom.max_depth,
        output.dom.truncated
    );

    for node in &output.dom.nodes {
        write_dom_node(node, 0);
    }

    Ok(())
}

fn write_open(output: NavigationCommandOutput, json: bool) -> Result<(), CliError> {
    if json {
        write_json(&output)?;
        return Ok(());
    }

    println!(
        "open: {} requested={} final={} loading={}",
        short_target_id(&output.page.target_id),
        output.navigation.requested_url_shape,
        output
            .navigation
            .final_url_shape
            .as_deref()
            .unwrap_or("null"),
        output.navigation.loading_state.as_deref().unwrap_or("null")
    );
    Ok(())
}

fn write_wait(output: WaitCommandOutput, json: bool) -> Result<(), CliError> {
    if json {
        write_json(&output)?;
        return Ok(());
    }

    println!(
        "wait: {} condition={} matched={} timed_out={} elapsed_ms={} timeout_ms={} observed={}",
        short_target_id(&output.page.target_id),
        wait_condition_label(output.wait.condition),
        output.wait.matched,
        output.wait.timed_out,
        output.wait.elapsed_ms,
        output.wait.timeout_ms,
        wait_observed_human(&output.wait.observed)
    );
    Ok(())
}

fn write_console(output: ConsoleCommandOutput, json: bool) -> Result<(), CliError> {
    if json {
        write_json(&output)?;
        return Ok(());
    }

    println!(
        "console: {} title=\"{}\" url={} messages={} exceptions={} truncated={}",
        short_target_id(&output.page.target_id),
        escape_human(output.page.title.as_deref().unwrap_or("null")),
        output.page.url_shape.as_deref().unwrap_or("null"),
        output.console.message_count,
        output.console.exception_count,
        output.console.truncated
    );

    for entry in &output.console.entries {
        write_console_entry(entry);
    }

    Ok(())
}

fn write_network(output: NetworkCommandOutput, json: bool) -> Result<(), CliError> {
    if json {
        write_json(&output)?;
        return Ok(());
    }

    println!(
        "network: {} title=\"{}\" url={} failed={} http_errors={} observed_clean={} collection_gap={} truncated={}",
        short_target_id(&output.page.target_id),
        escape_human(output.page.title.as_deref().unwrap_or("null")),
        output.page.url_shape.as_deref().unwrap_or("null"),
        output.network.failed_count,
        output.network.http_error_count,
        output.network.observed_clean,
        output.network.collection_gap,
        output.network.truncated
    );

    for entry in &output.network.entries {
        write_network_entry(entry);
    }

    Ok(())
}

fn write_screenshot(output: ScreenshotCommandOutput, json: bool) -> Result<(), CliError> {
    if json {
        write_json(&output)?;
        return Ok(());
    }

    let dimensions = match (output.screenshot.width, output.screenshot.height) {
        (Some(width), Some(height)) => format!("{width}x{height}"),
        _ => "unknown".to_owned(),
    };

    println!(
        "screenshot: {} title=\"{}\" url={} dimensions={} bytes={} path={} overwritten={} caveat={}",
        short_target_id(&output.page.target_id),
        escape_human(output.page.title.as_deref().unwrap_or("null")),
        output.page.url_shape.as_deref().unwrap_or("null"),
        dimensions,
        output.screenshot.byte_count,
        output.screenshot.path,
        output.screenshot.overwritten,
        output.screenshot.redaction_caveat
    );
    Ok(())
}

fn write_interaction(output: InteractionCommandOutput, json: bool) -> Result<(), CliError> {
    if json {
        write_json(&output)?;
        return Ok(());
    }

    println!(
        "{}: {} title=\"{}\" url={} selector=\"{}\" dispatched={} timed_out={} elapsed_ms={} timeout_ms={} observed={} error={}",
        output.command,
        short_target_id(&output.page.target_id),
        escape_human(output.page.title.as_deref().unwrap_or("null")),
        output.page.url_shape.as_deref().unwrap_or("null"),
        escape_human(&output.interaction.selector),
        output.interaction.dispatched,
        output.interaction.timed_out,
        output.interaction.elapsed_ms,
        output.interaction.timeout_ms,
        interaction_observed_human(&output.interaction.observed),
        output.interaction.immediate_error.unwrap_or("null")
    );
    Ok(())
}

fn validate_screenshot_output_path(
    output_path: &str,
    overwrite: bool,
    json: bool,
) -> Result<(), CliError> {
    if !overwrite && Path::new(output_path).exists() {
        return Err(CliError {
            exit_code: 2,
            json,
            code: "screenshot_output_exists",
            message: SCREENSHOT_OUTPUT_EXISTS_MESSAGE,
            hint: SCREENSHOT_OUTPUT_EXISTS_HINT,
        });
    }

    Ok(())
}

fn write_screenshot_file(
    output_path: &str,
    bytes: &[u8],
    overwrite: bool,
    json: bool,
) -> Result<bool, CliError> {
    let path = Path::new(output_path);
    let existed = path.exists();
    if overwrite {
        fs::write(path, bytes).map_err(|_| {
            CliError::runtime(
                json,
                "screenshot_write_failed",
                SCREENSHOT_WRITE_FAILED_MESSAGE,
                SCREENSHOT_WRITE_FAILED_HINT,
            )
        })?;
        return Ok(existed);
    }

    match fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
    {
        Ok(mut file) => {
            use std::io::Write;
            file.write_all(bytes).map_err(|_| {
                CliError::runtime(
                    json,
                    "screenshot_write_failed",
                    SCREENSHOT_WRITE_FAILED_MESSAGE,
                    SCREENSHOT_WRITE_FAILED_HINT,
                )
            })?;
            Ok(false)
        }
        Err(source) if source.kind() == ErrorKind::AlreadyExists => Err(CliError {
            exit_code: 2,
            json,
            code: "screenshot_output_exists",
            message: SCREENSHOT_OUTPUT_EXISTS_MESSAGE,
            hint: SCREENSHOT_OUTPUT_EXISTS_HINT,
        }),
        Err(_) => Err(CliError::runtime(
            json,
            "screenshot_write_failed",
            SCREENSHOT_WRITE_FAILED_MESSAGE,
            SCREENSHOT_WRITE_FAILED_HINT,
        )),
    }
}

fn wait_condition_label(condition: WaitConditionName) -> &'static str {
    match condition {
        WaitConditionName::Ready => "ready",
        WaitConditionName::Url => "url",
        WaitConditionName::Selector => "selector",
        WaitConditionName::Text => "text",
        WaitConditionName::Quiet => "quiet",
    }
}

fn wait_observed_human(observed: &lantern_core::wait::WaitObservedState) -> String {
    match observed {
        lantern_core::wait::WaitObservedState::Ready { ready_state } => ready_state
            .as_deref()
            .unwrap_or("ready_state=null")
            .to_owned(),
        lantern_core::wait::WaitObservedState::Url {
            current_url_shape, ..
        } => current_url_shape
            .as_deref()
            .unwrap_or("current_url_shape=null")
            .to_owned(),
        lantern_core::wait::WaitObservedState::Selector { present, .. } => {
            format!("present={present}")
        }
        lantern_core::wait::WaitObservedState::Text { current_text, .. } => format!(
            "text=\"{}\"",
            escape_human(current_text.as_deref().unwrap_or("null"))
        ),
        lantern_core::wait::WaitObservedState::Quiet {
            quiet_ms,
            event_count,
            last_event,
        } => format!(
            "quiet_ms={quiet_ms} events={event_count} last_event={}",
            last_event.as_deref().unwrap_or("null")
        ),
    }
}

fn write_console_entry(entry: &ConsoleEntry) {
    let location = match (&entry.source_url_shape, entry.line, entry.column) {
        (Some(url), Some(line), Some(column)) => format!("{url}:{line}:{column}"),
        (Some(url), Some(line), None) => format!("{url}:{line}"),
        (Some(url), None, _) => url.clone(),
        (None, _, _) => "source=null".to_owned(),
    };
    println!(
        "{} {:?} {} args={} frames={} \"{}\"",
        entry.sequence,
        entry.kind,
        location,
        entry.argument_count,
        entry.stack_frame_count,
        escape_human(&entry.message)
    );
}

fn write_network_entry(entry: &NetworkEntry) {
    println!(
        "{} {} {} {} resource={} status={} reason={}",
        entry.sequence,
        network_entry_kind_label(&entry.kind),
        entry.method.as_deref().unwrap_or("method=null"),
        entry.url_shape.as_deref().unwrap_or("url=null"),
        entry.resource_type.as_deref().unwrap_or("null"),
        entry
            .status
            .map(|status| status.to_string())
            .unwrap_or_else(|| "null".to_owned()),
        entry.failure_reason.as_deref().unwrap_or("null")
    );
}

fn network_entry_kind_label(kind: &lantern_core::network::NetworkEntryKind) -> &'static str {
    match kind {
        lantern_core::network::NetworkEntryKind::FailedRequest => "failed_request",
        lantern_core::network::NetworkEntryKind::HttpErrorResponse => "http_error_response",
    }
}

fn interaction_observed_human(
    observed: &lantern_core::interaction::InteractionObservedState,
) -> String {
    let point = observed
        .clickable_point
        .map(|point| format!("{},{}", point.x, point.y))
        .unwrap_or_else(|| "null".to_owned());
    let inserted = observed
        .inserted_text_length
        .map(|length| length.to_string())
        .unwrap_or_else(|| "null".to_owned());
    format!(
        "node={} point={} inserted_text_length={}",
        observed.node_name.as_deref().unwrap_or("null"),
        point,
        inserted
    )
}

fn write_dom_node(node: &DomNodeSummary, indent: usize) {
    let mut label = node.tag.clone();
    if let Some(id) = node.attributes.get("id") {
        label.push('#');
        label.push_str(id);
    }
    for attr in ["data-testid", "data-test", "data-cy", "role"] {
        if let Some(value) = node.attributes.get(attr) {
            label.push('[');
            label.push_str(attr);
            label.push('=');
            label.push_str(value);
            label.push(']');
        }
    }
    if let Some(text) = &node.text {
        label.push_str(" \"");
        label.push_str(&escape_human(text));
        label.push('"');
    }

    println!("{}{}", "  ".repeat(indent), label);
    for child in &node.children {
        write_dom_node(child, indent + 1);
    }
}

fn write_json<T: Serialize>(value: &T) -> Result<(), CliError> {
    let json = serde_json::to_string(value).map_err(|_| {
        CliError::runtime(
            true,
            "cdp_response_invalid",
            CDP_RESPONSE_INVALID_MESSAGE,
            CDP_RESPONSE_INVALID_HINT,
        )
    })?;

    println!("{json}");
    Ok(())
}

fn ordered_targets(mut targets: Vec<TargetInfo>) -> Vec<TargetInfo> {
    targets.sort_by_key(|target| (target.kind != "page", target.attached != Some(true)));
    targets
}

fn select_page_target(
    targets: Vec<TargetInfo>,
    target_id: Option<&str>,
) -> Result<TargetInfo, CliError> {
    if let Some(target_id) = target_id {
        return targets
            .into_iter()
            .find(|target| target.id == target_id && target.kind == "page")
            .ok_or_else(|| {
                CliError::runtime(
                    false,
                    "target_not_found",
                    TARGET_ID_NOT_FOUND_MESSAGE,
                    TARGET_ID_NOT_FOUND_HINT,
                )
            });
    }

    let page_targets: Vec<TargetInfo> = targets
        .into_iter()
        .filter(|target| target.kind == "page")
        .collect();

    match page_targets.len() {
        0 => Err(CliError::runtime(
            false,
            "target_not_found",
            TARGET_NOT_FOUND_MESSAGE,
            TARGET_NOT_FOUND_HINT,
        )),
        1 => Ok(page_targets.into_iter().next().expect("one page target")),
        _ => {
            let attached: Vec<TargetInfo> = page_targets
                .iter()
                .filter(|target| target.attached == Some(true))
                .cloned()
                .collect();
            if attached.len() == 1 {
                Ok(attached.into_iter().next().expect("one attached target"))
            } else {
                Err(
                    CliError::usage(false, TARGET_AMBIGUOUS_MESSAGE, TARGET_AMBIGUOUS_HINT)
                        .with_code("target_ambiguous"),
                )
            }
        }
    }
}

fn target_output(target: TargetInfo, no_redact: bool) -> TargetOutput {
    let mode = RedactionMode::from_no_redact(no_redact);
    TargetOutput {
        id: target.id,
        kind: target.kind,
        title: target.title.map(|title| sanitize_title(&title, mode)),
        url_shape: target.url.and_then(|url| sanitize_url(&url, mode)),
        attached: target.attached,
    }
}

fn short_target_id(id: &str) -> &str {
    id.get(..8).unwrap_or(id)
}

fn escape_human(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn print_help() {
    println!(
        "Usage: lantern <doctor|targets|page|dom|open|wait|console|network|screenshot|click|type> [--endpoint <URL>] [--json] [--no-redact] [--target-id <ID>]\n       lantern open <URL> [--endpoint <URL>] [--json] [--no-redact] [--target-id <ID>]\n       lantern wait <ready|url|selector|text|quiet> --timeout-ms <MS> [condition flags]\n       lantern screenshot --output <PATH> [--overwrite]\n       lantern click --selector <CSS> --timeout-ms <MS>\n       lantern type --selector <CSS> --text <TEXT> --timeout-ms <MS>\n\nShared flags:\n  --endpoint <URL>  Local Chromium CDP HTTP endpoint\n  --json            Emit JSON on stdout; errors remain on stderr\n  --no-redact       Disable redaction for command output metadata\n  --target-id <ID>  Exact CDP page target id for selected-page commands\n\nWait and interaction flags:\n  --timeout-ms <MS> Explicit timeout from 1 through 30000\n  --state <STATE>   ready state: loading, interactive, or complete\n  --url-shape <URL> Expected URL shape for wait url\n  --selector <CSS>  CSS selector for wait selector/text, click, or type\n  --text <TEXT>     Text substring for wait text, or inserted text for type\n  --quiet-ms <MS>   Quiet period for wait quiet\n\nScreenshot flags:\n  --output <PATH>   Required local PNG output path\n  --overwrite       Replace an existing output file\n\n  -h, --help        Print help\n  -V, --version     Print version"
    );
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Invocation {
    command: Option<Command>,
    endpoint: Option<String>,
    json: bool,
    no_redact: bool,
    target_id: Option<String>,
    open_url: Option<String>,
    wait_kind: Option<WaitConditionName>,
    timeout_ms: Option<u64>,
    wait_state: Option<String>,
    wait_url_shape: Option<String>,
    wait_selector: Option<String>,
    wait_text: Option<String>,
    quiet_ms: Option<u64>,
    screenshot_output: Option<String>,
    screenshot_overwrite: bool,
    dom_depth: Option<usize>,
    dom_max_nodes: Option<usize>,
    help: bool,
    version: bool,
}

#[derive(Debug, Serialize)]
struct DoctorOutput {
    schema_version: u8,
    command: &'static str,
    ok: bool,
    endpoint: EndpointOutput,
    browser: BrowserOutput,
    websocket: WebSocketOutput,
}

#[derive(Debug, Serialize)]
struct EndpointOutput {
    source: &'static str,
    display: String,
}

#[derive(Debug, Serialize)]
struct BrowserOutput {
    product: Option<String>,
    protocol_version: Option<String>,
}

#[derive(Debug, Serialize)]
struct WebSocketOutput {
    available: bool,
}

#[derive(Debug, Serialize)]
struct TargetsOutput {
    schema_version: u8,
    command: &'static str,
    ok: bool,
    targets: Vec<TargetOutput>,
}

#[derive(Debug, Serialize)]
struct TargetOutput {
    id: String,
    #[serde(rename = "type")]
    kind: String,
    title: Option<String>,
    url_shape: Option<String>,
    attached: Option<bool>,
}

#[derive(Debug, Serialize)]
struct PageCommandOutput {
    schema_version: u8,
    command: &'static str,
    ok: bool,
    page: PageOutput,
}

#[derive(Debug, Serialize)]
struct PageOutput {
    target_id: String,
    title: Option<String>,
    url_shape: Option<String>,
    loading_state: Option<String>,
}

impl Invocation {
    fn has_wait_only_flags(&self) -> bool {
        self.wait_kind.is_some()
            || self.wait_state.is_some()
            || self.wait_url_shape.is_some()
            || self.quiet_ms.is_some()
    }

    fn has_screenshot_flags(&self) -> bool {
        self.screenshot_output.is_some() || self.screenshot_overwrite
    }

    fn has_dom_flags(&self) -> bool {
        self.dom_depth.is_some() || self.dom_max_nodes.is_some()
    }

    fn parse(args: impl IntoIterator<Item = String>) -> Result<Self, CliError> {
        let mut invocation = Self {
            command: None,
            endpoint: None,
            json: false,
            no_redact: false,
            target_id: None,
            open_url: None,
            wait_kind: None,
            timeout_ms: None,
            wait_state: None,
            wait_url_shape: None,
            wait_selector: None,
            wait_text: None,
            quiet_ms: None,
            screenshot_output: None,
            screenshot_overwrite: false,
            dom_depth: None,
            dom_max_nodes: None,
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
                "--overwrite" => invocation.screenshot_overwrite = true,
                "doctor" | "targets" | "page" | "dom" | "open" | "wait" | "console" | "network"
                | "screenshot" | "click" | "type" => {
                    if invocation.command.is_some() {
                        return Err(CliError::usage(
                            invocation.json,
                            "Multiple commands provided.",
                            "Run one command at a time.",
                        ));
                    }
                    invocation.command = Some(Command::from_name(&arg).expect("matched command"));
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

fn parse_wait_condition(value: &str, json: bool) -> Result<WaitConditionName, CliError> {
    match value {
        "ready" => Ok(WaitConditionName::Ready),
        "url" => Ok(WaitConditionName::Url),
        "selector" => Ok(WaitConditionName::Selector),
        "text" => Ok(WaitConditionName::Text),
        "quiet" => Ok(WaitConditionName::Quiet),
        _ => Err(CliError::usage(
            json,
            "Unknown wait condition.",
            "Use ready, url, selector, text, or quiet.",
        )),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Command {
    Doctor,
    Targets,
    Page,
    Dom,
    Open,
    Wait,
    Console,
    Network,
    Screenshot,
    Click,
    Type,
}

impl Command {
    fn from_name(name: &str) -> Option<Self> {
        match name {
            "doctor" => Some(Self::Doctor),
            "targets" => Some(Self::Targets),
            "page" => Some(Self::Page),
            "dom" => Some(Self::Dom),
            "open" => Some(Self::Open),
            "wait" => Some(Self::Wait),
            "console" => Some(Self::Console),
            "network" => Some(Self::Network),
            "screenshot" => Some(Self::Screenshot),
            "click" => Some(Self::Click),
            "type" => Some(Self::Type),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CliError {
    exit_code: u8,
    json: bool,
    code: &'static str,
    message: &'static str,
    hint: &'static str,
}

impl CliError {
    fn usage(json: bool, message: &'static str, hint: &'static str) -> Self {
        Self {
            exit_code: 2,
            json,
            code: "usage",
            message,
            hint,
        }
    }

    fn runtime(json: bool, code: &'static str, message: &'static str, hint: &'static str) -> Self {
        Self {
            exit_code: 1,
            json,
            code,
            message,
            hint,
        }
    }

    fn with_code(mut self, code: &'static str) -> Self {
        self.code = code;
        self
    }

    fn with_json(mut self, json: bool) -> Self {
        self.json = json;
        self
    }

    fn from_endpoint(error: EndpointResolutionError, json: bool) -> Self {
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

    fn from_cdp(error: CdpError, json: bool) -> Self {
        match error {
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

    fn from_dom_read(error: DomReadError, json: bool) -> Self {
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

    fn from_navigation(error: NavigationError, json: bool) -> Self {
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

    fn from_console_read(error: ConsoleReadError, json: bool) -> Self {
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

    fn from_network_read(error: NetworkReadError, json: bool) -> Self {
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

    fn from_wait(error: WaitError, json: bool) -> Self {
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

    fn from_screenshot(error: ScreenshotError, json: bool) -> Self {
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

    fn from_interaction(error: InteractionError, json: bool) -> Self {
        match error {
            InteractionError::TargetWebSocketMissing => Self::runtime(
                json,
                "target_websocket_missing",
                TARGET_WEBSOCKET_MISSING_MESSAGE,
                TARGET_WEBSOCKET_MISSING_HINT,
            ),
            InteractionError::Cdp(CdpError::WebSocketTransport { .. }) => Self::runtime(
                json,
                "endpoint_unreachable",
                ENDPOINT_UNREACHABLE_MESSAGE,
                ENDPOINT_UNREACHABLE_HINT,
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

    fn write_stderr(&self) {
        if self.json {
            let error = ErrorOutput {
                schema_version: 1,
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
    fn parses_flag_endpoint_and_command_in_any_order() {
        let invocation = Invocation::parse([
            "--json".to_string(),
            "--endpoint".to_string(),
            "http://127.0.0.1:9222".to_string(),
            "doctor".to_string(),
        ])
        .expect("invocation should parse");

        assert_eq!(invocation.command, Some(Command::Doctor));
        assert_eq!(
            invocation.endpoint.as_deref(),
            Some("http://127.0.0.1:9222")
        );
        assert!(invocation.json);
        assert_eq!(invocation.target_id, None);
    }

    #[test]
    fn parses_exact_target_id_selector() {
        let invocation = Invocation::parse([
            "page".to_string(),
            "--target-id".to_string(),
            "PAGE_123".to_string(),
        ])
        .expect("invocation should parse");

        assert_eq!(invocation.command, Some(Command::Page));
        assert_eq!(invocation.target_id.as_deref(), Some("PAGE_123"));
    }

    #[test]
    fn parses_wait_condition_and_timeout_flags() {
        let invocation = Invocation::parse([
            "wait".to_string(),
            "text".to_string(),
            "--selector".to_string(),
            "main".to_string(),
            "--text".to_string(),
            "Ready".to_string(),
            "--timeout-ms".to_string(),
            "5000".to_string(),
        ])
        .expect("invocation should parse");

        assert_eq!(invocation.command, Some(Command::Wait));
        assert_eq!(invocation.wait_kind, Some(WaitConditionName::Text));
        assert_eq!(invocation.wait_selector.as_deref(), Some("main"));
        assert_eq!(invocation.wait_text.as_deref(), Some("Ready"));
        assert_eq!(invocation.timeout_ms, Some(5000));
    }

    #[test]
    fn parses_dom_depth_and_node_limit_flags() {
        let invocation = Invocation::parse([
            "dom".to_string(),
            "--depth".to_string(),
            "8".to_string(),
            "--max-nodes".to_string(),
            "200".to_string(),
        ])
        .expect("invocation should parse");

        assert_eq!(invocation.command, Some(Command::Dom));
        assert_eq!(invocation.dom_depth, Some(8));
        assert_eq!(invocation.dom_max_nodes, Some(200));
    }

    #[test]
    fn parses_screenshot_output_and_overwrite_flags() {
        let invocation = Invocation::parse([
            "screenshot".to_string(),
            "--output".to_string(),
            "artifacts/page.png".to_string(),
            "--overwrite".to_string(),
        ])
        .expect("invocation should parse");

        assert_eq!(invocation.command, Some(Command::Screenshot));
        assert_eq!(
            invocation.screenshot_output.as_deref(),
            Some("artifacts/page.png")
        );
        assert!(invocation.screenshot_overwrite);
    }

    #[test]
    fn parses_network_command() {
        let invocation = Invocation::parse([
            "network".to_string(),
            "--target-id".to_string(),
            "PAGE_123".to_string(),
        ])
        .expect("invocation should parse");

        assert_eq!(invocation.command, Some(Command::Network));
        assert_eq!(invocation.target_id.as_deref(), Some("PAGE_123"));
    }

    #[test]
    fn endpoint_errors_use_contract_codes() {
        let missing = CliError::from_endpoint(EndpointResolutionError::Missing, true);

        assert_eq!(missing.exit_code, 2);
        assert_eq!(missing.code, "endpoint_missing");
        assert!(missing.json);
    }

    #[test]
    fn url_shape_omits_query_and_redacts_sensitive_path_segments() {
        let url = sanitize_url(
            "https://user:pass@example.test/reset/4a7f9c0e2d1b4c6a8e9f0123456789ab?token=secret#frag",
            RedactionMode::Redacted,
        );

        assert_eq!(url.as_deref(), Some("https://example.test/reset/:redacted"));
    }

    #[test]
    fn page_selection_uses_single_attached_page_when_multiple_pages_exist() {
        let selected = select_page_target(
            vec![
                target("PAGE_A", "page", Some(false)),
                target("PAGE_B", "page", Some(true)),
                target("WORKER", "service_worker", Some(true)),
            ],
            None,
        )
        .expect("one attached page should be selected");

        assert_eq!(selected.id, "PAGE_B");
    }

    #[test]
    fn page_selection_uses_exact_target_id_when_provided() {
        let selected = select_page_target(
            vec![
                target("PAGE_A", "page", Some(false)),
                target("PAGE_B", "page", Some(true)),
            ],
            Some("PAGE_A"),
        )
        .expect("requested page target should be selected");

        assert_eq!(selected.id, "PAGE_A");
    }

    #[test]
    fn page_selection_rejects_exact_target_id_for_non_page_target() {
        let error = select_page_target(
            vec![
                target("PAGE_A", "page", Some(true)),
                target("WORKER", "service_worker", Some(true)),
            ],
            Some("WORKER"),
        )
        .expect_err("non-page target id should not match");

        assert_eq!(error.exit_code, 1);
        assert_eq!(error.code, "target_not_found");
        assert_eq!(error.message, TARGET_ID_NOT_FOUND_MESSAGE);
    }

    #[test]
    fn page_selection_reports_ambiguous_pages_as_usage_error() {
        let error = select_page_target(
            vec![
                target("PAGE_A", "page", Some(false)),
                target("PAGE_B", "page", Some(false)),
            ],
            None,
        )
        .expect_err("multiple detached pages are ambiguous");

        assert_eq!(error.exit_code, 2);
        assert_eq!(error.code, "target_ambiguous");
    }

    fn target(id: &str, kind: &str, attached: Option<bool>) -> TargetInfo {
        TargetInfo {
            id: id.to_owned(),
            kind: kind.to_owned(),
            title: Some(format!("Title {id}")),
            url: Some(format!("https://example.test/{id}")),
            attached,
            browser_context_id: None,
            web_socket_debugger_url: None,
        }
    }
}
