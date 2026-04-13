use std::{env, process::ExitCode};

use lantern_core::{
    cdp::{BrowserVersion, CdpClient, CdpError, TargetInfo},
    endpoint::{EndpointResolutionError, ResolvedEndpoint, resolve_endpoint},
};
use serde::Serialize;
use url::{Host, Url};

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
const TARGET_AMBIGUOUS_MESSAGE: &str = "Multiple page targets matched.";
const TARGET_AMBIGUOUS_HINT: &str =
    "Close extra pages or leave exactly one attached page target before retrying.";

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

    let endpoint = resolve_endpoint(invocation.endpoint.as_deref(), env_endpoint.as_deref())
        .map_err(|error| CliError::from_endpoint(error, invocation.json))?;

    run_command(command, endpoint, invocation.json, invocation.no_redact)
}

fn run_command(
    command: Command,
    endpoint: ResolvedEndpoint,
    json: bool,
    no_redact: bool,
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
            let page = select_page_target(targets).map_err(|error| error.with_json(json))?;
            write_page(page, json, no_redact)?;
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

fn select_page_target(targets: Vec<TargetInfo>) -> Result<TargetInfo, CliError> {
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
    TargetOutput {
        id: target.id,
        kind: target.kind,
        title: target.title.map(|title| sanitize_text(&title, no_redact)),
        url_shape: target.url.and_then(|url| sanitize_url(&url, no_redact)),
        attached: target.attached,
    }
}

fn sanitize_text(value: &str, no_redact: bool) -> String {
    let normalized = value
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .chars()
        .map(|ch| {
            if ch.is_ascii_control() && ch != '\n' && ch != '\t' {
                ' '
            } else {
                ch
            }
        })
        .collect::<String>();

    if no_redact {
        return normalized;
    }

    truncate_scalars(&normalized, 120)
}

fn truncate_scalars(value: &str, limit: usize) -> String {
    if value.chars().count() <= limit {
        return value.to_owned();
    }

    value.chars().take(limit).collect::<String>() + "..."
}

fn sanitize_url(value: &str, no_redact: bool) -> Option<String> {
    if no_redact {
        return Some(value.to_owned());
    }

    let mut url = Url::parse(value).ok()?;
    let scheme = url.scheme().to_owned();
    let host = display_host(url.host()?);
    let port = url.port();
    let path = sanitized_path(url.path());
    url.set_username("").ok()?;
    url.set_password(None).ok()?;
    url.set_query(None);
    url.set_fragment(None);

    let mut output = format!("{scheme}://{host}");
    if let Some(port) = port.filter(|port| !is_default_port(&scheme, *port)) {
        output.push_str(&format!(":{port}"));
    }
    output.push_str(&path);
    Some(output)
}

fn sanitized_path(path: &str) -> String {
    if path.is_empty() || path == "/" {
        return "/".to_owned();
    }

    let segments = path
        .split('/')
        .filter(|segment| !segment.is_empty())
        .scan(false, |redact_next, segment| {
            let redact = *redact_next || is_sensitive_segment(segment);
            *redact_next = is_sensitive_label(segment);
            Some(if redact { ":redacted" } else { segment })
        })
        .collect::<Vec<_>>();

    format!("/{}", segments.join("/"))
}

fn is_default_port(scheme: &str, port: u16) -> bool {
    matches!(
        (scheme, port),
        ("http", 80) | ("https", 443) | ("ws", 80) | ("wss", 443)
    )
}

fn display_host(host: Host<&str>) -> String {
    match host {
        Host::Domain(domain) => domain.to_owned(),
        Host::Ipv4(addr) => addr.to_string(),
        Host::Ipv6(addr) => format!("[{addr}]"),
    }
}

fn is_sensitive_segment(segment: &str) -> bool {
    segment.chars().count() > 64
        || segment.contains('@')
        || segment.starts_with("eyJ")
        || is_long_hex(segment)
        || is_long_token(segment)
}

fn is_sensitive_label(segment: &str) -> bool {
    matches!(
        segment.to_ascii_lowercase().as_str(),
        "token"
            | "key"
            | "secret"
            | "session"
            | "auth"
            | "password"
            | "passwd"
            | "invite"
            | "reset"
            | "code"
            | "otp"
            | "jwt"
            | "access_token"
            | "refresh_token"
    )
}

fn is_long_hex(segment: &str) -> bool {
    segment.len() >= 24 && segment.chars().all(|ch| ch.is_ascii_hexdigit())
}

fn is_long_token(segment: &str) -> bool {
    segment.len() >= 32
        && segment
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '=' | '+'))
}

fn short_target_id(id: &str) -> &str {
    id.get(..8).unwrap_or(id)
}

fn escape_human(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn print_help() {
    println!(
        "Usage: lantern <doctor|targets|page> [--endpoint <URL>] [--json] [--no-redact]\n\nShared flags:\n  --endpoint <URL>  Local Chromium CDP HTTP endpoint\n  --json            Emit JSON on stdout; errors remain on stderr\n  --no-redact       Disable redaction for command output\n  -h, --help        Print help\n  -V, --version     Print version"
    );
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Invocation {
    command: Option<Command>,
    endpoint: Option<String>,
    json: bool,
    no_redact: bool,
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
                "doctor" | "targets" | "page" => {
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
}

impl Command {
    fn from_name(name: &str) -> Option<Self> {
        match name {
            "doctor" => Some(Self::Doctor),
            "targets" => Some(Self::Targets),
            "page" => Some(Self::Page),
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
            false,
        );

        assert_eq!(url.as_deref(), Some("https://example.test/reset/:redacted"));
    }

    #[test]
    fn page_selection_uses_single_attached_page_when_multiple_pages_exist() {
        let selected = select_page_target(vec![
            target("PAGE_A", "page", Some(false)),
            target("PAGE_B", "page", Some(true)),
            target("WORKER", "service_worker", Some(true)),
        ])
        .expect("one attached page should be selected");

        assert_eq!(selected.id, "PAGE_B");
    }

    #[test]
    fn page_selection_reports_ambiguous_pages_as_usage_error() {
        let error = select_page_target(vec![
            target("PAGE_A", "page", Some(false)),
            target("PAGE_B", "page", Some(false)),
        ])
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
