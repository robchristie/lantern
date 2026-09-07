use crate::{
    args::Invocation,
    dispatch::EndpointContext,
    error::CliError,
    output::write_json,
    screenshot::{validate_screenshot_output_path, write_screenshot_file},
    selection::select_page_target,
};
use lantern_core::{
    action_flow::{Postcondition, Verdict, run_action_flow_until},
    redaction::RedactionMode,
    screenshot::{SCREENSHOT_REDACTION_CAVEAT, ScreenshotSummary},
};

pub(crate) fn validate(i: &Invocation) -> Result<(), CliError> {
    let condition_valid = match (&i.expect_selector, &i.expect_text, &i.expect_url) {
        (Some(selector), text, None) => {
            !selector.is_empty() && text.as_ref().is_none_or(|t| !t.is_empty())
        }
        (None, None, Some(url)) => !url.is_empty(),
        _ => false,
    };
    if !condition_valid
        || i.region_x.is_some()
        || i.region_y.is_some()
        || i.region_width.is_some()
        || i.region_height.is_some()
        || (i.screenshot_overwrite && i.screenshot_output.is_none())
    {
        return Err(CliError::usage(
            i.json,
            "Invalid action-flow expectation or capture flags.",
            "Use --expect-selector <CSS> [--expect-text <TEXT>] or --expect-url <URL>; optional --output <PNG> [--overwrite].",
        ));
    }
    Ok(())
}

pub(crate) fn run_action_flow(context: EndpointContext) -> Result<bool, CliError> {
    let EndpointContext {
        invocation: i,
        client,
        budget,
        ..
    } = context;
    if let Some(path) = i.screenshot_output.as_deref() {
        validate_screenshot_output_path(path, i.screenshot_overwrite, i.json)?;
        // Reject special sinks before mutation, including overwrite of a FIFO.
        if std::fs::metadata(path).is_ok_and(|m| !m.is_file()) {
            return Err(CliError::usage(
                i.json,
                "Capture requires a regular file.",
                "Choose a local PNG file path.",
            ));
        }
    }
    let targets = client
        .targets()
        .map_err(|e| CliError::from_cdp(e, i.json))?;
    let page =
        select_page_target(targets, i.target_id.as_deref()).map_err(|e| e.with_json(i.json))?;
    let condition = if let Some(url) = i.expect_url {
        Postcondition::Url { url }
    } else if let Some(text) = i.expect_text {
        Postcondition::Text {
            selector: i.expect_selector.unwrap(),
            text,
        }
    } else {
        Postcondition::Selector {
            selector: i.expect_selector.unwrap(),
        }
    };
    let output = run_action_flow_until(
        &page,
        i.wait_selector.as_deref().unwrap(),
        condition,
        RedactionMode::from_no_redact(i.no_redact),
        budget.unwrap(),
        i.screenshot_output.is_some(),
        |capture| {
            let path = i.screenshot_output.as_deref().unwrap();
            let overwritten =
                write_screenshot_file(path, &capture.bytes, i.screenshot_overwrite, i.json)
                    .map_err(|e| e.code)?;
            Ok(ScreenshotSummary {
                format: capture.format,
                width: capture.width,
                height: capture.height,
                region: capture.region,
                byte_count: capture.bytes.len(),
                path: path.to_owned(),
                overwritten,
                redaction_caveat: SCREENSHOT_REDACTION_CAVEAT,
            })
        },
    )
    .map_err(|e| CliError::from_flow(e, i.json))?;
    let success = !i.strict || output.verdict == Verdict::Passed;
    if i.json {
        write_json(&output)?;
    } else {
        println!(
            "action-flow: verdict={:?} dispatch={:?} matched={} baseline={:?} capture={} errors={} http_errors={} network_failures={} evidence_incomplete={} error={}",
            output.verdict,
            output.interaction.dispatch_state,
            output.postcondition.matched,
            output.postcondition.matched_before_action,
            output.capture.status,
            output.console.message_count + output.console.exception_count,
            output.network.http_error_count,
            output.network.failed_count,
            output.console.evidence_loss.incomplete() || output.network.evidence_loss.incomplete(),
            output
                .error
                .or(output.capture.error)
                .or(output.interaction.immediate_error)
                .unwrap_or("none")
        );
        if output.capture.requested {
            println!(
                "capture: path={} caveat={}",
                crate::output::escape_human(i.screenshot_output.as_deref().unwrap()),
                SCREENSHOT_REDACTION_CAVEAT
            );
        }
    }
    Ok(success)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{registry::Command, validation::validate_endpoint_invocation};

    #[test]
    fn validates_narrow_action_flow_before_endpoint_access() {
        for args in [
            vec![
                "action-flow",
                "--selector",
                "#save",
                "--timeout-ms",
                "100",
                "--expect-selector",
                "#state",
                "--expect-text",
                "saved",
                "--strict",
            ],
            vec![
                "action-flow",
                "--selector",
                "#save",
                "--timeout-ms",
                "100",
                "--expect-url",
                "http://localhost/done",
                "--output",
                "capture.png",
            ],
        ] {
            let invocation = Invocation::parse(args.into_iter().map(str::to_owned)).unwrap();
            validate_endpoint_invocation(&invocation, Command::ActionFlow).unwrap();
        }
        for flags in [
            vec![],
            vec!["--expect-text", "saved"],
            vec!["--expect-selector", ""],
            vec![
                "--expect-selector",
                "#state",
                "--expect-url",
                "http://localhost",
            ],
            vec!["--expect-selector", "#state", "--quiet-ms", "100"],
            vec!["--expect-selector", "#state", "--text", "secret"],
            vec!["--expect-selector", "#state", "--region-x", "0"],
            vec!["--expect-selector", "#state", "--overwrite"],
        ] {
            let invocation = Invocation::parse(
                ["action-flow", "--selector", "#save", "--timeout-ms", "100"]
                    .into_iter()
                    .chain(flags)
                    .map(str::to_owned),
            )
            .unwrap();
            assert!(validate_endpoint_invocation(&invocation, Command::ActionFlow).is_err());
        }
    }
}
