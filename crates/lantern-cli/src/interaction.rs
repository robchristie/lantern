use crate::args::Invocation;
use crate::dispatch::EndpointContext;
use crate::error::{
    CliError, INTERACTION_DELTA_INVALID_HINT, INTERACTION_DELTA_INVALID_MESSAGE,
    INTERACTION_DURATION_INVALID_HINT, INTERACTION_DURATION_INVALID_MESSAGE,
    INTERACTION_TIMEOUT_INVALID_HINT, INTERACTION_TIMEOUT_INVALID_MESSAGE, TYPE_INPUT_INVALID_HINT,
    TYPE_INPUT_INVALID_MESSAGE, TYPE_INPUT_TOO_LARGE_HINT, TYPE_INPUT_TOO_LARGE_MESSAGE,
};
use crate::inspection::write_evidence_loss;
use crate::output::{escape_human, write_json};
use crate::registry::Command;
use crate::selection::{select_page_target, short_target_id};
use lantern_core::interaction::{
    INTERACTION_MAX_DURATION_MS, INTERACTION_MAX_TIMEOUT_MS, InteractionCommandOutput,
    click_element, drag_element, hover_element, press_key, type_text, wheel_element,
};
use lantern_core::redaction::RedactionMode;
use std::fs::OpenOptions;
use std::io::Read;
use std::path::Path;
use std::time::Duration;

pub(crate) fn run_interaction(context: EndpointContext) -> Result<bool, CliError> {
    let EndpointContext {
        command,
        invocation,
        client,
        budget,
        ..
    } = context;
    let Invocation {
        json,
        no_redact,
        strict,
        target_id,
        timeout_ms,
        wait_selector,
        wait_text,
        key,
        delta_x,
        delta_y,
        duration_ms,
        ..
    } = invocation;
    let successful;
    match command {
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
                budget.expect("bounded command budget"),
                RedactionMode::from_no_redact(no_redact),
            )
            .map_err(|error| CliError::from_interaction(error, json))?;
            successful = output.ok
                && (!strict
                    || (output.interaction.dispatched
                        && !output.interaction.timed_out
                        && output.interaction.immediate_error.is_none()));
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
                budget.expect("bounded command budget"),
                RedactionMode::from_no_redact(no_redact),
            )
            .map_err(|error| CliError::from_interaction(error, json))?;
            successful = output.ok
                && (!strict
                    || (output.interaction.dispatched
                        && !output.interaction.timed_out
                        && output.interaction.immediate_error.is_none()));
            write_interaction(output, json)?;
        }
        Command::Key => {
            let selector = wait_selector
                .as_deref()
                .expect("interaction selector checked before endpoint");
            let key = key.as_deref().expect("key checked before endpoint");
            let timeout_ms = timeout_ms.expect("interaction timeout checked before endpoint");
            validate_interaction_timeout(timeout_ms, json)?;
            let targets = client
                .targets()
                .map_err(|error| CliError::from_cdp(error, json))?;
            let page = select_page_target(targets, target_id.as_deref())
                .map_err(|error| error.with_json(json))?;
            let output = press_key(
                &page,
                selector,
                key,
                budget.expect("bounded command budget"),
                RedactionMode::from_no_redact(no_redact),
            )
            .map_err(|error| CliError::from_interaction(error, json))?;
            successful = output.ok
                && (!strict
                    || (output.interaction.dispatched
                        && !output.interaction.timed_out
                        && output.interaction.immediate_error.is_none()));
            write_interaction(output, json)?;
        }
        Command::Hover => {
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
            let output = hover_element(
                &page,
                selector,
                budget.expect("bounded command budget"),
                RedactionMode::from_no_redact(no_redact),
            )
            .map_err(|error| CliError::from_interaction(error, json))?;
            successful = output.ok
                && (!strict
                    || (output.interaction.dispatched
                        && !output.interaction.timed_out
                        && output.interaction.immediate_error.is_none()));
            write_interaction(output, json)?;
        }
        Command::Wheel => {
            let selector = wait_selector
                .as_deref()
                .expect("interaction selector checked before endpoint");
            let timeout_ms = timeout_ms.expect("interaction timeout checked before endpoint");
            validate_interaction_timeout(timeout_ms, json)?;
            validate_wheel_delta(delta_x, delta_y, json)?;
            let targets = client
                .targets()
                .map_err(|error| CliError::from_cdp(error, json))?;
            let page = select_page_target(targets, target_id.as_deref())
                .map_err(|error| error.with_json(json))?;
            let output = wheel_element(
                &page,
                selector,
                delta_x.unwrap_or(0.0),
                delta_y.unwrap_or(0.0),
                budget.expect("bounded command budget"),
                RedactionMode::from_no_redact(no_redact),
            )
            .map_err(|error| CliError::from_interaction(error, json))?;
            successful = output.ok
                && (!strict
                    || (output.interaction.dispatched
                        && !output.interaction.timed_out
                        && output.interaction.immediate_error.is_none()));
            write_interaction(output, json)?;
        }
        Command::Drag => {
            let selector = wait_selector
                .as_deref()
                .expect("interaction selector checked before endpoint");
            let timeout_ms = timeout_ms.expect("interaction timeout checked before endpoint");
            let duration_ms = duration_ms.expect("drag duration checked before endpoint");
            validate_interaction_timeout(timeout_ms, json)?;
            validate_interaction_duration(duration_ms, json)?;
            validate_drag_delta(delta_x, delta_y, json)?;
            let targets = client
                .targets()
                .map_err(|error| CliError::from_cdp(error, json))?;
            let page = select_page_target(targets, target_id.as_deref())
                .map_err(|error| error.with_json(json))?;
            let output = drag_element(
                &page,
                selector,
                delta_x.unwrap_or(0.0),
                delta_y.unwrap_or(0.0),
                Duration::from_millis(duration_ms),
                budget.expect("bounded command budget"),
                RedactionMode::from_no_redact(no_redact),
            )
            .map_err(|error| CliError::from_interaction(error, json))?;
            successful = output.ok
                && (!strict
                    || (output.interaction.dispatched
                        && !output.interaction.timed_out
                        && output.interaction.immediate_error.is_none()));
            write_interaction(output, json)?;
        }
        _ => unreachable!("dispatcher routes only interaction commands"),
    }
    Ok(successful)
}

