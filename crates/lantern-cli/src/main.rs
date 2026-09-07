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

    if command != Command::Type && invocation.type_text_file.is_some() {
        return Err(CliError::usage(
            invocation.json,
            "--text-file is only supported by type.",
            "Run lantern type with exactly one of --text <TEXT> or --text-file <PATH>.",
        ));
    }

    if matches!(command, Command::Browser) {
        return run_browser_invocation(invocation);
    }

    validate_endpoint_invocation(&invocation, command)?;
    if command == Command::Capabilities {
        return capabilities::write_capabilities();
    }
    let mut invocation = invocation;
    if let Some(path) = invocation.type_text_file.as_deref() {
        invocation.wait_text = Some(read_type_text_file(path, invocation.json)?);
    }
    let endpoint = resolve_endpoint(invocation.endpoint.as_deref(), env_endpoint.as_deref())
        .map_err(|error| CliError::from_endpoint(error, invocation.json))?;
    run_command(command, invocation, endpoint)
}
