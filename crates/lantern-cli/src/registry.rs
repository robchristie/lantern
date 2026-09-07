//! The parser and discovery share command spellings, aliases and output schemas.
use lantern_core::{
    console::CONSOLE_SCHEMA_VERSION,
    dom::DOM_SCHEMA_VERSION,
    flow::FLOW_SCHEMA_VERSION,
    interaction::INTERACTION_SCHEMA_VERSION,
    layout::LAYOUT_SCHEMA_VERSION,
    navigation::NAVIGATION_SCHEMA_VERSION,
    network::NETWORK_SCHEMA_VERSION,
    screenshot::SCREENSHOT_SCHEMA_VERSION,
    wait::{WAIT_SCHEMA_VERSION, WaitConditionName},
};
use serde::Serialize;

use crate::output::CLI_SCHEMA_VERSION;

#[derive(Debug, Serialize)]
pub(crate) struct CommandCapability {
    pub(crate) name: &'static str,
    pub(crate) aliases: &'static [&'static str],
    pub(crate) output_schema_versions: &'static [u8],
    pub(crate) subcommands: &'static [CommandCapability],
}

// Define the enum and its spelling registry together so a new dispatch variant
// cannot silently disappear from discovery or the parser.
macro_rules! commands {
    ($enum:ident, $registry:ident, [$($variant:ident => ($name:literal, [$($alias:literal),*], $versions:expr, $children:expr)),* $(,)?]) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) enum $enum { $($variant),* }

        impl $enum {
            pub(crate) fn from_name(name: &str) -> Option<Self> {
                match name {
                    $($name $(| $alias)* => Some(Self::$variant),)*
                    _ => None,
                }
            }
        }

        pub(crate) const $registry: &[CommandCapability] = &[
            $(CommandCapability {
                name: $name,
                aliases: &[$($alias),*],
                output_schema_versions: $versions,
                subcommands: $children,
            }),*
        ];
    };
}

commands!(Command, COMMANDS, [
    Doctor => ("doctor", [], &[CLI_SCHEMA_VERSION], &[]),
    Targets => ("targets", [], &[CLI_SCHEMA_VERSION], &[]),
    Page => ("page", [], &[CLI_SCHEMA_VERSION], &[]),
    Dom => ("dom", [], &[DOM_SCHEMA_VERSION], &[]),
    Open => ("open", [], &[NAVIGATION_SCHEMA_VERSION], &[]),
    Wait => ("wait", [], &[WAIT_SCHEMA_VERSION], WAIT_COMMANDS),
    Console => ("console", [], &[CONSOLE_SCHEMA_VERSION], &[]),
    Network => ("network", [], &[NETWORK_SCHEMA_VERSION], &[]),
    Screenshot => ("screenshot", [], &[SCREENSHOT_SCHEMA_VERSION], &[]),
    Layout => ("layout", [], &[LAYOUT_SCHEMA_VERSION], &[]),
    Click => ("click", [], &[INTERACTION_SCHEMA_VERSION], &[]),
    Type => ("type", [], &[INTERACTION_SCHEMA_VERSION], &[]),
    Key => ("key", [], &[INTERACTION_SCHEMA_VERSION], &[]),
    Hover => ("hover", [], &[INTERACTION_SCHEMA_VERSION], &[]),
    Wheel => ("wheel", [], &[INTERACTION_SCHEMA_VERSION], &[]),
    Drag => ("drag", ["pointer-drag"], &[INTERACTION_SCHEMA_VERSION], &[]),
    Flow => ("flow", [], &[FLOW_SCHEMA_VERSION], &[]),
    Browser => ("browser", [], &[], BROWSER_COMMANDS),
    Capabilities => ("capabilities", [], &[CLI_SCHEMA_VERSION], &[]),
]);

commands!(BrowserCommand, BROWSER_COMMANDS, [
    Start => ("start", [], &[CLI_SCHEMA_VERSION], &[]),
    List => ("list", [], &[CLI_SCHEMA_VERSION], &[]),
    Status => ("status", [], &[CLI_SCHEMA_VERSION], &[]),
    Endpoint => ("endpoint", [], &[CLI_SCHEMA_VERSION], &[]),
    Stop => ("stop", [], &[CLI_SCHEMA_VERSION], &[]),
    Prune => ("prune", [], &[CLI_SCHEMA_VERSION], &[]),
    Profile => ("profile", [], &[], PROFILE_COMMANDS),
]);

commands!(BrowserProfileCommand, PROFILE_COMMANDS, [
    Create => ("create", [], &[CLI_SCHEMA_VERSION], &[]),
    List => ("list", [], &[CLI_SCHEMA_VERSION], &[]),
    Status => ("status", [], &[CLI_SCHEMA_VERSION], &[]),
    Delete => ("delete", [], &[CLI_SCHEMA_VERSION], &[]),
]);

macro_rules! wait_conditions {
    ($($variant:ident => $name:literal),* $(,)?) => {
        pub(crate) fn wait_condition_from_name(name: &str) -> Option<WaitConditionName> {
            match name { $($name => Some(WaitConditionName::$variant),)* _ => None }
        }
        pub(crate) const WAIT_COMMANDS: &[CommandCapability] = &[
            $(CommandCapability { name: $name, aliases: &[], output_schema_versions: &[WAIT_SCHEMA_VERSION], subcommands: &[] }),*
        ];
    };
}
wait_conditions!(Ready => "ready", Url => "url", Selector => "selector", Text => "text", Quiet => "quiet");

#[cfg(test)]
mod tests {
    use super::*;
    use crate::args::Invocation;
    use std::collections::HashSet;

    #[test]
    fn registry_names_and_aliases_parse_uniquely() {
        let mut seen = HashSet::new();
        for entry in COMMANDS {
            for name in std::iter::once(&entry.name).chain(entry.aliases) {
                assert!(seen.insert(name));
                let parsed = Invocation::parse([name.to_string()]).unwrap();
                assert_eq!(parsed.command, Command::from_name(entry.name));
            }
        }
        for entry in WAIT_COMMANDS {
            let parsed = Invocation::parse(["wait", entry.name].map(str::to_owned)).unwrap();
            assert_eq!(parsed.wait_kind, wait_condition_from_name(entry.name));
        }
        for entry in BROWSER_COMMANDS {
            let parsed = Invocation::parse(["browser", entry.name].map(str::to_owned)).unwrap();
            assert_eq!(
                parsed.browser_command,
                BrowserCommand::from_name(entry.name)
            );
        }
        for entry in PROFILE_COMMANDS {
            let parsed =
                Invocation::parse(["browser", "profile", entry.name].map(str::to_owned)).unwrap();
            assert_eq!(
                parsed.browser_profile_command,
                BrowserProfileCommand::from_name(entry.name)
            );
        }
    }
}
