use crate::args::Invocation;
use crate::dispatch::EndpointContext;
use crate::error::{
    CliError, DOM_LIMIT_INVALID_HINT, DOM_LIMIT_INVALID_MESSAGE, WAIT_TIMEOUT_INVALID_HINT,
    WAIT_TIMEOUT_INVALID_MESSAGE,
};
use crate::output::{CLI_SCHEMA_VERSION, escape_human, write_json};
use crate::registry::Command;
use crate::selection::{select_page_target, short_target_id, write_page, write_targets};
use lantern_core::cdp::BrowserVersion;
use lantern_core::console::{ConsoleCommandOutput, ConsoleEntry, read_console_errors};
use lantern_core::dom::{
    DOM_DEFAULT_DEPTH, DOM_DEFAULT_MAX_NODES, DomCommandOutput, DomNodeSummary, DomSummaryOptions,
    read_dom_summary,
};
use lantern_core::endpoint::ResolvedEndpoint;
use lantern_core::flow::{FlowCommandOutput, FlowOptions, run_observation_flow_until};
use lantern_core::layout::{LayoutCommandOutput, LayoutFinding, read_layout_audit};
use lantern_core::navigation::{NavigationCommandOutput, navigate_page, validate_navigation_url};
use lantern_core::network::{NetworkCommandOutput, NetworkEntry, read_network_failures};
use lantern_core::redaction::RedactionMode;
use lantern_core::wait::{
    ReadyState, WAIT_MAX_TIMEOUT_MS, WaitCommandOutput, WaitCondition, WaitConditionName,
    wait_for_condition,
};
use serde::Serialize;
use std::time::Duration;

pub(crate) fn run_inspection(context: EndpointContext) -> Result<(), CliError> {
    let EndpointContext {
        command,
        invocation,
        client,
        endpoint,
        budget,
    } = context;
    let Invocation {
        json,
        no_redact,
        target_id,
        open_url,
        wait_kind,
        timeout_ms,
        wait_state,
        wait_url_shape,
        wait_selector,
        wait_text,
        quiet_ms,
        dom_depth,
        dom_max_nodes,
        ..
    } = invocation;
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

            let targets = client
                .targets()
                .map_err(|error| CliError::from_cdp(error, json))?;
            let page = select_page_target(targets, target_id.as_deref())
                .map_err(|error| error.with_json(json))?;
            let output = wait_for_condition(
                &page,
                condition,
                budget.expect("bounded command budget"),
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
        Command::Flow => {
            let timeout_ms = timeout_ms.expect("flow timeout checked before endpoint");
            let targets = client
                .targets()
                .map_err(|error| CliError::from_cdp(error, json))?;
            let page = select_page_target(targets, target_id.as_deref())
                .map_err(|error| error.with_json(json))?;
            let output = run_observation_flow_until(
                &page,
                FlowOptions {
                    open_url,
                    timeout: Duration::from_millis(timeout_ms),
                    quiet_ms,
                },
                RedactionMode::from_no_redact(no_redact),
                budget.expect("bounded command budget"),
            )
            .map_err(|error| CliError::from_flow(error, json))?;
            write_flow(output, json)?;
        }
        Command::Layout => {
            let targets = client
                .targets()
                .map_err(|error| CliError::from_cdp(error, json))?;
            let page = select_page_target(targets, target_id.as_deref())
                .map_err(|error| error.with_json(json))?;
            let output = read_layout_audit(&page, RedactionMode::from_no_redact(no_redact))
                .map_err(|error| CliError::from_layout_read(error, json))?;
            write_layout(output, json)?;
        }
        _ => unreachable!("dispatcher routes only inspection commands"),
    }
    Ok(())
}

