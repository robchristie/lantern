pub(crate) const CLI_SCHEMA_VERSION: u8 = 1;

use crate::error::{CDP_RESPONSE_INVALID_HINT, CDP_RESPONSE_INVALID_MESSAGE, CliError};
use serde::Serialize;

pub(crate) fn write_json<T: Serialize>(value: &T) -> Result<(), CliError> {
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

pub(crate) fn escape_human(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}
