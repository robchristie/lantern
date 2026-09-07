use crate::args::Invocation;
use crate::dispatch::EndpointContext;
use crate::error::{
    CliError, SCREENSHOT_OUTPUT_EXISTS_HINT, SCREENSHOT_OUTPUT_EXISTS_MESSAGE,
    SCREENSHOT_REGION_INVALID_HINT, SCREENSHOT_REGION_INVALID_MESSAGE,
    SCREENSHOT_WRITE_FAILED_HINT, SCREENSHOT_WRITE_FAILED_MESSAGE,
};
use crate::output::{escape_human, write_json};
use crate::registry::Command;
use crate::selection::{select_page_target, short_target_id};
use lantern_core::redaction::RedactionMode;
use lantern_core::screenshot::{
    SCREENSHOT_REDACTION_CAVEAT, ScreenshotCommandOutput, ScreenshotRegion, ScreenshotSummary,
    capture_visible_viewport_screenshot,
};
use std::fs;
use std::io::ErrorKind;
use std::path::Path;

pub(crate) fn run_screenshot(context: EndpointContext) -> Result<(), CliError> {
    let EndpointContext {
        command,
        invocation,
        client,
        ..
    } = context;
    let Invocation {
        json,
        no_redact,
        target_id,
        screenshot_output,
        screenshot_overwrite,
        region_x,
        region_y,
        region_width,
        region_height,
        ..
    } = invocation;
    match command {
        Command::Screenshot => {
            let output_path = screenshot_output
                .as_deref()
                .expect("screenshot output checked before endpoint");
            validate_screenshot_output_path(output_path, screenshot_overwrite, json)?;
            let region =
                build_screenshot_region(region_x, region_y, region_width, region_height, json)?;
            let targets = client
                .targets()
                .map_err(|error| CliError::from_cdp(error, json))?;
            let page = select_page_target(targets, target_id.as_deref())
                .map_err(|error| error.with_json(json))?;
            let capture = capture_visible_viewport_screenshot(
                &page,
                RedactionMode::from_no_redact(no_redact),
                region,
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
                    region: capture.region,
                    byte_count: capture.bytes.len(),
                    path: output_path.to_owned(),
                    overwritten,
                    redaction_caveat: SCREENSHOT_REDACTION_CAVEAT,
                },
            );
            write_screenshot(output, json)?;
        }
        _ => unreachable!("dispatcher routes only screenshot commands"),
    }
    Ok(())
}

pub(crate) fn build_screenshot_region(
    x: Option<f64>,
    y: Option<f64>,
    width: Option<f64>,
    height: Option<f64>,
    json: bool,
) -> Result<Option<ScreenshotRegion>, CliError> {
    match (x, y, width, height) {
        (None, None, None, None) => Ok(None),
        (Some(x), Some(y), Some(width), Some(height))
            if x.is_finite()
                && y.is_finite()
                && width.is_finite()
                && height.is_finite()
                && x >= 0.0
                && y >= 0.0
                && width > 0.0
                && height > 0.0 =>
        {
            Ok(Some(ScreenshotRegion {
                x,
                y,
                width,
                height,
            }))
        }
        _ => Err(CliError {
            exit_code: 2,
            json,
            code: "screenshot_region_invalid",
            message: SCREENSHOT_REGION_INVALID_MESSAGE,
            hint: SCREENSHOT_REGION_INVALID_HINT,
        }),
    }
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
    let region = output
        .screenshot
        .region
        .map(|region| {
            format!(
                "{},{},{},{}",
                region.x, region.y, region.width, region.height
            )
        })
        .unwrap_or_else(|| "null".to_owned());

    println!(
        "screenshot: {} title=\"{}\" url={} dimensions={} region={} bytes={} path={} overwritten={} caveat={}",
        short_target_id(&output.page.target_id),
        escape_human(output.page.title.as_deref().unwrap_or("null")),
        output.page.url_shape.as_deref().unwrap_or("null"),
        dimensions,
        region,
        output.screenshot.byte_count,
        output.screenshot.path,
        output.screenshot.overwritten,
        output.screenshot.redaction_caveat
    );
    Ok(())
}

pub(crate) fn validate_screenshot_output_path(
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

pub(crate) fn write_screenshot_file(
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