pub(crate) fn build_dom_summary_options(
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

pub(crate) fn validate_wait_timeout(
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

fn write_doctor(
    endpoint: ResolvedEndpoint,
    version: BrowserVersion,
    json: bool,
) -> Result<(), CliError> {
    if json {
        write_json(&DoctorOutput {
            schema_version: CLI_SCHEMA_VERSION,
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

    write_evidence_loss("wait", &output.evidence_loss);

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

pub(crate) fn write_evidence_loss(owner: &str, loss: &lantern_core::cdp::EvidenceLoss) {
    if loss.incomplete() {
        println!(
            "evidence_loss: {owner} dropped_events={} dropped_event_bytes={} drain_limit_reached={} collection_deadline_reached={}",
            loss.dropped_events,
            loss.dropped_event_bytes,
            loss.drain_limit_reached,
            loss.collection_deadline_reached
        );
    }
}

fn write_console(output: ConsoleCommandOutput, json: bool) -> Result<(), CliError> {
    if json {
        write_json(&output)?;
        return Ok(());
    }

    write_evidence_loss("console", &output.console.evidence_loss);

    println!(
        "console: {} title=\"{}\" url={} messages={} exceptions={} observed_clean={} collection_gap={} truncated={}",
        short_target_id(&output.page.target_id),
        escape_human(output.page.title.as_deref().unwrap_or("null")),
        output.page.url_shape.as_deref().unwrap_or("null"),
        output.console.message_count,
        output.console.exception_count,
        output.console.observed_clean,
        output.console.collection_gap,
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

    write_evidence_loss("network", &output.network.evidence_loss);

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

fn write_flow(output: FlowCommandOutput, json: bool) -> Result<(), CliError> {
    if json {
        write_json(&output)?;
        return Ok(());
    }

    write_evidence_loss("console", &output.console.evidence_loss);
    write_evidence_loss("network", &output.network.evidence_loss);

    println!(
        "flow: {} title=\"{}\" url={} opened={} single_attachment={} before_navigation={} wait_condition={} matched={} timed_out={} elapsed_ms={} timeout_ms={} console_messages={} console_exceptions={} network_failed={} network_http_errors={} console_gap={} network_gap={}",
        short_target_id(&output.page.target_id),
        escape_human(output.page.title.as_deref().unwrap_or("null")),
        output.page.url_shape.as_deref().unwrap_or("null"),
        output.flow.opened_url,
        output.flow.single_attachment,
        output.flow.observation_started_before_navigation,
        wait_condition_label(output.flow.wait.condition),
        output.flow.wait.matched,
        output.flow.wait.timed_out,
        output.flow.wait.elapsed_ms,
        output.flow.wait.timeout_ms,
        output.console.message_count,
        output.console.exception_count,
        output.network.failed_count,
        output.network.http_error_count,
        output.console.collection_gap,
        output.network.collection_gap,
    );

    for entry in &output.console.entries {
        write_console_entry(entry);
    }
    for entry in &output.network.entries {
        write_network_entry(entry);
    }

    Ok(())
}

fn write_layout(output: LayoutCommandOutput, json: bool) -> Result<(), CliError> {
    if json {
        write_json(&output)?;
        return Ok(());
    }

    println!(
        "layout: {} title=\"{}\" url={} findings={} truncated={} scroll_width={} client_width={}",
        short_target_id(&output.page.target_id),
        escape_human(output.page.title.as_deref().unwrap_or("null")),
        output.page.url_shape.as_deref().unwrap_or("null"),
        output.layout.finding_count,
        output.layout.truncated,
        output.layout.viewport.scroll_width,
        output.layout.viewport.client_width,
    );

    for finding in &output.layout.findings {
        write_layout_finding(finding);
    }

    Ok(())
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

fn write_layout_finding(finding: &LayoutFinding) {
    println!(
        "  {} severity={} selector={} text=\"{}\" right={} container_right={}",
        finding.kind,
        finding.severity,
        escape_human(&finding.selector),
        escape_human(finding.text_sample.as_deref().unwrap_or("")),
        finding.metrics.right,
        finding
            .metrics
            .container_right
            .map(|value| value.to_string())
            .unwrap_or_else(|| "null".to_owned()),
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
