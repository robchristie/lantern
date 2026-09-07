mod action_flow;
mod args;
mod browser;
mod capabilities;
mod dispatch;
mod error;
mod inspection;
mod interaction;
mod output;
mod registry;
mod screenshot;
mod selection;
mod validation;

use crate::args::{Invocation, print_help};
use crate::browser::run_browser_invocation;
use crate::dispatch::run_command;
use crate::error::CliError;
use crate::interaction::read_type_text_file;
use crate::registry::Command;
use crate::validation::validate_endpoint_invocation;
use lantern_core::endpoint::resolve_endpoint;
use std::env;
use std::process::ExitCode;

fn main() -> ExitCode {
    match run(env::args().skip(1), env::var("LANTERN_CDP_ENDPOINT").ok()) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(error) => {
            error.write_stderr();
            ExitCode::from(error.exit_code)
        }
    }
}

fn run(
    args: impl IntoIterator<Item = String>,
    env_endpoint: Option<String>,
) -> Result<bool, CliError> {
    let invocation = Invocation::parse(args)?;

    if invocation.help {
        print_help();
        return Ok(true);
    }

    if invocation.version {
        println!("lantern {}", env!("CARGO_PKG_VERSION"));
        return Ok(true);
    }

    let command = invocation.command.ok_or_else(|| {
        CliError::usage(
            invocation.json,
            "No command provided.",
            "Run lantern --help for usage.",
        )
    })?;

    if command != Command::Type && invocation.type_text_file.is_some() {
        return Err(CliError::usage(
            invocation.json,
            "--text-file is only supported by type.",
            "Run lantern type with exactly one of --text <TEXT> or --text-file <PATH>.",
        ));
    }

    if invocation.strict
        && !matches!(
            command,
            Command::ActionFlow
                | Command::Click
                | Command::Type
                | Command::Key
                | Command::Hover
                | Command::Wheel
                | Command::Drag
        )
    {
        return Err(CliError::usage(
            invocation.json,
            "--strict is only supported by interaction commands and action-flow.",
            "Use --strict with click, type, key, hover, wheel, drag or action-flow.",
        ));
    }

    if matches!(command, Command::Browser) {
        return run_browser_invocation(invocation).map(|()| true);
    }

    validate_endpoint_invocation(&invocation, command)?;
    if command == Command::Capabilities {
        return capabilities::write_capabilities().map(|()| true);
    }
    let mut invocation = invocation;
    if let Some(path) = invocation.type_text_file.as_deref() {
        invocation.wait_text = Some(read_type_text_file(path, invocation.json)?);
    }
    let endpoint = resolve_endpoint(invocation.endpoint.as_deref(), env_endpoint.as_deref())
        .map_err(|error| CliError::from_endpoint(error, invocation.json))?;
    run_command(command, invocation, endpoint)
}
