use crate::args::Invocation;
use crate::error::CliError;
use crate::inspection::run_inspection;
use crate::interaction::run_interaction;
use crate::registry::Command;
use crate::screenshot::run_screenshot;
use lantern_core::cdp::CdpClient;
use lantern_core::endpoint::ResolvedEndpoint;
use std::time::Duration;

pub(crate) struct EndpointContext {
    pub(crate) command: Command,
    pub(crate) invocation: Invocation,
    pub(crate) client: CdpClient,
    pub(crate) endpoint: ResolvedEndpoint,
    pub(crate) budget: Option<lantern_core::cdp::OperationDeadline>,
}

pub(crate) fn run_command(
    command: Command,
    invocation: Invocation,
    endpoint: ResolvedEndpoint,
) -> Result<(), CliError> {
    let budget = invocation
        .timeout_ms
        .map(|ms| lantern_core::cdp::OperationDeadline::from(Duration::from_millis(ms)));
    let mut client = CdpClient::new(endpoint.clone());
    if let Some(budget) = budget {
        client = client.with_deadline(budget.end());
    }
    let context = EndpointContext {
        command,
        invocation,
        client,
        endpoint,
        budget,
    };
    match command {
        Command::Doctor
        | Command::Targets
        | Command::Page
        | Command::Dom
        | Command::Open
        | Command::Wait
        | Command::Console
        | Command::Network
        | Command::Flow
        | Command::Layout => run_inspection(context),
        Command::Click
        | Command::Type
        | Command::Key
        | Command::Hover
        | Command::Wheel
        | Command::Drag => run_interaction(context),
        Command::Screenshot => run_screenshot(context),
        Command::Browser | Command::Capabilities => {
            unreachable!("browser lifecycle commands bypass endpoint commands")
        }
    }
}
