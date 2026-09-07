//! Endpoint-independent discovery of this executable's supported CLI surface.
use serde::Serialize;

use crate::{
    error::CliError,
    output::{CLI_SCHEMA_VERSION, write_json},
    registry::{COMMANDS, CommandCapability},
};

#[derive(Serialize)]
struct CapabilitiesOutput {
    schema_version: u8,
    command: &'static str,
    ok: bool,
    package_version: &'static str,
    build: BuildIdentity,
    error_schema_versions: &'static [u8],
    commands: &'static [CommandCapability],
}

#[derive(Serialize)]
struct BuildIdentity {
    provenance: &'static str,
    commit: Option<&'static str>,
    dirty: Option<bool>,
}

pub(crate) fn write_capabilities() -> Result<(), CliError> {
    let commit = env!("LANTERN_BUILD_COMMIT");
    write_json(&CapabilitiesOutput {
        schema_version: CLI_SCHEMA_VERSION,
        command: "capabilities",
        ok: true,
        package_version: env!("CARGO_PKG_VERSION"),
        build: BuildIdentity {
            provenance: env!("LANTERN_BUILD_PROVENANCE"),
            commit: (!commit.is_empty()).then_some(commit),
            dirty: match env!("LANTERN_BUILD_DIRTY") {
                "true" => Some(true),
                "false" => Some(false),
                _ => None,
            },
        },
        error_schema_versions: &[CLI_SCHEMA_VERSION],
        commands: COMMANDS,
    })
}
