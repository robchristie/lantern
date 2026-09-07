use super::*;

#[test]
fn parses_flag_endpoint_and_command_in_any_order() {
    let invocation = Invocation::parse([
        "--json".to_string(),
        "--endpoint".to_string(),
        "http://127.0.0.1:9222".to_string(),
        "doctor".to_string(),
    ])
    .expect("invocation should parse");

    assert_eq!(invocation.command, Some(Command::Doctor));
    assert_eq!(
        invocation.endpoint.as_deref(),
        Some("http://127.0.0.1:9222")
    );
    assert!(invocation.json);
    assert_eq!(invocation.target_id, None);
}

#[test]
fn parses_exact_target_id_selector() {
    let invocation = Invocation::parse([
        "page".to_string(),
        "--target-id".to_string(),
        "PAGE_123".to_string(),
    ])
    .expect("invocation should parse");

    assert_eq!(invocation.command, Some(Command::Page));
    assert_eq!(invocation.target_id.as_deref(), Some("PAGE_123"));
}

#[test]
fn parses_wait_condition_and_timeout_flags() {
    let invocation = Invocation::parse([
        "wait".to_string(),
        "text".to_string(),
        "--selector".to_string(),
        "main".to_string(),
        "--text".to_string(),
        "Ready".to_string(),
        "--timeout-ms".to_string(),
        "5000".to_string(),
    ])
    .expect("invocation should parse");

    assert_eq!(invocation.command, Some(Command::Wait));
    assert_eq!(invocation.wait_kind, Some(WaitConditionName::Text));
    assert_eq!(invocation.wait_selector.as_deref(), Some("main"));
    assert_eq!(invocation.wait_text.as_deref(), Some("Ready"));
    assert_eq!(invocation.timeout_ms, Some(5000));
}

#[test]
fn parses_dom_depth_and_node_limit_flags() {
    let invocation = Invocation::parse([
        "dom".to_string(),
        "--depth".to_string(),
        "8".to_string(),
        "--max-nodes".to_string(),
        "200".to_string(),
    ])
    .expect("invocation should parse");

    assert_eq!(invocation.command, Some(Command::Dom));
    assert_eq!(invocation.dom_depth, Some(8));
    assert_eq!(invocation.dom_max_nodes, Some(200));
}

#[test]
fn parses_layout_command_with_target_id() {
    let invocation = Invocation::parse([
        "layout".to_string(),
        "--target-id".to_string(),
        "PAGE_123".to_string(),
        "--json".to_string(),
    ])
    .expect("invocation should parse");

    assert_eq!(invocation.command, Some(Command::Layout));
    assert_eq!(invocation.target_id.as_deref(), Some("PAGE_123"));
    assert!(invocation.json);
}

#[test]
fn parses_screenshot_output_and_overwrite_flags() {
    let invocation = Invocation::parse([
        "screenshot".to_string(),
        "--output".to_string(),
        "artifacts/page.png".to_string(),
        "--overwrite".to_string(),
    ])
    .expect("invocation should parse");

    assert_eq!(invocation.command, Some(Command::Screenshot));
    assert_eq!(
        invocation.screenshot_output.as_deref(),
        Some("artifacts/page.png")
    );
    assert!(invocation.screenshot_overwrite);
}

#[test]
fn parses_flow_command_with_open_and_quiet_window() {
    let invocation = Invocation::parse([
        "flow".to_string(),
        "--open".to_string(),
        "https://example.test".to_string(),
        "--timeout-ms".to_string(),
        "5000".to_string(),
        "--quiet-ms".to_string(),
        "500".to_string(),
        "--target-id".to_string(),
        "PAGE_123".to_string(),
    ])
    .expect("invocation should parse");

    assert_eq!(invocation.command, Some(Command::Flow));
    assert_eq!(invocation.open_url.as_deref(), Some("https://example.test"));
    assert_eq!(invocation.timeout_ms, Some(5000));
    assert_eq!(invocation.quiet_ms, Some(500));
    assert_eq!(invocation.target_id.as_deref(), Some("PAGE_123"));
}

#[test]
fn parses_network_command() {
    let invocation = Invocation::parse([
        "network".to_string(),
        "--target-id".to_string(),
        "PAGE_123".to_string(),
    ])
    .expect("invocation should parse");

    assert_eq!(invocation.command, Some(Command::Network));
    assert_eq!(invocation.target_id.as_deref(), Some("PAGE_123"));
}