const TYPE_TEXT_FILE_MAX_BYTES: u64 = 64 * 1024;

pub(crate) fn read_type_text_file(path: &Path, json: bool) -> Result<String, CliError> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK);
    }

    let file = options
        .open(path)
        .map_err(|_| type_text_file_invalid(json))?;
    let metadata = file.metadata().map_err(|_| type_text_file_invalid(json))?;
    if !metadata.is_file() {
        return Err(type_text_file_invalid(json));
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err(type_text_file_invalid(json));
        }
    }

    if metadata.len() > TYPE_TEXT_FILE_MAX_BYTES {
        return Err(type_text_file_too_large(json));
    }

    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.take(TYPE_TEXT_FILE_MAX_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| type_text_file_invalid(json))?;
    if bytes.len() as u64 > TYPE_TEXT_FILE_MAX_BYTES {
        return Err(type_text_file_too_large(json));
    }

    String::from_utf8(bytes).map_err(|_| type_text_file_invalid(json))
}

fn type_text_file_invalid(json: bool) -> CliError {
    CliError::usage(json, TYPE_INPUT_INVALID_MESSAGE, TYPE_INPUT_INVALID_HINT)
        .with_code("interaction_input_invalid")
}

fn type_text_file_too_large(json: bool) -> CliError {
    CliError::usage(
        json,
        TYPE_INPUT_TOO_LARGE_MESSAGE,
        TYPE_INPUT_TOO_LARGE_HINT,
    )
    .with_code("interaction_input_too_large")
}

