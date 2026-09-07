use super::*;
use crate::error::BROWSER_GRAPHICS_INVALID_MESSAGE;
use crate::registry::Command;
#[test]
fn parses_browser_start_with_runtime_image_and_id() {
    let invocation = Invocation::parse([
        "browser".to_string(),
        "start".to_string(),
        "--runtime".to_string(),
        "podman".to_string(),
        "--image".to_string(),
        "localhost/lantern-browser-cdp:test".to_string(),
        "--id".to_string(),
        "agent1".to_string(),
        "--wait-ms".to_string(),
        "1000".to_string(),
        "--host-gateway".to_string(),
        "LV426.YUTANI.TECH".to_string(),
        "--graphics".to_string(),
        "swiftshader".to_string(),
        "--json".to_string(),
    ])
    .expect("browser start should parse");

    assert_eq!(invocation.command, Some(Command::Browser));
    assert_eq!(invocation.browser_command, Some(BrowserCommand::Start));
    assert_eq!(invocation.browser_runtime, Some(RuntimeKind::Podman));
    assert_eq!(
        invocation.browser_image.as_deref(),
        Some("localhost/lantern-browser-cdp:test")
    );
    assert_eq!(invocation.browser_id.as_deref(), Some("agent1"));
    assert_eq!(invocation.browser_wait_ms, Some(1000));
    assert_eq!(
        invocation.browser_host_gateway.as_deref(),
        Some("lv426.yutani.tech")
    );
    assert_eq!(
        invocation.browser_graphics,
        Some(BrowserGraphicsMode::SwiftShader)
    );
    assert_eq!(invocation.browser_gpu_device, None);
    assert!(invocation.json);
}

#[test]
fn browser_host_gateway_is_start_only_and_rejects_non_hostnames() {
    let status = Invocation::parse([
        "browser".to_string(),
        "status".to_string(),
        "agent1".to_string(),
        "--host-gateway".to_string(),
        "lv426.yutani.tech".to_string(),
    ])
    .expect("status shape should parse before validation");
    assert_eq!(
        validate_browser_invocation(&status)
            .expect_err("host gateway must be start-only")
            .exit_code,
        2
    );

    for invalid in ["127.0.0.1", "http://lv426.yutani.tech", "bad_host.test"] {
        let start = Invocation::parse([
            "browser".to_string(),
            "start".to_string(),
            "--host-gateway".to_string(),
            invalid.to_string(),
        ])
        .expect("start shape should parse before validation");
        let error =
            validate_browser_invocation(&start).expect_err("invalid gateway hostname should fail");
        assert_eq!(error.message, BROWSER_HOST_GATEWAY_INVALID_MESSAGE);
    }
}

#[test]
fn parses_and_validates_explicit_hardware_webgpu_start() {
    let invocation = Invocation::parse([
        "browser".to_string(),
        "start".to_string(),
        "--graphics".to_string(),
        "webgpu".to_string(),
        "--gpu-device".to_string(),
        "nvidia.com/gpu=0".to_string(),
    ])
    .expect("hardware WebGPU start should parse");

    validate_browser_invocation(&invocation).expect("hardware WebGPU start should validate");
    assert_eq!(
        invocation.browser_graphics,
        Some(BrowserGraphicsMode::WebGpu)
    );
    assert_eq!(
        invocation.browser_gpu_device.as_deref(),
        Some("nvidia.com/gpu=0")
    );
}

#[test]
fn hardware_webgpu_requires_an_explicit_device() {
    let invocation = Invocation::parse([
        "browser".to_string(),
        "start".to_string(),
        "--graphics".to_string(),
        "webgpu".to_string(),
    ])
    .expect("hardware WebGPU start should parse before semantic validation");

    let error = validate_browser_invocation(&invocation)
        .expect_err("hardware WebGPU without a device should fail");

    assert_eq!(error.exit_code, 2);
    assert!(
        error
            .message
            .contains(BROWSER_WEBGPU_DEVICE_REQUIRED_MESSAGE)
    );
}

