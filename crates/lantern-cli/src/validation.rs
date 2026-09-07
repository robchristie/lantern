use crate::args::Invocation;
use crate::error::{BROWSER_USAGE_HINT, CliError};
use crate::inspection::{build_dom_summary_options, validate_wait_timeout};
use crate::interaction::{
    validate_drag_delta, validate_interaction_duration, validate_interaction_timeout,
    validate_wheel_delta,
};
use crate::registry::Command;
use crate::screenshot::build_screenshot_region;
use lantern_core::navigation::validate_navigation_url;
use lantern_core::wait::WaitConditionName;

pub(crate) fn validate_endpoint_invocation(
    invocation: &Invocation,
    command: Command,
) -> Result<(), CliError> {
    if invocation.has_browser_lifecycle_flags() {
        return Err(CliError::usage(
            invocation.json,
            "Browser lifecycle flags are only supported by browser commands.",
            BROWSER_USAGE_HINT,
        ));
    }

    if command == Command::ActionFlow {
        crate::action_flow::validate(invocation)?;
    } else if invocation.expect_selector.is_some()
        || invocation.expect_text.is_some()
        || invocation.expect_url.is_some()
    {
        return Err(CliError::usage(
            invocation.json,
            "Postcondition flags require action-flow.",
            "Run lantern action-flow with an explicit expectation.",
        ));
    }

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
                | Command::Layout
                | Command::Click
                | Command::Type
                | Command::Key
                | Command::Hover
                | Command::Wheel
                | Command::Drag
                | Command::ActionFlow
                | Command::Flow
        )
    {
        return Err(CliError::usage(
            invocation.json,
            "--target-id is only supported by page, dom, open, wait, console, network, screenshot, layout, click, type, key, hover, wheel, drag, flow, and action-flow.",
            "Run a selected-page command with --target-id <CDP_TARGET_ID>.",
        ));
    }

    if !matches!(command, Command::Wait | Command::Flow) && invocation.has_wait_only_flags() {
        return Err(CliError::usage(
            invocation.json,
            "Wait flags are only supported by wait and flow.",
            "Run lantern wait <ready|url|selector|text|quiet>, or lantern flow with --timeout-ms and optional --quiet-ms.",
        ));
    }

    if command == Command::Flow
        && (invocation.wait_kind.is_some()
            || invocation.wait_state.is_some()
            || invocation.wait_url_shape.is_some()
            || invocation.wait_selector.is_some()
            || invocation.wait_text.is_some())
    {
        return Err(CliError::usage(
            invocation.json,
            "Flow supports only --timeout-ms, optional --quiet-ms, optional --open, and target selection.",
            "Run lantern flow --timeout-ms <MS> [--quiet-ms <MS>] [--open <URL>] [--target-id <ID>].",
        ));
    }

    if !matches!(
        command,
        Command::Wait
            | Command::Click
            | Command::Type
            | Command::Key
            | Command::Hover
            | Command::Wheel
            | Command::Drag
            | Command::ActionFlow
            | Command::Flow
    ) && invocation.timeout_ms.is_some()
    {
        return Err(CliError::usage(
            invocation.json,
            "--timeout-ms is only supported by wait, click, type, key, hover, wheel, drag, flow, and action-flow.",
            "Run a bounded command with --timeout-ms <MS>.",
        ));
    }

    if !matches!(
        command,
        Command::Wait
            | Command::Click
            | Command::Type
            | Command::Key
            | Command::Hover
            | Command::Wheel
            | Command::Drag
            | Command::ActionFlow
    ) && invocation.wait_selector.is_some()
    {
        return Err(CliError::usage(
            invocation.json,
            "--selector is only supported by wait, click, type, key, hover, wheel, drag, and action-flow.",
            "Run a selected-element command with --selector <CSS_SELECTOR>.",
        ));
    }

    if !matches!(
        command,
        Command::Wait | Command::Type | Command::Click | Command::Key
    ) && invocation.wait_text.is_some()
    {
        return Err(CliError::usage(
            invocation.json,
            "--text is only supported by wait text and type.",
            "Run lantern wait text or lantern type with --text <TEXT>.",
        ));
    }

    if command != Command::Key && invocation.key.is_some() {
        return Err(CliError::usage(
            invocation.json,
            "--key is only supported by key.",
            "Run lantern key --selector <CSS_SELECTOR> --key <KEY> --timeout-ms <MS>.",
        ));
    }

    if !matches!(command, Command::Wheel | Command::Drag)
        && (invocation.delta_x.is_some() || invocation.delta_y.is_some())
    {
        return Err(CliError::usage(
            invocation.json,
            "--dx and --dy are only supported by wheel and drag.",
            "Run lantern wheel with --dx/--dy, or lantern drag with --dx <PX> --dy <PX>.",
        ));
    }

    if command != Command::Drag && invocation.duration_ms.is_some() {
        return Err(CliError::usage(
            invocation.json,
            "--duration-ms is only supported by drag.",
            "Run lantern drag --selector <CSS_SELECTOR> --dx <PX> --dy <PX> --duration-ms <MS> --timeout-ms <MS>.",
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
        validate_wait_flag_shape(invocation)?;
    }

    if command == Command::Flow {
        let timeout_ms = invocation.timeout_ms.ok_or_else(|| {
            CliError::usage(
                invocation.json,
                "Missing --timeout-ms for flow.",
                "Run lantern flow --timeout-ms <MS> [--quiet-ms <MS>] [--open <URL>].",
            )
        })?;
        validate_wait_timeout(timeout_ms, invocation.quiet_ms, invocation.json)?;
        if let Some(url) = invocation.open_url.as_deref() {
            validate_navigation_url(url)
                .map_err(|error| CliError::from_navigation(error, invocation.json))?;
        }
    }

    if !matches!(command, Command::Open | Command::Flow) && invocation.open_url.is_some() {
        return Err(CliError::usage(
            invocation.json,
            "Navigation URL is only supported by open and flow.",
            "Run lantern open <URL> or lantern flow --open <URL>.",
        ));
    }

    if command != Command::Dom && invocation.has_dom_flags() {
        return Err(CliError::usage(
            invocation.json,
            "DOM limit flags are only supported by dom.",
            "Run lantern dom with --depth <N> or --max-nodes <N>.",
        ));
    }

    if !matches!(command, Command::Screenshot | Command::ActionFlow)
        && invocation.has_screenshot_flags()
    {
        return Err(CliError::usage(
            invocation.json,
            "Capture output flags are only supported by screenshot and action-flow; region flags require screenshot.",
            "Run lantern screenshot --output <PATH>, or add --output <PATH> to action-flow.",
        ));
    }

    if command == Command::Screenshot && invocation.screenshot_output.is_none() {
        return Err(CliError::usage(
            invocation.json,
            "Missing --output for screenshot.",
            "Run lantern screenshot --output <PATH>.",
        ));
    }

    if command == Command::Screenshot {
        build_screenshot_region(
            invocation.region_x,
            invocation.region_y,
            invocation.region_width,
            invocation.region_height,
            invocation.json,
        )?;
    }

    if matches!(
        command,
        Command::Click
            | Command::Type
            | Command::Key
            | Command::Hover
            | Command::Wheel
            | Command::Drag
            | Command::ActionFlow
    ) && invocation.wait_selector.is_none()
    {
        return Err(CliError::usage(
            invocation.json,
            "Missing --selector for interaction.",
            "Run an interaction command with --selector <CSS_SELECTOR> and --timeout-ms <MS>.",
        ));
    }

    if matches!(
        command,
        Command::Click
            | Command::Type
            | Command::Key
            | Command::Hover
            | Command::Wheel
            | Command::Drag
            | Command::ActionFlow
    ) && invocation.timeout_ms.is_none()
    {
        return Err(CliError::usage(
            invocation.json,
            "Missing --timeout-ms for interaction.",
            "Run an interaction command with --timeout-ms <MS> from 1 through 30000.",
        ));
    }

    if command == Command::Type
        && invocation.wait_text.is_none()
        && invocation.type_text_file.is_none()
    {
        return Err(CliError::usage(
            invocation.json,
            "Missing text source for type.",
            "Run lantern type with exactly one of --text <TEXT> or --text-file <PATH>.",
        ));
    }

    if command == Command::Type
        && invocation.wait_text.is_some()
        && invocation.type_text_file.is_some()
    {
        return Err(CliError::usage(
            invocation.json,
            "Multiple text sources for type.",
            "Pass exactly one of --text <TEXT> or --text-file <PATH>.",
        ));
    }

    if command == Command::Click && invocation.wait_text.is_some() {
        return Err(CliError::usage(
            invocation.json,
            "--text is not supported by click.",
            "Run lantern click with --selector <CSS_SELECTOR> --timeout-ms <MS>.",
        ));
    }

    if command == Command::Key && invocation.key.is_none() {
        return Err(CliError::usage(
            invocation.json,
            "Missing --key for key.",
            "Run lantern key --selector <CSS_SELECTOR> --key <KEY> --timeout-ms <MS>.",
        ));
    }

    if command == Command::Key && invocation.wait_text.is_some() {
        return Err(CliError::usage(
            invocation.json,
            "--text is not supported by key.",
            "Run lantern key --selector <CSS_SELECTOR> --key <KEY> --timeout-ms <MS>.",
        ));
    }

    if command == Command::Wheel {
        validate_wheel_delta(invocation.delta_x, invocation.delta_y, invocation.json)?;
    }

    if command == Command::Drag {
        let duration_ms = invocation.duration_ms.ok_or_else(|| {
            CliError::usage(
                invocation.json,
                "Missing --duration-ms for drag.",
                "Run lantern drag --selector <CSS_SELECTOR> --dx <PX> --dy <PX> --duration-ms <MS> --timeout-ms <MS>.",
            )
        })?;
        validate_drag_delta(invocation.delta_x, invocation.delta_y, invocation.json)?;
        validate_interaction_duration(duration_ms, invocation.json)?;
    }

    if matches!(
        command,
        Command::Click
            | Command::Type
            | Command::Key
            | Command::Hover
            | Command::Wheel
            | Command::Drag
            | Command::ActionFlow
    ) {
        validate_interaction_timeout(
            invocation
                .timeout_ms
                .expect("interaction timeout checked before validation"),
            invocation.json,
        )?;
    }

    if command == Command::Dom {
        build_dom_summary_options(
            invocation.dom_depth,
            invocation.dom_max_nodes,
            invocation.json,
        )?;
    }

    Ok(())
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
