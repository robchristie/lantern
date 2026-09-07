use std::process::Command;

use serde_json::Value;

fn lantern(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_lantern"))
        .args(args)
        .env(
            "LANTERN_CDP_ENDPOINT",
            "invalid endpoint that must not be resolved",
        )
        .env("LANTERN_BUILD_COMMIT", "untrusted-runtime-override")
        .output()
        .unwrap()
}

#[test]
fn discovery_is_endpoint_independent_and_version_compatible() {
    let output = lantern(&["capabilities", "--json"]);
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["schema_version"], 1);
    assert_eq!(result["command"], "capabilities");
    assert_eq!(result["ok"], true);
    assert_eq!(result["package_version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(result["error_schema_versions"], serde_json::json!([1]));
    assert_eq!(output.stdout, lantern(&["capabilities"]).stdout);
    assert_eq!(
        output.stdout,
        lantern(&["--endpoint", "also invalid", "capabilities", "--json"]).stdout
    );
    for flag in ["--version", "-V"] {
        let version = lantern(&[flag]);
        assert!(version.status.success());
        assert_eq!(
            String::from_utf8(version.stdout).unwrap(),
            format!("lantern {}\n", env!("CARGO_PKG_VERSION"))
        );
    }
    match result["build"]["provenance"].as_str().unwrap() {
        "git" => {
            let commit = result["build"]["commit"].as_str().unwrap();
            assert!(matches!(commit.len(), 40 | 64));
            assert!(commit.bytes().all(|c| c.is_ascii_hexdigit()));
            assert!(result["build"]["dirty"].is_boolean());
        }
        "unknown" => {
            assert!(result["build"]["commit"].is_null());
            assert!(result["build"]["dirty"].is_null());
        }
        source => panic!("unsupported provenance: {source}"),
    }
}

#[test]
fn discovery_lists_the_existing_command_surface_and_schema_owners() {
    let output: Value = serde_json::from_slice(&lantern(&["capabilities"]).stdout).unwrap();
    let commands = output["commands"].as_array().unwrap();
    let names: Vec<_> = commands
        .iter()
        .map(|c| c["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        [
            "doctor",
            "targets",
            "page",
            "dom",
            "open",
            "wait",
            "console",
            "network",
            "screenshot",
            "layout",
            "click",
            "type",
            "key",
            "hover",
            "wheel",
            "drag",
            "flow",
            "browser",
            "capabilities"
        ]
    );
    let schema = |name: &str| &commands.iter().find(|c| c["name"] == name).unwrap()["output_schema_versions"];
    assert_eq!(
        schema("dom"),
        &serde_json::json!([lantern_core::dom::DOM_SCHEMA_VERSION])
    );
    assert_eq!(
        schema("flow"),
        &serde_json::json!([lantern_core::flow::FLOW_SCHEMA_VERSION])
    );
    assert_eq!(
        schema("click"),
        &serde_json::json!([lantern_core::interaction::INTERACTION_SCHEMA_VERSION])
    );
    let wait = commands.iter().find(|c| c["name"] == "wait").unwrap();
    assert_eq!(
        wait["subcommands"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["name"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["ready", "url", "selector", "text", "quiet"]
    );
    let drag = commands.iter().find(|c| c["name"] == "drag").unwrap();
    assert_eq!(drag["aliases"], serde_json::json!(["pointer-drag"]));
    let browser = commands.iter().find(|c| c["name"] == "browser").unwrap();
    assert_eq!(browser["output_schema_versions"], serde_json::json!([]));
    let subcommands = browser["subcommands"].as_array().unwrap();
    assert_eq!(
        subcommands
            .iter()
            .map(|c| c["name"].as_str().unwrap())
            .collect::<Vec<_>>(),
        [
            "start", "list", "status", "endpoint", "stop", "prune", "profile"
        ]
    );
    let profile = subcommands.iter().find(|c| c["name"] == "profile").unwrap();
    assert_eq!(
        profile["subcommands"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["name"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["create", "list", "status", "delete"]
    );
}

#[test]
fn discovery_rejects_execution_only_flags_without_endpoint_access() {
    for args in [
        vec!["--target-id", "page"],
        vec!["--timeout-ms", "100"],
        vec!["--output", "/tmp/unused"],
        vec!["--text-file", "/tmp/unused"],
        vec!["--runtime", "docker"],
    ] {
        let mut full = vec!["capabilities", "--json"];
        full.extend(args);
        let output = lantern(&full);
        assert_eq!(output.status.code(), Some(2));
        let error: Value = serde_json::from_slice(&output.stderr).unwrap();
        assert_eq!(error["error"]["code"], "usage");
    }
}