#[test]
fn gpu_device_is_rejected_for_non_hardware_modes() {
    let invocation = Invocation::parse([
        "browser".to_string(),
        "start".to_string(),
        "--graphics".to_string(),
        "swiftshader".to_string(),
        "--gpu-device".to_string(),
        "/dev/dri/renderD128".to_string(),
    ])
    .expect("device selector should parse before semantic validation");

    let error = validate_browser_invocation(&invocation)
        .expect_err("software graphics with a hardware device should fail");

    assert_eq!(error.exit_code, 2);
    assert!(error.message.contains(BROWSER_GPU_DEVICE_MODE_MESSAGE));
}

#[test]
fn gpu_device_rejects_runtime_option_injection() {
    let invocation = Invocation::parse([
        "browser".to_string(),
        "start".to_string(),
        "--graphics".to_string(),
        "gpu".to_string(),
        "--gpu-device".to_string(),
        "--privileged".to_string(),
    ])
    .expect("device selector should parse before semantic validation");

    let error =
        validate_browser_invocation(&invocation).expect_err("runtime option injection should fail");

    assert_eq!(error.exit_code, 2);
    assert!(error.message.contains(BROWSER_GPU_DEVICE_INVALID_MESSAGE));
}

#[test]
fn graphics_flags_are_start_only_and_profile_modes_are_explicit() {
    for flag in ["--graphics", "--gpu-device"] {
        for command in ["list", "status", "endpoint", "stop", "prune", "profile"] {
            let value = if flag == "--graphics" {
                "gpu"
            } else {
                "nvidia.com/gpu=0"
            };
            let invocation =
                Invocation::parse(["browser", command, flag, value].map(str::to_owned))
                    .expect("flag shape parses");
            assert!(validate_browser_invocation(&invocation).is_err());
        }
    }
    for mode in ["disabled", "swiftshader", "gpu"] {
        let invocation = Invocation::parse(
            [
                "browser",
                "start",
                "--profile",
                "review",
                "--graphics",
                mode,
            ]
            .map(str::to_owned),
        )
        .unwrap();
        validate_browser_invocation(&invocation)
            .expect("safe graphics modes accept named profiles");
    }
}

#[test]
fn instance_output_preserves_graphics_profile_and_gateway_metadata() {
    let mut record = BrowserInstanceRecord::pending(
        "instance".to_owned(),
        "instance".to_owned(),
        RuntimeKind::Podman,
        DEFAULT_BROWSER_IMAGE.to_owned(),
        "/tmp/profile".into(),
        BrowserProfileKind::Persistent,
        Some("review".to_owned()),
        Some("token".to_owned()),
    );
    record.graphics = BrowserGraphicsMode::Gpu;
    record.gpu_device = Some("nvidia.com/gpu=0".to_owned());
    record.host_gateway = Some("app.test".to_owned());
    let output = serde_json::to_value(BrowserInstanceOutput::from_record(record)).unwrap();
    assert_eq!(output["graphics"], "gpu");
    assert_eq!(output["gpu_device"], "nvidia.com/gpu=0");
    assert_eq!(output["unsafe_webgpu"], false);
    assert_eq!(output["profile_kind"], "persistent");
    assert_eq!(output["profile_name"], "review");
    assert_eq!(output["host_gateway"], "app.test");
}

#[test]
fn rejects_invalid_browser_graphics_mode() {
    let error = Invocation::parse([
        "browser".to_string(),
        "start".to_string(),
        "--graphics".to_string(),
        "raster-pixies".to_string(),
    ])
    .expect_err("invalid graphics mode should fail");

    assert_eq!(error.exit_code, 2);
    assert!(error.message.contains(BROWSER_GRAPHICS_INVALID_MESSAGE));
}

#[test]
fn parses_browser_status_positional_id() {
    let invocation = Invocation::parse([
        "browser".to_string(),
        "status".to_string(),
        "agent1".to_string(),
    ])
    .expect("browser status should parse");

    assert_eq!(invocation.command, Some(Command::Browser));
    assert_eq!(invocation.browser_command, Some(BrowserCommand::Status));
    assert_eq!(invocation.browser_id.as_deref(), Some("agent1"));
}