pub(crate) fn validate_interaction_timeout(timeout_ms: u64, json: bool) -> Result<(), CliError> {
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

pub(crate) fn validate_interaction_duration(duration_ms: u64, json: bool) -> Result<(), CliError> {
    if duration_ms <= INTERACTION_MAX_DURATION_MS {
        return Ok(());
    }

    Err(CliError {
        exit_code: 2,
        json,
        code: "interaction_duration_invalid",
        message: INTERACTION_DURATION_INVALID_MESSAGE,
        hint: INTERACTION_DURATION_INVALID_HINT,
    })
}

pub(crate) fn validate_wheel_delta(
    delta_x: Option<f64>,
    delta_y: Option<f64>,
    json: bool,
) -> Result<(), CliError> {
    let delta_x = delta_x.unwrap_or(0.0);
    let delta_y = delta_y.unwrap_or(0.0);
    if delta_x.is_finite() && delta_y.is_finite() && (delta_x != 0.0 || delta_y != 0.0) {
        return Ok(());
    }

    Err(CliError {
        exit_code: 2,
        json,
        code: "interaction_delta_invalid",
        message: INTERACTION_DELTA_INVALID_MESSAGE,
        hint: INTERACTION_DELTA_INVALID_HINT,
    })
}

pub(crate) fn validate_drag_delta(
    delta_x: Option<f64>,
    delta_y: Option<f64>,
    json: bool,
) -> Result<(), CliError> {
    let delta_x = delta_x.unwrap_or(0.0);
    let delta_y = delta_y.unwrap_or(0.0);
    if delta_x.is_finite() && delta_y.is_finite() {
        return Ok(());
    }

    Err(CliError {
        exit_code: 2,
        json,
        code: "interaction_delta_invalid",
        message: INTERACTION_DELTA_INVALID_MESSAGE,
        hint: INTERACTION_DELTA_INVALID_HINT,
    })
}

fn write_interaction(output: InteractionCommandOutput, json: bool) -> Result<(), CliError> {
    if json {
        write_json(&output)?;
        return Ok(());
    }

    write_evidence_loss("interaction", &output.interaction.evidence_loss);

    println!(
        "{}: {} title=\"{}\" url={} selector=\"{}\" dispatched={} dispatch_state={} application_outcome={} timed_out={} elapsed_ms={} timeout_ms={} observed={} error={}",
        output.command,
        short_target_id(&output.page.target_id),
        escape_human(output.page.title.as_deref().unwrap_or("null")),
        output.page.url_shape.as_deref().unwrap_or("null"),
        escape_human(&output.interaction.selector),
        output.interaction.dispatched,
        match output.interaction.dispatch_state {
            lantern_core::interaction::DispatchState::NotDispatched => "not_dispatched",
            lantern_core::interaction::DispatchState::Acknowledged => "acknowledged",
            lantern_core::interaction::DispatchState::Uncertain => "uncertain",
        },
        output.interaction.application_outcome,
        output.interaction.timed_out,
        output.interaction.elapsed_ms,
        output.interaction.timeout_ms,
        interaction_observed_human(&output.interaction.observed),
        output.interaction.immediate_error.unwrap_or("null")
    );
    Ok(())
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
    let key = observed.key.as_deref().unwrap_or("null");
    let key_events = observed
        .key_event_count
        .map(|count| count.to_string())
        .unwrap_or_else(|| "null".to_owned());
    let pointer_start = observed
        .pointer_start
        .map(|point| format!("{},{}", point.x, point.y))
        .unwrap_or_else(|| "null".to_owned());
    let pointer_end = observed
        .pointer_end
        .map(|point| format!("{},{}", point.x, point.y))
        .unwrap_or_else(|| "null".to_owned());
    let delta_x = observed
        .delta_x
        .map(|delta| delta.to_string())
        .unwrap_or_else(|| "null".to_owned());
    let delta_y = observed
        .delta_y
        .map(|delta| delta.to_string())
        .unwrap_or_else(|| "null".to_owned());
    let duration = observed
        .duration_ms
        .map(|duration| duration.to_string())
        .unwrap_or_else(|| "null".to_owned());
    let input_events = observed
        .input_event_count
        .map(|count| count.to_string())
        .unwrap_or_else(|| "null".to_owned());
    format!(
        "node={} point={} inserted_text_length={} key={} key_event_count={} pointer_start={} pointer_end={} delta_x={} delta_y={} duration_ms={} input_event_count={}",
        observed.node_name.as_deref().unwrap_or("null"),
        point,
        inserted,
        escape_human(key),
        key_events,
        pointer_start,
        pointer_end,
        delta_x,
        delta_y,
        duration,
        input_events
    )
}
