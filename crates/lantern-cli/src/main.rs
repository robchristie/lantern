use std::{env, process::ExitCode};

use lantern_core::{
    cdp::{BrowserVersion, CdpClient, CdpError, TargetInfo},
    console::{ConsoleCommandOutput, ConsoleEntry, ConsoleReadError, read_console_errors},
    dom::{DomCommandOutput, DomNodeSummary, DomReadError, read_dom_summary},
    endpoint::{EndpointResolutionError, ResolvedEndpoint, resolve_endpoint},
    navigation::{
        NavigationCommandOutput, NavigationError, navigate_page, validate_navigation_url,
    },
    redaction::{RedactionMode, sanitize_title, sanitize_url},
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
            Command::Page | Command::Dom | Command::Open | Command::Console
        )
    {
        return Err(CliError::usage(
            invocation.json,
            "--target-id is only supported by page, dom, open, and console.",
            "Run lantern page --target-id <CDP_TARGET_ID>, lantern dom --target-id <CDP_TARGET_ID>, lantern open --target-id <CDP_TARGET_ID> <URL>, or lantern console --target-id <CDP_TARGET_ID>.",
        ));
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
    )
}

fn run_command(
    command: Command,
    endpoint: ResolvedEndpoint,
    json: bool,
    no_redact: bool,
    target_id: Option<String>,
    open_url: Option<String>,
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
            let output = read_dom_summary(&page, RedactionMode::from_no_redact(no_redact))
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
    }

    Ok(())
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
        "Usage: lantern <doctor|targets|page|dom|open|console> [--endpoint <URL>] [--json] [--no-redact] [--target-id <ID>]\n       lantern open <URL> [--endpoint <URL>] [--json] [--no-redact] [--target-id <ID>]\n\nShared flags:\n  --endpoint <URL>  Local Chromium CDP HTTP endpoint\n  --json            Emit JSON on stdout; errors remain on stderr\n  --no-redact       Disable redaction for command output\n  --target-id <ID>  Exact CDP page target id for page, dom, open, and console\n  -h, --help        Print help\n  -V, --version     Print version"
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
    fn parse(args: impl IntoIterator<Item = String>) -> Result<Self, CliError> {
        let mut invocation = Self {
            command: None,
            endpoint: None,
            json: false,
            no_redact: false,
            target_id: None,
            open_url: None,
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
                "doctor" | "targets" | "page" | "dom" | "open" | "console" => {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Command {
    Doctor,
    Targets,
    Page,
    Dom,
    Open,
    Console,
}

impl Command {
    fn from_name(name: &str) -> Option<Self> {
        match name {
            "doctor" => Some(Self::Doctor),
            "targets" => Some(Self::Targets),
            "page" => Some(Self::Page),
            "dom" => Some(Self::Dom),
            "open" => Some(Self::Open),
            "console" => Some(Self::Console),
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