#[test]
fn parses_persistent_browser_start_with_named_profile() {
    let invocation = Invocation::parse([
        "browser".to_string(),
        "start".to_string(),
        "--profile".to_string(),
        "geometis-review".to_string(),
    ])
    .expect("persistent browser start should parse");

    assert_eq!(invocation.browser_command, Some(BrowserCommand::Start));
    assert_eq!(
        invocation.browser_profile_name.as_deref(),
        Some("geometis-review")
    );
    assert_eq!(invocation.browser_id, None);
    validate_browser_invocation(&invocation).expect("persistent start should validate");
}

#[test]
fn persistent_browser_id_cannot_be_overridden() {
    let invocation = Invocation::parse([
        "browser".to_string(),
        "start".to_string(),
        "--profile".to_string(),
        "geometis-review".to_string(),
        "--id".to_string(),
        "other-browser".to_string(),
    ])
    .expect("persistent browser start should parse");

    let error = validate_browser_invocation(&invocation)
        .expect_err("cross-profile instance identity must fail");
    assert_eq!(error.exit_code, 2);
    assert_eq!(
        error.message,
        "Persistent browser ids are derived from the profile and state home."
    );

    let disposable = Invocation::parse([
        "browser".to_string(),
        "start".to_string(),
        "--id".to_string(),
        "lantern-profile-geometis-review".to_string(),
    ])
    .expect("disposable browser start should parse");
    let error = validate_browser_invocation(&disposable)
        .expect_err("disposable start must not claim the persistent namespace");
    assert_eq!(
        error.message,
        "Disposable browser ids cannot use the persistent profile namespace."
    );
}

#[test]
fn parses_browser_profile_lifecycle_commands() {
    let create = Invocation::parse([
        "browser".to_string(),
        "profile".to_string(),
        "create".to_string(),
        "geometis-review".to_string(),
    ])
    .expect("profile create should parse");
    assert_eq!(create.browser_command, Some(BrowserCommand::Profile));
    assert_eq!(
        create.browser_profile_command,
        Some(BrowserProfileCommand::Create)
    );
    assert_eq!(
        create.browser_profile_name.as_deref(),
        Some("geometis-review")
    );
    validate_browser_invocation(&create).expect("profile create should validate");

    let list = Invocation::parse([
        "browser".to_string(),
        "profile".to_string(),
        "list".to_string(),
    ])
    .expect("profile list should parse");
    assert_eq!(
        list.browser_profile_command,
        Some(BrowserProfileCommand::List)
    );
    validate_browser_invocation(&list).expect("profile list should validate");

    let delete = Invocation::parse([
        "browser".to_string(),
        "profile".to_string(),
        "delete".to_string(),
        "geometis-review".to_string(),
        "--yes".to_string(),
    ])
    .expect("profile delete should parse");
    assert_eq!(
        delete.browser_profile_command,
        Some(BrowserProfileCommand::Delete)
    );
    assert!(delete.browser_confirm);
    validate_browser_invocation(&delete).expect("confirmed delete should validate");
}

#[test]
fn profile_delete_requires_confirmation_and_profile_names_are_bounded() {
    let delete = Invocation::parse([
        "browser".to_string(),
        "profile".to_string(),
        "delete".to_string(),
        "geometis-review".to_string(),
    ])
    .expect("profile delete should parse");
    validate_browser_invocation(&delete).expect("shape should validate before execution");
    let error = run_browser_profile_invocation(
        &BrowserProfileRegistry::new("/tmp/not-used"),
        &BrowserRegistry::new("/tmp/not-used-instances"),
        BrowserProfileCommand::Delete,
        Some("geometis-review".to_owned()),
        false,
        true,
    )
    .expect_err("unconfirmed deletion should fail before storage access");
    assert_eq!(error.exit_code, 2);
    assert_eq!(error.code, "usage");

    let invalid = Invocation::parse([
        "browser".to_string(),
        "start".to_string(),
        "--profile".to_string(),
        "../daily".to_string(),
    ])
    .expect("invalid name should parse before semantic validation");
    assert!(validate_browser_invocation(&invalid).is_err());
}

#[test]
fn resolves_persistent_state_home_outside_repository_state() {
    assert_eq!(
        resolve_lantern_state_home(Some("/var/lib/lantern-test"), None, None),
        Some(PathBuf::from("/var/lib/lantern-test"))
    );
    assert_eq!(
        resolve_lantern_state_home(None, Some("/var/state"), Some("/home/test")),
        Some(PathBuf::from("/var/state/lantern"))
    );
    assert_eq!(
        resolve_lantern_state_home(None, None, Some("/home/test")),
        Some(PathBuf::from("/home/test/.local/state/lantern"))
    );
    assert_eq!(
        resolve_lantern_state_home(Some("relative"), None, Some("/home/test")),
        None
    );
    assert_eq!(resolve_lantern_state_home(None, None, None), None);
}

#[test]
fn missing_profile_attachment_retains_a_bounded_starting_grace() {
    assert!(profile_attachment_is_within_starting_grace(1_000, 1_000));
    assert!(profile_attachment_is_within_starting_grace(
        1_000,
        1_000 + BROWSER_PROFILE_STARTING_GRACE_MS - 1
    ));
    assert!(!profile_attachment_is_within_starting_grace(
        1_000,
        1_000 + BROWSER_PROFILE_STARTING_GRACE_MS
    ));
    assert!(profile_attachment_is_within_starting_grace(2_000, 1_000));
}

#[test]
fn only_confirmed_inactive_runtime_states_allow_profile_release() {
    assert!(profile_status_allows_release(
        BrowserInstanceStatus::Stopped
    ));
    assert!(profile_status_allows_release(
        BrowserInstanceStatus::Missing
    ));
    assert!(!profile_status_allows_release(
        BrowserInstanceStatus::Starting
    ));
    assert!(!profile_status_allows_release(
        BrowserInstanceStatus::Running
    ));
    assert!(!profile_status_allows_release(BrowserInstanceStatus::Error));
    assert!(!profile_status_allows_prune(
        BrowserProfileKind::Persistent,
        BrowserInstanceStatus::Error
    ));
    assert!(profile_status_allows_prune(
        BrowserProfileKind::Disposable,
        BrowserInstanceStatus::Error
    ));
}

#[test]
fn stale_owner_does_not_claim_the_same_name_on_another_runtime() {
    let attachment = BrowserProfileAttachment {
        instance_id: "review-browser".to_owned(),
        container_name: "review-browser".to_owned(),
        runtime: RuntimeKind::Podman,
        reservation_id: "reservation-review".to_owned(),
        state: BrowserProfileAttachmentState::Active,
    };
    let record = BrowserInstanceRecord::pending(
        "review-browser".to_owned(),
        "review-browser".to_owned(),
        RuntimeKind::Podman,
        DEFAULT_BROWSER_IMAGE.to_owned(),
        PathBuf::from("/tmp/review-profile"),
        BrowserProfileKind::Persistent,
        Some("review".to_owned()),
        Some("reservation-review".to_owned()),
    );

    assert!(persistent_attachment_owns_runtime_target(
        Some(&attachment),
        RuntimeKind::Podman,
        "review-browser"
    ));
    assert!(persistent_record_owns_runtime_target(
        &record,
        RuntimeKind::Podman,
        "review-browser"
    ));
    assert!(!persistent_attachment_owns_runtime_target(
        Some(&attachment),
        RuntimeKind::Docker,
        "review-browser"
    ));
    assert!(!persistent_record_owns_runtime_target(
        &record,
        RuntimeKind::Docker,
        "review-browser"
    ));
}
#[test]
fn prune_preserves_a_fresh_persistent_starting_reservation() {
    let mut record = BrowserInstanceRecord::pending(
        "review-browser".to_owned(),
        "review-browser".to_owned(),
        RuntimeKind::Podman,
        DEFAULT_BROWSER_IMAGE.to_owned(),
        PathBuf::from("/tmp/review-profile"),
        BrowserProfileKind::Persistent,
        Some("review".to_owned()),
        Some("reservation-starting".to_owned()),
    );
    record.updated_at_unix_ms = 1_000;

    assert!(profile_record_has_fresh_starting_reservation(
        &record,
        1_000 + BROWSER_PROFILE_STARTING_GRACE_MS - 1
    ));
    assert!(!profile_record_has_fresh_starting_reservation(
        &record,
        1_000 + BROWSER_PROFILE_STARTING_GRACE_MS
    ));
    record.status = BrowserInstanceStatus::Stopped;
    assert!(!profile_record_has_fresh_starting_reservation(
        &record, 1_001
    ));
}
