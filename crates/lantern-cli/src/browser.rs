use crate::args::Invocation;
use crate::error::{
    BROWSER_GPU_DEVICE_INVALID_HINT, BROWSER_GPU_DEVICE_INVALID_MESSAGE,
    BROWSER_GPU_DEVICE_MODE_HINT, BROWSER_GPU_DEVICE_MODE_MESSAGE,
    BROWSER_HOST_GATEWAY_INVALID_HINT, BROWSER_HOST_GATEWAY_INVALID_MESSAGE,
    BROWSER_ID_MISSING_HINT, BROWSER_ID_MISSING_MESSAGE, BROWSER_NOT_READY_HINT,
    BROWSER_NOT_READY_MESSAGE, BROWSER_PORT_MISSING_HINT, BROWSER_PORT_MISSING_MESSAGE,
    BROWSER_PROFILE_DELETE_CONFIRM_HINT, BROWSER_PROFILE_DELETE_CONFIRM_MESSAGE,
    BROWSER_PROFILE_EXISTS_HINT, BROWSER_PROFILE_EXISTS_MESSAGE, BROWSER_PROFILE_IN_USE_HINT,
    BROWSER_PROFILE_IN_USE_MESSAGE, BROWSER_PROFILE_MISSING_HINT, BROWSER_PROFILE_MISSING_MESSAGE,
    BROWSER_PROFILE_NOT_FOUND_HINT, BROWSER_PROFILE_NOT_FOUND_MESSAGE, BROWSER_RUNTIME_FAILED_HINT,
    BROWSER_RUNTIME_FAILED_MESSAGE, BROWSER_RUNTIME_UNAVAILABLE_HINT,
    BROWSER_RUNTIME_UNAVAILABLE_MESSAGE, BROWSER_STATE_FAILED_HINT, BROWSER_STATE_FAILED_MESSAGE,
    BROWSER_STATE_HOME_INVALID_HINT, BROWSER_STATE_HOME_INVALID_MESSAGE, BROWSER_USAGE_HINT,
    BROWSER_WEBGPU_DEVICE_REQUIRED_HINT, BROWSER_WEBGPU_DEVICE_REQUIRED_MESSAGE, CliError,
};
use crate::output::{CLI_SCHEMA_VERSION, write_json};
use crate::registry::{BrowserCommand, BrowserProfileCommand};
use lantern_core::cdp::{CdpClient, CdpError};
use lantern_core::endpoint::ResolvedEndpoint;
use lantern_storage::{
    BrowserGraphicsMode, BrowserInstanceRecord, BrowserInstanceStatus, BrowserProfileAttachment,
    BrowserProfileAttachmentState, BrowserProfileKind, BrowserProfileRecord,
    BrowserProfileRegistry, BrowserRegistry, BrowserRunSpec, DEFAULT_BROWSER_IMAGE,
    HostGatewayHostname, PERSISTENT_INSTANCE_ID_PREFIX, RuntimeCommand, RuntimeKind,
    browser_inspect_status_command, browser_port_command, browser_ps_all_command,
    browser_ps_managed_command, browser_rm_command, browser_run_command, browser_stop_command,
    generate_instance_id, generate_profile_reservation_id, instance_name, parse_published_port,
    parse_runtime_status, validate_host_gateway_hostname, validate_profile_name,
};
use serde::Serialize;
use std::collections::HashSet;
use std::env;
use std::io::ErrorKind;
use std::path::PathBuf;
use std::process::Command as ProcessCommand;
use std::time::{Duration, Instant};

const BROWSER_PROFILE_STARTING_GRACE_MS: u128 = 60_000;

pub(crate) fn run_browser_invocation(invocation: Invocation) -> Result<(), CliError> {
    validate_browser_invocation(&invocation)?;
    let command = invocation
        .browser_command
        .expect("browser subcommand checked before run");
    let repo_root = repo_root().map_err(|_| {
        CliError::runtime(
            invocation.json,
            "browser_state_failed",
            BROWSER_STATE_FAILED_MESSAGE,
            BROWSER_STATE_FAILED_HINT,
        )
    })?;
    let disposable_registry = BrowserRegistry::under_repo(repo_root);
    let persistence_required = command == BrowserCommand::Profile
        || (command == BrowserCommand::Start && invocation.browser_profile_name.is_some());
    let state_home = if persistence_required {
        Some(lantern_state_home(invocation.json)?)
    } else {
        optional_lantern_state_home(invocation.json)?
    };
    let persistent_state = state_home.map(|state_home| {
        (
            BrowserRegistry::new(state_home.join("browser-instances")),
            BrowserProfileRegistry::new(state_home.join("browser-profiles")),
        )
    });
    let persistent_registry = persistent_state.as_ref().map(|state| &state.0);
    let profile_registry = persistent_state.as_ref().map(|state| &state.1);

    match command {
        BrowserCommand::Start => browser_start(
            &disposable_registry,
            persistent_registry,
            profile_registry,
            invocation.browser_runtime,
            invocation
                .browser_image
                .unwrap_or_else(|| DEFAULT_BROWSER_IMAGE.to_owned()),
            invocation.browser_id,
            invocation.browser_wait_ms.unwrap_or(15_000),
            invocation.browser_profile_name,
            invocation.browser_host_gateway,
            invocation
                .browser_graphics
                .unwrap_or(BrowserGraphicsMode::Disabled),
            invocation.browser_gpu_device,
            invocation.json,
        ),
        BrowserCommand::List => {
            browser_list(&disposable_registry, persistent_registry, invocation.json)
        }
        BrowserCommand::Status => {
            let id = required_browser_id(invocation.browser_id, invocation.json)?;
            browser_status(
                &disposable_registry,
                persistent_registry,
                profile_registry,
                &id,
                invocation.json,
            )
        }
        BrowserCommand::Endpoint => {
            let id = required_browser_id(invocation.browser_id, invocation.json)?;
            browser_endpoint(
                &disposable_registry,
                persistent_registry,
                &id,
                invocation.json,
            )
        }
        BrowserCommand::Stop => {
            let id = required_browser_id(invocation.browser_id, invocation.json)?;
            browser_stop(
                &disposable_registry,
                persistent_registry,
                profile_registry,
                &id,
                invocation.json,
            )
        }
        BrowserCommand::Prune => browser_prune(
            &disposable_registry,
            persistent_registry,
            profile_registry,
            invocation.json,
        ),
        BrowserCommand::Profile => run_browser_profile_invocation(
            profile_registry.expect("profile state resolved for profile command"),
            persistent_registry.expect("persistent state resolved for profile command"),
            invocation
                .browser_profile_command
                .expect("profile subcommand checked before run"),
            invocation.browser_profile_name,
            invocation.browser_confirm,
            invocation.json,
        ),
    }
}

fn validate_browser_invocation(invocation: &Invocation) -> Result<(), CliError> {
    let command = invocation.browser_command.ok_or_else(|| {
        CliError::usage(
            invocation.json,
            "Missing browser subcommand.",
            BROWSER_USAGE_HINT,
        )
    })?;

    if invocation.endpoint.is_some()
        || invocation.no_redact
        || invocation.target_id.is_some()
        || invocation.open_url.is_some()
        || invocation.has_wait_only_flags()
        || invocation.wait_selector.is_some()
        || invocation.wait_text.is_some()
        || invocation.key.is_some()
        || invocation.delta_x.is_some()
        || invocation.delta_y.is_some()
        || invocation.duration_ms.is_some()
        || invocation.timeout_ms.is_some()
        || invocation.has_screenshot_flags()
        || invocation.has_dom_flags()
    {
        return Err(CliError::usage(
            invocation.json,
            "Unsupported flag for browser command.",
            BROWSER_USAGE_HINT,
        ));
    }

    if !matches!(command, BrowserCommand::Start)
        && (invocation.browser_runtime.is_some()
            || invocation.browser_image.is_some()
            || invocation.browser_wait_ms.is_some()
            || invocation.browser_host_gateway.is_some()
            || invocation.browser_graphics.is_some()
            || invocation.browser_gpu_device.is_some())
    {
        return Err(CliError::usage(
            invocation.json,
            "Start-only browser flag was used with another browser subcommand.",
            "Use --runtime, --image, --wait-ms, --graphics, --gpu-device, and --host-gateway only with lantern browser start.",
        ));
    }

    if command == BrowserCommand::Start {
        if invocation.browser_graphics == Some(BrowserGraphicsMode::WebGpu)
            && invocation.browser_profile_name.is_some()
        {
            return Err(CliError::usage(
                invocation.json,
                "Hardware WebGPU requires a disposable profile.",
                "Remove --profile for trusted-site WebGPU, or use disabled, swiftshader, or gpu with a named profile.",
            ));
        }
        let graphics = invocation
            .browser_graphics
            .unwrap_or(BrowserGraphicsMode::Disabled);
        if let Some(device) = invocation.browser_gpu_device.as_deref() {
            if device.is_empty()
                || device != device.trim()
                || device.starts_with('-')
                || device.len() > 256
                || device.chars().any(char::is_control)
            {
                return Err(CliError::usage(
                    invocation.json,
                    BROWSER_GPU_DEVICE_INVALID_MESSAGE,
                    BROWSER_GPU_DEVICE_INVALID_HINT,
                ));
            }
            if !matches!(
                graphics,
                BrowserGraphicsMode::Gpu | BrowserGraphicsMode::WebGpu
            ) {
                return Err(CliError::usage(
                    invocation.json,
                    BROWSER_GPU_DEVICE_MODE_MESSAGE,
                    BROWSER_GPU_DEVICE_MODE_HINT,
                ));
            }
        }
        if graphics == BrowserGraphicsMode::WebGpu && invocation.browser_gpu_device.is_none() {
            return Err(CliError::usage(
                invocation.json,
                BROWSER_WEBGPU_DEVICE_REQUIRED_MESSAGE,
                BROWSER_WEBGPU_DEVICE_REQUIRED_HINT,
            ));
        }
    }

    if command == BrowserCommand::List && invocation.browser_id.is_some() {
        return Err(CliError::usage(
            invocation.json,
            "Browser list does not accept an instance id.",
            "Run lantern browser list.",
        ));
    }

    if command == BrowserCommand::Prune && invocation.browser_id.is_some() {
        return Err(CliError::usage(
            invocation.json,
            "Browser prune does not accept an instance id.",
            "Run lantern browser prune.",
        ));
    }

    if command == BrowserCommand::Start {
        if let Some(hostname) = invocation.browser_host_gateway.as_deref() {
            validate_browser_host_gateway(hostname, invocation.json)?;
        }
        if let Some(name) = invocation.browser_profile_name.as_deref() {
            validate_browser_profile_name(name, invocation.json)?;
            if invocation.browser_id.is_some() {
                return Err(CliError::usage(
                    invocation.json,
                    "Persistent browser ids are derived from the profile and state home.",
                    "Omit --id when using --profile.",
                ));
            }
        } else if invocation
            .browser_id
            .as_deref()
            .is_some_and(|browser_id| browser_id.starts_with(PERSISTENT_INSTANCE_ID_PREFIX))
        {
            return Err(CliError::usage(
                invocation.json,
                "Disposable browser ids cannot use the persistent profile namespace.",
                "Choose an id that does not start with lantern-profile-.",
            ));
        }
        if invocation.browser_profile_command.is_some() || invocation.browser_confirm {
            return Err(CliError::usage(
                invocation.json,
                "Profile command flag was used with browser start.",
                "Use --profile NAME with start; use --yes only with browser profile delete.",
            ));
        }
    } else if command != BrowserCommand::Profile && invocation.browser_profile_name.is_some() {
        return Err(CliError::usage(
            invocation.json,
            "--profile is only supported by browser start.",
            "Run lantern browser start --profile NAME.",
        ));
    }

    if command == BrowserCommand::Profile {
        let profile_command = invocation.browser_profile_command.ok_or_else(|| {
            CliError::usage(
                invocation.json,
                "Missing browser profile subcommand.",
                "Run lantern browser profile <create|list|status|delete>.",
            )
        })?;
        if invocation.browser_id.is_some() {
            return Err(CliError::usage(
                invocation.json,
                "Browser profile commands do not accept an instance id.",
                "Pass the profile name positionally after create, status, or delete.",
            ));
        }
        let requires_name = !matches!(profile_command, BrowserProfileCommand::List);
        if requires_name && invocation.browser_profile_name.is_none() {
            return Err(CliError::usage(
                invocation.json,
                BROWSER_PROFILE_MISSING_MESSAGE,
                BROWSER_PROFILE_MISSING_HINT,
            ));
        }
        if !requires_name && invocation.browser_profile_name.is_some() {
            return Err(CliError::usage(
                invocation.json,
                "Profile list does not accept a profile name.",
                "Run lantern browser profile list.",
            ));
        }
        if let Some(name) = invocation.browser_profile_name.as_deref() {
            validate_browser_profile_name(name, invocation.json)?;
        }
        if invocation.browser_confirm && profile_command != BrowserProfileCommand::Delete {
            return Err(CliError::usage(
                invocation.json,
                "--yes is only supported by browser profile delete.",
                "Run lantern browser profile delete NAME --yes.",
            ));
        }
    } else if invocation.browser_confirm {
        return Err(CliError::usage(
            invocation.json,
            "--yes is only supported by browser profile delete.",
            "Run lantern browser profile delete NAME --yes.",
        ));
    }

    Ok(())
}

fn browser_start(
    disposable_registry: &BrowserRegistry,
    persistent_registry: Option<&BrowserRegistry>,
    profile_registry: Option<&BrowserProfileRegistry>,
    requested_runtime: Option<RuntimeKind>,
    image: String,
    requested_id: Option<String>,
    wait_ms: u64,
    profile_name: Option<String>,
    host_gateway: Option<String>,
    graphics: BrowserGraphicsMode,
    gpu_device: Option<String>,
    json: bool,
) -> Result<(), CliError> {
    let runtime = select_runtime(requested_runtime, json)?;
    let id = if let Some(profile_name) = profile_name.as_deref() {
        debug_assert!(requested_id.is_none());
        profile_registry
            .expect("profile state resolved for named profile start")
            .persistent_instance_id(profile_name)
            .map_err(|error| browser_profile_store_error(error, json))?
    } else {
        requested_id.unwrap_or_else(generate_instance_id)
    };
    validate_browser_id(&id, json)?;
    let name = instance_name(&id);
    let mut profile_reservation_id = None;
    let mut _profile_operation_lock = None;
    let (registry, layout, profile_kind) = if let Some(profile_name) = profile_name.as_deref() {
        let persistent_registry =
            persistent_registry.expect("persistent state resolved for named profile start");
        let profile_registry =
            profile_registry.expect("profile state resolved for named profile start");
        _profile_operation_lock = Some(
            profile_registry
                .try_operation_lock(profile_name)
                .map_err(|error| browser_profile_store_error(error, json))?,
        );
        let profile = profile_registry
            .read(profile_name)
            .map_err(|error| browser_profile_store_error(error, json))?;
        let expected = profile.attachment.clone();
        let mut selected_target_owned =
            persistent_attachment_owns_runtime_target(expected.as_ref(), runtime, &name);
        let mut stale_containers = Vec::new();
        if let Some(attachment) = expected.as_ref() {
            if attachment.state == BrowserProfileAttachmentState::Stopping {
                return Err(CliError::runtime(
                    json,
                    "browser_profile_in_use",
                    BROWSER_PROFILE_IN_USE_MESSAGE,
                    BROWSER_PROFILE_IN_USE_HINT,
                ));
            }
            let status =
                managed_runtime_status(attachment.runtime, &attachment.container_name, json)?;
            if matches!(
                status,
                BrowserInstanceStatus::Starting
                    | BrowserInstanceStatus::Running
                    | BrowserInstanceStatus::Error
            ) {
                return Err(CliError::runtime(
                    json,
                    "browser_profile_in_use",
                    BROWSER_PROFILE_IN_USE_MESSAGE,
                    BROWSER_PROFILE_IN_USE_HINT,
                ));
            }
            if status == BrowserInstanceStatus::Missing
                && profile_attachment_is_within_starting_grace(
                    profile.updated_at_unix_ms,
                    lantern_storage::now_unix_ms(),
                )
            {
                return Err(CliError::runtime(
                    json,
                    "browser_profile_in_use",
                    BROWSER_PROFILE_IN_USE_MESSAGE,
                    BROWSER_PROFILE_IN_USE_HINT,
                ));
            }
            if status != BrowserInstanceStatus::Missing {
                stale_containers.push((attachment.runtime, attachment.container_name.clone()));
            }
        }

        match persistent_registry.read_record(&id) {
            Ok(existing) => {
                if existing.profile_name.as_deref() != Some(profile_name) {
                    return Err(CliError::runtime(
                        json,
                        "browser_state_failed",
                        BROWSER_STATE_FAILED_MESSAGE,
                        BROWSER_STATE_FAILED_HINT,
                    ));
                }
                selected_target_owned |=
                    persistent_record_owns_runtime_target(&existing, runtime, &name);
                let status = managed_runtime_status(existing.runtime, &existing.name, json)?;
                if matches!(
                    status,
                    BrowserInstanceStatus::Starting
                        | BrowserInstanceStatus::Running
                        | BrowserInstanceStatus::Error
                ) {
                    return Err(CliError::runtime(
                        json,
                        "browser_profile_in_use",
                        BROWSER_PROFILE_IN_USE_MESSAGE,
                        BROWSER_PROFILE_IN_USE_HINT,
                    ));
                }
                if status != BrowserInstanceStatus::Missing
                    && !stale_containers
                        .iter()
                        .any(|(_, name)| name == &existing.name)
                {
                    stale_containers.push((existing.runtime, existing.name));
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(browser_state_error(json)),
        }

        match disposable_registry.read_record(&id) {
            Ok(_) => {
                return Err(CliError::runtime(
                    json,
                    "browser_profile_in_use",
                    BROWSER_PROFILE_IN_USE_MESSAGE,
                    BROWSER_PROFILE_IN_USE_HINT,
                ));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(browser_state_error(json)),
        }
        if !selected_target_owned
            && managed_runtime_status(runtime, &name, json)? != BrowserInstanceStatus::Missing
        {
            return Err(CliError::runtime(
                json,
                "browser_profile_in_use",
                BROWSER_PROFILE_IN_USE_MESSAGE,
                BROWSER_PROFILE_IN_USE_HINT,
            ));
        }

        let profile_dir = profile_registry
            .data_dir(profile_name)
            .map_err(|error| browser_profile_store_error(error, json))?;
        let layout = persistent_registry
            .create_persistent_instance_layout(&id, profile_dir)
            .map_err(|_| browser_state_error(json))?;
        let reservation_id = generate_profile_reservation_id();
        let attachment = BrowserProfileAttachment {
            instance_id: id.clone(),
            container_name: name.clone(),
            runtime,
            reservation_id: reservation_id.clone(),
            state: BrowserProfileAttachmentState::Active,
        };
        profile_registry
            .compare_and_swap_attachment(profile_name, expected.as_ref(), Some(attachment.clone()))
            .map_err(|error| browser_profile_store_error(error, json))?;
        profile_reservation_id = Some(reservation_id);
        for (stale_runtime, stale_name) in stale_containers {
            if let Err(error) =
                run_runtime_command(browser_rm_command(stale_runtime, &stale_name), json)
            {
                let _ = profile_registry.release_if_owner(profile_name, &attachment);
                return Err(error);
            }
        }
        (persistent_registry, layout, BrowserProfileKind::Persistent)
    } else {
        let layout = disposable_registry
            .create_instance_layout(&id)
            .map_err(|_| browser_state_error(json))?;
        (disposable_registry, layout, BrowserProfileKind::Disposable)
    };

    let mut record = BrowserInstanceRecord::pending(
        id.clone(),
        name.clone(),
        runtime,
        image.clone(),
        layout.profile_dir.clone(),
        profile_kind,
        profile_name.clone(),
        profile_reservation_id.clone(),
    );
    record.graphics = graphics;
    record.gpu_device = gpu_device.clone();
    record.host_gateway = host_gateway.clone();
    let runtime_host_gateway = host_gateway
        .map(HostGatewayHostname::parse)
        .transpose()
        .map_err(|_| {
            CliError::usage(
                json,
                BROWSER_HOST_GATEWAY_INVALID_MESSAGE,
                BROWSER_HOST_GATEWAY_INVALID_HINT,
            )
        })?;
    let mut runtime_attempted = false;
    let start_result = (|| {
        registry
            .write_record_atomic(&record)
            .map_err(|_| browser_state_error(json))?;
        let spec = BrowserRunSpec {
            id: id.clone(),
            name: name.clone(),
            image,
            profile_dir: layout.profile_dir,
            profile_name: profile_name.clone(),
            host_gateway: runtime_host_gateway,
            graphics,
            gpu_device,
        };
        runtime_attempted = true;
        let container_id = run_runtime_command(browser_run_command(runtime, &spec), json)?;
        let cdp_port = runtime_port(runtime, &name, 9222, json)?;
        let vnc_port = runtime_port(runtime, &name, 5900, json).ok();
        let novnc_port = runtime_port(runtime, &name, 6080, json).ok();
        let endpoint = format!("http://127.0.0.1:{cdp_port}");
        let novnc_url = novnc_port.map(|port| format!("http://127.0.0.1:{port}/vnc.html"));

        wait_for_browser_ready(&endpoint, Duration::from_millis(wait_ms), json)?;

        record.container_id = Some(container_id.trim().to_owned());
        record.status = BrowserInstanceStatus::Running;
        record.endpoint = Some(endpoint);
        record.cdp_host_port = Some(cdp_port);
        record.vnc_host_port = vnc_port;
        record.novnc_host_port = novnc_port;
        record.novnc_url = novnc_url;
        record.updated_at_unix_ms = lantern_storage::now_unix_ms();
        registry
            .write_record_atomic(&record)
            .map_err(|_| browser_state_error(json))?;

        write_browser_output("browser_start", record, json)
    })();

    if start_result.is_err() {
        if let Some(profile_name) = profile_name.as_deref() {
            let profile_registry =
                profile_registry.expect("profile state resolved for named profile cleanup");
            let safe_to_release = if runtime_attempted {
                let removed =
                    run_runtime_command(browser_rm_command(runtime, &name), false).is_ok();
                removed
                    || managed_runtime_status(runtime, &name, false)
                        .map(profile_status_allows_release)
                        .unwrap_or(false)
            } else {
                true
            };
            if safe_to_release {
                if let Some(reservation_id) = profile_reservation_id.as_deref() {
                    let owner = BrowserProfileAttachment {
                        instance_id: id.clone(),
                        container_name: name.clone(),
                        runtime,
                        reservation_id: reservation_id.to_owned(),
                        state: BrowserProfileAttachmentState::Active,
                    };
                    let _ = profile_registry.release_if_owner(profile_name, &owner);
                }
            }
        }
    }
    start_result
}

fn browser_list(
    disposable_registry: &BrowserRegistry,
    persistent_registry: Option<&BrowserRegistry>,
    json: bool,
) -> Result<(), CliError> {
    let mut records = disposable_registry
        .list_records()
        .map_err(|_| browser_state_error(json))?;
    if let Some(persistent_registry) = persistent_registry {
        records.extend(
            persistent_registry
                .list_records()
                .map_err(|_| browser_state_error(json))?,
        );
    }
    records.sort_by(|left, right| left.id.cmp(&right.id));
    if records
        .windows(2)
        .any(|records| records[0].id == records[1].id)
    {
        return Err(browser_state_error(json));
    }
    reconcile_labeled_runtime_instances(&mut records);

    if json {
        write_json(&BrowserListOutput {
            schema_version: CLI_SCHEMA_VERSION,
            command: "browser_list",
            ok: true,
            instances: records
                .into_iter()
                .map(BrowserInstanceOutput::from_record)
                .collect(),
        })?;
        return Ok(());
    }

    for record in records {
        println!(
            "{} status={} runtime={} graphics={} gpu_device={} unsafe_webgpu={} endpoint={} profile_kind={} profile_name={} host_gateway={} profile={}",
            record.id,
            record.status.as_str(),
            record.runtime.as_str(),
            record.graphics.as_str(),
            record.gpu_device.as_deref().unwrap_or("null"),
            record.graphics == BrowserGraphicsMode::WebGpu,
            record.endpoint.as_deref().unwrap_or("null"),
            record.profile_kind.as_str(),
            record.profile_name.as_deref().unwrap_or("null"),
            record.host_gateway.as_deref().unwrap_or("null"),
            record.profile_dir.display()
        );
    }

    Ok(())
}

fn reconcile_labeled_runtime_instances(records: &mut Vec<BrowserInstanceRecord>) {
    let mut known: HashSet<String> = records.iter().map(|record| record.name.clone()).collect();

    for runtime in [RuntimeKind::Podman, RuntimeKind::Docker] {
        if !runtime_available(runtime) {
            continue;
        }
        let Ok(names) = run_runtime_command(browser_ps_managed_command(runtime), false) else {
            continue;
        };

        for name in names.lines().map(str::trim).filter(|name| !name.is_empty()) {
            if known.contains(name) {
                continue;
            }
            let status = run_runtime_command(browser_inspect_status_command(runtime, name), false)
                .map(|status| parse_runtime_status(&status))
                .unwrap_or(BrowserInstanceStatus::Missing);
            let now = lantern_storage::now_unix_ms();
            records.push(BrowserInstanceRecord {
                schema_version: CLI_SCHEMA_VERSION,
                id: name.to_owned(),
                name: name.to_owned(),
                runtime,
                image: String::new(),
                graphics: BrowserGraphicsMode::Disabled,
                gpu_device: None,
                container_id: None,
                status,
                endpoint: None,
                cdp_host_port: None,
                novnc_url: None,
                novnc_host_port: None,
                vnc_host_port: None,
                profile_dir: PathBuf::new(),
                profile_kind: BrowserProfileKind::Disposable,
                profile_name: None,
                profile_reservation_id: None,
                host_gateway: None,
                created_at_unix_ms: now,
                updated_at_unix_ms: now,
            });
            known.insert(name.to_owned());
        }
    }

    records.sort_by(|left, right| left.id.cmp(&right.id));
}

fn browser_status(
    disposable_registry: &BrowserRegistry,
    persistent_registry: Option<&BrowserRegistry>,
    profile_registry: Option<&BrowserProfileRegistry>,
    id: &str,
    json: bool,
) -> Result<(), CliError> {
    let (registry, mut record) =
        find_browser_instance(disposable_registry, persistent_registry, id, json)?;
    let mut _profile_operation_lock = None;

    if record.profile_kind == BrowserProfileKind::Persistent {
        let profile_name = record
            .profile_name
            .as_deref()
            .expect("persistent record identity validated by storage")
            .to_owned();
        let profile_registry = profile_registry.ok_or_else(|| browser_state_error(json))?;
        _profile_operation_lock = Some(
            profile_registry
                .try_operation_lock(&profile_name)
                .map_err(|error| browser_profile_store_error(error, json))?,
        );
        record = registry
            .read_record(id)
            .map_err(|_| browser_state_error(json))?;
        if record.profile_name.as_deref() != Some(profile_name.as_str()) {
            return Err(browser_state_error(json));
        }
        record.status = managed_runtime_status(record.runtime, &record.name, json)?;
    } else if let Ok(status) = run_runtime_command(
        browser_inspect_status_command(record.runtime, &record.name),
        json,
    ) {
        record.status = parse_runtime_status(&status);
    } else {
        record.status = BrowserInstanceStatus::Missing;
    }

    registry.write_record_atomic(&record).map_err(|_| {
        CliError::runtime(
            json,
            "browser_state_failed",
            BROWSER_STATE_FAILED_MESSAGE,
            BROWSER_STATE_FAILED_HINT,
        )
    })?;

    write_browser_output("browser_status", record, json)
}

fn browser_endpoint(
    disposable_registry: &BrowserRegistry,
    persistent_registry: Option<&BrowserRegistry>,
    id: &str,
    json: bool,
) -> Result<(), CliError> {
    let (_, record) = find_browser_instance(disposable_registry, persistent_registry, id, json)?;
    write_browser_output("browser_endpoint", record, json)
}

fn browser_stop(
    disposable_registry: &BrowserRegistry,
    persistent_registry: Option<&BrowserRegistry>,
    profile_registry: Option<&BrowserProfileRegistry>,
    id: &str,
    json: bool,
) -> Result<(), CliError> {
    let (registry, mut record) =
        find_browser_instance(disposable_registry, persistent_registry, id, json)?;
    let mut _profile_operation_lock = None;

    let persistent_owner = if record.profile_kind == BrowserProfileKind::Persistent {
        let profile_registry = profile_registry.ok_or_else(|| browser_state_error(json))?;
        let profile_name = record
            .profile_name
            .as_deref()
            .expect("persistent record identity validated by storage")
            .to_owned();
        _profile_operation_lock = Some(
            profile_registry
                .try_operation_lock(&profile_name)
                .map_err(|error| browser_profile_store_error(error, json))?,
        );
        record = registry
            .read_record(id)
            .map_err(|_| browser_state_error(json))?;
        if record.profile_name.as_deref() != Some(profile_name.as_str()) {
            return Err(browser_state_error(json));
        }
        let expected_active = profile_attachment_from_record(&record, json)?;
        let profile = profile_registry
            .read(&profile_name)
            .map_err(|error| browser_profile_store_error(error, json))?;
        let stopping_owner = match profile.attachment.as_ref() {
            Some(current) if current == &expected_active => {
                let mut stopping = current.clone();
                stopping.state = BrowserProfileAttachmentState::Stopping;
                profile_registry
                    .compare_and_swap_attachment(
                        &profile_name,
                        Some(current),
                        Some(stopping.clone()),
                    )
                    .map_err(|error| browser_profile_store_error(error, json))?;
                stopping
            }
            Some(current)
                if current.state == BrowserProfileAttachmentState::Stopping
                    && same_profile_reservation(current, &expected_active) =>
            {
                current.clone()
            }
            _ => {
                return Err(CliError::runtime(
                    json,
                    "browser_profile_in_use",
                    BROWSER_PROFILE_IN_USE_MESSAGE,
                    BROWSER_PROFILE_IN_USE_HINT,
                ));
            }
        };
        Some(stopping_owner)
    } else {
        None
    };

    let initial_persistent_status = if record.profile_kind == BrowserProfileKind::Persistent {
        Some(managed_runtime_status(record.runtime, &record.name, json)?)
    } else {
        None
    };
    let gracefully_stopped = if initial_persistent_status == Some(BrowserInstanceStatus::Running) {
        record.endpoint.as_deref().is_some_and(|endpoint| {
            let _ = request_browser_close(endpoint);
            wait_for_managed_runtime_inactive(record.runtime, &record.name, Duration::from_secs(5))
                .unwrap_or(false)
        })
    } else {
        false
    };
    let stop_result = if gracefully_stopped
        || initial_persistent_status.is_some_and(profile_status_allows_release)
    {
        Ok(String::new())
    } else {
        run_runtime_command(browser_stop_command(record.runtime, &record.name), json)
    };
    if record.profile_kind == BrowserProfileKind::Persistent {
        stop_result?;
        let status = managed_runtime_status(record.runtime, &record.name, json)?;
        if !profile_status_allows_release(status) {
            return Err(CliError::runtime(
                json,
                "browser_profile_in_use",
                BROWSER_PROFILE_IN_USE_MESSAGE,
                BROWSER_PROFILE_IN_USE_HINT,
            ));
        }
    }
    record.status = BrowserInstanceStatus::Stopped;
    record.updated_at_unix_ms = lantern_storage::now_unix_ms();
    registry.write_record_atomic(&record).map_err(|_| {
        CliError::runtime(
            json,
            "browser_state_failed",
            BROWSER_STATE_FAILED_MESSAGE,
            BROWSER_STATE_FAILED_HINT,
        )
    })?;
    if let (Some(profile_name), Some(owner)) =
        (record.profile_name.as_deref(), persistent_owner.as_ref())
    {
        let profile_registry = profile_registry.expect("persistent owner requires profile state");
        let released = profile_registry
            .release_if_owner(profile_name, owner)
            .map_err(|error| browser_profile_store_error(error, json))?;
        if !released {
            return Err(browser_state_error(json));
        }
    }

    write_browser_output("browser_stop", record, json)
}

fn browser_prune(
    disposable_registry: &BrowserRegistry,
    persistent_registry: Option<&BrowserRegistry>,
    profile_registry: Option<&BrowserProfileRegistry>,
    json: bool,
) -> Result<(), CliError> {
    let mut pruned = prune_browser_registry(disposable_registry, profile_registry, json)?;
    if let (Some(persistent_registry), Some(profile_registry)) =
        (persistent_registry, profile_registry)
    {
        pruned.extend(prune_browser_registry(
            persistent_registry,
            Some(profile_registry),
            json,
        )?);
    }
    pruned.sort();

    if json {
        write_json(&BrowserPruneOutput {
            schema_version: CLI_SCHEMA_VERSION,
            command: "browser_prune",
            ok: true,
            pruned,
        })?;
        return Ok(());
    }

    println!("browser_prune: pruned={}", pruned.len());
    for id in pruned {
        println!("{id}");
    }
    Ok(())
}

fn prune_browser_registry(
    registry: &BrowserRegistry,
    profile_registry: Option<&BrowserProfileRegistry>,
    json: bool,
) -> Result<Vec<String>, CliError> {
    let records = registry.list_records().map_err(|_| {
        CliError::runtime(
            json,
            "browser_state_failed",
            BROWSER_STATE_FAILED_MESSAGE,
            BROWSER_STATE_FAILED_HINT,
        )
    })?;
    let mut pruned = Vec::new();

    for mut record in records {
        let mut _profile_operation_lock = None;
        let mut persistent_owner = None;
        if record.profile_kind == BrowserProfileKind::Persistent {
            let profile_registry = profile_registry.ok_or_else(|| browser_state_error(json))?;
            let profile_name = record
                .profile_name
                .as_deref()
                .expect("persistent record identity validated by storage")
                .to_owned();
            _profile_operation_lock = match profile_registry.try_operation_lock(&profile_name) {
                Ok(operation_lock) => Some(operation_lock),
                Err(error) if error.kind() == ErrorKind::WouldBlock => continue,
                Err(error) => return Err(browser_profile_store_error(error, json)),
            };
            let current = match registry.read_record(&record.id) {
                Ok(current) => current,
                Err(error) if error.kind() == ErrorKind::NotFound => continue,
                Err(_) => return Err(browser_state_error(json)),
            };
            if current != record {
                continue;
            }
            record = current;
            if profile_record_has_fresh_starting_reservation(
                &record,
                lantern_storage::now_unix_ms(),
            ) {
                continue;
            }
            let expected = profile_attachment_from_record(&record, json)?;
            let profile = profile_registry
                .read(&profile_name)
                .map_err(|error| browser_profile_store_error(error, json))?;
            if profile.attachment.as_ref().is_some_and(|attachment| {
                attachment.state == BrowserProfileAttachmentState::Stopping
            }) {
                continue;
            }
            if profile
                .attachment
                .as_ref()
                .is_some_and(|attachment| attachment != &expected)
            {
                continue;
            }
            if profile.attachment.as_ref() == Some(&expected) {
                persistent_owner = Some(expected);
            }
        }
        let status = if record.profile_kind == BrowserProfileKind::Persistent {
            managed_runtime_status(record.runtime, &record.name, json)?
        } else {
            run_runtime_command(
                browser_inspect_status_command(record.runtime, &record.name),
                json,
            )
            .map(|status| parse_runtime_status(&status))
            .unwrap_or(BrowserInstanceStatus::Missing)
        };

        let pruneable = profile_status_allows_prune(record.profile_kind, status);
        if pruneable {
            let remove_result =
                run_runtime_command(browser_rm_command(record.runtime, &record.name), json);
            if record.profile_kind == BrowserProfileKind::Persistent
                && status != BrowserInstanceStatus::Missing
            {
                remove_result?;
            }
            registry.remove_instance_dir(&record.id).map_err(|_| {
                CliError::runtime(
                    json,
                    "browser_state_failed",
                    BROWSER_STATE_FAILED_MESSAGE,
                    BROWSER_STATE_FAILED_HINT,
                )
            })?;
            if let (Some(profile_name), Some(owner)) =
                (record.profile_name.as_deref(), persistent_owner.as_ref())
            {
                let profile_registry =
                    profile_registry.expect("persistent record requires profile state");
                let released = profile_registry
                    .release_if_owner(profile_name, owner)
                    .map_err(|error| browser_profile_store_error(error, json))?;
                if !released {
                    return Err(browser_state_error(json));
                }
            }
            pruned.push(record.id);
        }
    }

    Ok(pruned)
}

fn find_browser_instance<'a>(
    disposable_registry: &'a BrowserRegistry,
    persistent_registry: Option<&'a BrowserRegistry>,
    id: &str,
    json: bool,
) -> Result<(&'a BrowserRegistry, BrowserInstanceRecord), CliError> {
    let disposable = disposable_registry.read_record(id);
    let persistent = persistent_registry
        .map(|registry| registry.read_record(id))
        .unwrap_or_else(|| Err(std::io::Error::from(ErrorKind::NotFound)));
    match (disposable, persistent) {
        (Ok(_), Ok(_)) => Err(browser_state_error(json)),
        (Ok(record), Err(error)) if error.kind() == ErrorKind::NotFound => {
            Ok((disposable_registry, record))
        }
        (Err(error), Ok(record)) if error.kind() == ErrorKind::NotFound => Ok((
            persistent_registry.expect("successful persistent read requires registry"),
            record,
        )),
        (Err(left), Err(right))
            if left.kind() == ErrorKind::NotFound && right.kind() == ErrorKind::NotFound =>
        {
            Err(browser_state_error(json))
        }
        _ => Err(browser_state_error(json)),
    }
}

fn run_browser_profile_invocation(
    registry: &BrowserProfileRegistry,
    persistent_registry: &BrowserRegistry,
    command: BrowserProfileCommand,
    profile_name: Option<String>,
    confirm: bool,
    json: bool,
) -> Result<(), CliError> {
    match command {
        BrowserProfileCommand::Create => {
            let name = required_browser_profile_name(profile_name, json)?;
            let record = registry
                .create(&name)
                .map_err(|error| browser_profile_store_error(error, json))?;
            write_browser_profile_output("browser_profile_create", registry, record, json)
        }
        BrowserProfileCommand::List => {
            let records = registry
                .list()
                .map_err(|error| browser_profile_store_error(error, json))?;
            let profiles = records
                .into_iter()
                .map(|record| browser_profile_output(registry, record, json))
                .collect::<Result<Vec<_>, _>>()?;
            if json {
                return write_json(&BrowserProfileListOutput {
                    schema_version: CLI_SCHEMA_VERSION,
                    command: "browser_profile_list",
                    ok: true,
                    profiles,
                });
            }
            for profile in profiles {
                println!(
                    "{} attached_instance={} profile={}",
                    profile.name,
                    profile.attached_instance_id.as_deref().unwrap_or("null"),
                    profile.profile_dir
                );
            }
            Ok(())
        }
        BrowserProfileCommand::Status => {
            let name = required_browser_profile_name(profile_name, json)?;
            let record = registry
                .read(&name)
                .map_err(|error| browser_profile_store_error(error, json))?;
            write_browser_profile_output("browser_profile_status", registry, record, json)
        }
        BrowserProfileCommand::Delete => {
            let name = required_browser_profile_name(profile_name, json)?;
            if !confirm {
                return Err(CliError::usage(
                    json,
                    BROWSER_PROFILE_DELETE_CONFIRM_MESSAGE,
                    BROWSER_PROFILE_DELETE_CONFIRM_HINT,
                ));
            }
            let _operation_lock = registry
                .try_operation_lock(&name)
                .map_err(|error| browser_profile_store_error(error, json))?;
            if registry
                .read(&name)
                .map_err(|error| browser_profile_store_error(error, json))?
                .attachment
                .is_some()
            {
                return Err(CliError::runtime(
                    json,
                    "browser_profile_in_use",
                    BROWSER_PROFILE_IN_USE_MESSAGE,
                    BROWSER_PROFILE_IN_USE_HINT,
                ));
            }
            remove_stopped_profile_instances(persistent_registry, &name, json)?;
            registry
                .delete(&name)
                .map_err(|error| browser_profile_store_error(error, json))?;
            if json {
                return write_json(&BrowserProfileDeleteOutput {
                    schema_version: CLI_SCHEMA_VERSION,
                    command: "browser_profile_delete",
                    ok: true,
                    deleted_profile: name,
                });
            }
            println!("browser_profile_delete: deleted={name}");
            Ok(())
        }
    }
}

fn remove_stopped_profile_instances(
    persistent_registry: &BrowserRegistry,
    profile_name: &str,
    json: bool,
) -> Result<(), CliError> {
    for record in persistent_registry
        .list_records()
        .map_err(|_| browser_state_error(json))?
        .into_iter()
        .filter(|record| record.profile_name.as_deref() == Some(profile_name))
    {
        let status = managed_runtime_status(record.runtime, &record.name, json)?;
        if matches!(
            status,
            BrowserInstanceStatus::Starting | BrowserInstanceStatus::Running
        ) {
            return Err(CliError::runtime(
                json,
                "browser_profile_in_use",
                BROWSER_PROFILE_IN_USE_MESSAGE,
                BROWSER_PROFILE_IN_USE_HINT,
            ));
        }
        if status != BrowserInstanceStatus::Missing {
            run_runtime_command(browser_rm_command(record.runtime, &record.name), json)?;
        }
        persistent_registry
            .remove_instance_dir(&record.id)
            .map_err(|_| browser_state_error(json))?;
    }
    Ok(())
}

fn write_browser_profile_output(
    command: &'static str,
    registry: &BrowserProfileRegistry,
    record: BrowserProfileRecord,
    json: bool,
) -> Result<(), CliError> {
    let profile = browser_profile_output(registry, record, json)?;
    if json {
        return write_json(&BrowserProfileCommandOutput {
            schema_version: CLI_SCHEMA_VERSION,
            command,
            ok: true,
            profile,
        });
    }
    println!(
        "{command}: name={} attached_instance={} profile={}",
        profile.name,
        profile.attached_instance_id.as_deref().unwrap_or("null"),
        profile.profile_dir
    );
    Ok(())
}

fn browser_profile_output(
    registry: &BrowserProfileRegistry,
    record: BrowserProfileRecord,
    json: bool,
) -> Result<BrowserProfileOutput, CliError> {
    let profile_dir = registry
        .data_dir(&record.name)
        .map_err(|error| browser_profile_store_error(error, json))?;
    Ok(BrowserProfileOutput {
        name: record.name,
        profile_dir: profile_dir.display().to_string(),
        attached_instance_id: record
            .attachment
            .as_ref()
            .map(|attachment| attachment.instance_id.clone()),
        created_at_unix_ms: record.created_at_unix_ms,
        updated_at_unix_ms: record.updated_at_unix_ms,
    })
}

fn write_browser_output(
    command: &'static str,
    record: BrowserInstanceRecord,
    json: bool,
) -> Result<(), CliError> {
    if json {
        write_json(&BrowserCommandOutput {
            schema_version: CLI_SCHEMA_VERSION,
            command,
            ok: true,
            instance: BrowserInstanceOutput::from_record(record),
        })?;
        return Ok(());
    }

    println!(
        "{}: id={} status={} runtime={} graphics={} gpu_device={} unsafe_webgpu={} endpoint={} novnc={} profile_kind={} profile_name={} host_gateway={} profile={}",
        command,
        record.id,
        record.status.as_str(),
        record.runtime.as_str(),
        record.graphics.as_str(),
        record.gpu_device.as_deref().unwrap_or("null"),
        record.graphics == BrowserGraphicsMode::WebGpu,
        record.endpoint.as_deref().unwrap_or("null"),
        record.novnc_url.as_deref().unwrap_or("null"),
        record.profile_kind.as_str(),
        record.profile_name.as_deref().unwrap_or("null"),
        record.host_gateway.as_deref().unwrap_or("null"),
        record.profile_dir.display()
    );
    Ok(())
}

fn runtime_port(
    runtime: RuntimeKind,
    name: &str,
    container_port: u16,
    json: bool,
) -> Result<u16, CliError> {
    let output = run_runtime_command(browser_port_command(runtime, name, container_port), json)?;
    parse_published_port(&output).ok_or_else(|| {
        CliError::runtime(
            json,
            "browser_port_missing",
            BROWSER_PORT_MISSING_MESSAGE,
            BROWSER_PORT_MISSING_HINT,
        )
    })
}

fn managed_runtime_status(
    runtime: RuntimeKind,
    name: &str,
    json: bool,
) -> Result<BrowserInstanceStatus, CliError> {
    if let Ok(status) = run_runtime_command(browser_inspect_status_command(runtime, name), json) {
        return Ok(parse_runtime_status(&status));
    }
    let names = run_runtime_command(browser_ps_all_command(runtime), json)?;
    if names
        .lines()
        .map(str::trim)
        .any(|candidate| candidate == name)
    {
        Ok(BrowserInstanceStatus::Error)
    } else {
        Ok(BrowserInstanceStatus::Missing)
    }
}

fn wait_for_browser_ready(endpoint: &str, timeout: Duration, json: bool) -> Result<(), CliError> {
    let endpoint = ResolvedEndpoint {
        source: lantern_core::endpoint::EndpointSource::Flag,
        display: endpoint.to_owned(),
    };
    let started = Instant::now();
    let deadline = started + timeout;
    let client = CdpClient::new(endpoint).with_deadline(deadline);

    while Instant::now() < deadline {
        if client.browser_version().is_ok() {
            return Ok(());
        }
        std::thread::sleep(
            deadline
                .saturating_duration_since(Instant::now())
                .min(Duration::from_millis(100)),
        );
    }

    Err(CliError::runtime(
        json,
        "browser_not_ready",
        BROWSER_NOT_READY_MESSAGE,
        BROWSER_NOT_READY_HINT,
    ))
}

fn request_browser_close(endpoint: &str) -> Result<(), CdpError> {
    let endpoint = ResolvedEndpoint {
        source: lantern_core::endpoint::EndpointSource::Flag,
        display: endpoint.to_owned(),
    };
    CdpClient::new(endpoint).close_browser()
}

fn wait_for_managed_runtime_inactive(
    runtime: RuntimeKind,
    name: &str,
    timeout: Duration,
) -> Result<bool, CliError> {
    let started = Instant::now();
    while started.elapsed() <= timeout {
        if profile_status_allows_release(managed_runtime_status(runtime, name, false)?) {
            return Ok(true);
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Ok(false)
}

fn select_runtime(requested: Option<RuntimeKind>, json: bool) -> Result<RuntimeKind, CliError> {
    if let Some(runtime) = requested {
        if runtime_available(runtime) {
            return Ok(runtime);
        }
        return Err(CliError::runtime(
            json,
            "browser_runtime_unavailable",
            BROWSER_RUNTIME_UNAVAILABLE_MESSAGE,
            BROWSER_RUNTIME_UNAVAILABLE_HINT,
        ));
    }

    [RuntimeKind::Podman, RuntimeKind::Docker]
        .into_iter()
        .find(|runtime| runtime_available(*runtime))
        .ok_or_else(|| {
            CliError::runtime(
                json,
                "browser_runtime_unavailable",
                BROWSER_RUNTIME_UNAVAILABLE_MESSAGE,
                BROWSER_RUNTIME_UNAVAILABLE_HINT,
            )
        })
}

fn runtime_available(runtime: RuntimeKind) -> bool {
    ProcessCommand::new(runtime.program())
        .arg("--version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn run_runtime_command(command: RuntimeCommand, json: bool) -> Result<String, CliError> {
    let output = ProcessCommand::new(&command.program)
        .args(&command.args)
        .output()
        .map_err(|_| {
            CliError::runtime(
                json,
                "browser_runtime_failed",
                BROWSER_RUNTIME_FAILED_MESSAGE,
                BROWSER_RUNTIME_FAILED_HINT,
            )
        })?;

    if !output.status.success() {
        return Err(CliError::runtime(
            json,
            "browser_runtime_failed",
            BROWSER_RUNTIME_FAILED_MESSAGE,
            BROWSER_RUNTIME_FAILED_HINT,
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn required_browser_id(id: Option<String>, json: bool) -> Result<String, CliError> {
    let id = id.ok_or_else(|| {
        CliError::usage(json, BROWSER_ID_MISSING_MESSAGE, BROWSER_ID_MISSING_HINT)
    })?;
    validate_browser_id(&id, json)?;
    Ok(id)
}

fn required_browser_profile_name(name: Option<String>, json: bool) -> Result<String, CliError> {
    let name = name.ok_or_else(|| {
        CliError::usage(
            json,
            BROWSER_PROFILE_MISSING_MESSAGE,
            BROWSER_PROFILE_MISSING_HINT,
        )
    })?;
    validate_browser_profile_name(&name, json)?;
    Ok(name)
}

fn validate_browser_profile_name(name: &str, json: bool) -> Result<(), CliError> {
    validate_profile_name(name).map_err(|_| {
        CliError::usage(
            json,
            "Invalid browser profile name.",
            BROWSER_PROFILE_MISSING_HINT,
        )
    })
}

fn validate_browser_host_gateway(hostname: &str, json: bool) -> Result<(), CliError> {
    validate_host_gateway_hostname(hostname).map_err(|_| {
        CliError::usage(
            json,
            BROWSER_HOST_GATEWAY_INVALID_MESSAGE,
            BROWSER_HOST_GATEWAY_INVALID_HINT,
        )
    })
}

fn validate_browser_id(id: &str, json: bool) -> Result<(), CliError> {
    let valid = !id.is_empty()
        && id.len() <= 80
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_');
    if valid {
        Ok(())
    } else {
        Err(CliError::usage(
            json,
            "Invalid browser instance id.",
            "Use only ASCII letters, numbers, hyphen, and underscore.",
        ))
    }
}

fn repo_root() -> std::io::Result<PathBuf> {
    let output = ProcessCommand::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()?;
    if output.status.success() {
        let root = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        if !root.is_empty() {
            return Ok(PathBuf::from(root));
        }
    }
    env::current_dir()
}

fn lantern_state_home(json: bool) -> Result<PathBuf, CliError> {
    optional_lantern_state_home(json)?.ok_or_else(|| {
        CliError::runtime(
            json,
            "browser_state_home_invalid",
            BROWSER_STATE_HOME_INVALID_MESSAGE,
            BROWSER_STATE_HOME_INVALID_HINT,
        )
    })
}

fn optional_lantern_state_home(json: bool) -> Result<Option<PathBuf>, CliError> {
    let lantern_home = env::var("LANTERN_STATE_HOME").ok();
    let xdg_home = env::var("XDG_STATE_HOME").ok();
    let home = env::var("HOME").ok();
    let configured = [
        lantern_home.as_deref(),
        xdg_home.as_deref(),
        home.as_deref(),
    ]
    .into_iter()
    .any(|value| value.is_some_and(|value| !value.is_empty()));
    let resolved = resolve_lantern_state_home(
        lantern_home.as_deref(),
        xdg_home.as_deref(),
        home.as_deref(),
    );
    if configured && resolved.is_none() {
        return Err(CliError::runtime(
            json,
            "browser_state_home_invalid",
            BROWSER_STATE_HOME_INVALID_MESSAGE,
            BROWSER_STATE_HOME_INVALID_HINT,
        ));
    }
    Ok(resolved)
}

fn resolve_lantern_state_home(
    lantern_home: Option<&str>,
    xdg_home: Option<&str>,
    home: Option<&str>,
) -> Option<PathBuf> {
    if let Some(path) = lantern_home.filter(|value| !value.is_empty()) {
        let path = PathBuf::from(path);
        return path.is_absolute().then_some(path);
    }
    if let Some(path) = xdg_home.filter(|value| !value.is_empty()) {
        let path = PathBuf::from(path);
        return path.is_absolute().then(|| path.join("lantern"));
    }
    let home = PathBuf::from(home.filter(|value| !value.is_empty())?);
    home.is_absolute()
        .then(|| home.join(".local").join("state").join("lantern"))
}

fn profile_attachment_is_within_starting_grace(
    updated_at_unix_ms: u128,
    now_unix_ms: u128,
) -> bool {
    now_unix_ms.saturating_sub(updated_at_unix_ms) < BROWSER_PROFILE_STARTING_GRACE_MS
}

fn profile_status_allows_release(status: BrowserInstanceStatus) -> bool {
    matches!(
        status,
        BrowserInstanceStatus::Stopped | BrowserInstanceStatus::Missing
    )
}

fn profile_status_allows_prune(
    profile_kind: BrowserProfileKind,
    status: BrowserInstanceStatus,
) -> bool {
    if profile_kind == BrowserProfileKind::Persistent {
        return profile_status_allows_release(status);
    }
    matches!(
        status,
        BrowserInstanceStatus::Stopped
            | BrowserInstanceStatus::Missing
            | BrowserInstanceStatus::Error
    )
}

fn profile_record_has_fresh_starting_reservation(
    record: &BrowserInstanceRecord,
    now_unix_ms: u128,
) -> bool {
    record.profile_kind == BrowserProfileKind::Persistent
        && record.status == BrowserInstanceStatus::Starting
        && profile_attachment_is_within_starting_grace(record.updated_at_unix_ms, now_unix_ms)
}

fn persistent_attachment_owns_runtime_target(
    attachment: Option<&BrowserProfileAttachment>,
    runtime: RuntimeKind,
    container_name: &str,
) -> bool {
    attachment.is_some_and(|attachment| {
        attachment.runtime == runtime && attachment.container_name == container_name
    })
}

fn persistent_record_owns_runtime_target(
    record: &BrowserInstanceRecord,
    runtime: RuntimeKind,
    container_name: &str,
) -> bool {
    record.runtime == runtime && record.name == container_name
}

fn profile_attachment_from_record(
    record: &BrowserInstanceRecord,
    json: bool,
) -> Result<BrowserProfileAttachment, CliError> {
    let reservation_id = record.profile_reservation_id.clone().ok_or_else(|| {
        CliError::runtime(
            json,
            "browser_state_failed",
            BROWSER_STATE_FAILED_MESSAGE,
            BROWSER_STATE_FAILED_HINT,
        )
    })?;
    Ok(BrowserProfileAttachment {
        instance_id: record.id.clone(),
        container_name: record.name.clone(),
        runtime: record.runtime,
        reservation_id,
        state: BrowserProfileAttachmentState::Active,
    })
}

fn same_profile_reservation(
    left: &BrowserProfileAttachment,
    right: &BrowserProfileAttachment,
) -> bool {
    left.instance_id == right.instance_id
        && left.container_name == right.container_name
        && left.runtime == right.runtime
        && left.reservation_id == right.reservation_id
}

fn browser_state_error(json: bool) -> CliError {
    CliError::runtime(
        json,
        "browser_state_failed",
        BROWSER_STATE_FAILED_MESSAGE,
        BROWSER_STATE_FAILED_HINT,
    )
}

fn browser_profile_store_error(error: std::io::Error, json: bool) -> CliError {
    match error.kind() {
        ErrorKind::NotFound => CliError::runtime(
            json,
            "browser_profile_not_found",
            BROWSER_PROFILE_NOT_FOUND_MESSAGE,
            BROWSER_PROFILE_NOT_FOUND_HINT,
        ),
        ErrorKind::AlreadyExists => CliError::runtime(
            json,
            "browser_profile_exists",
            BROWSER_PROFILE_EXISTS_MESSAGE,
            BROWSER_PROFILE_EXISTS_HINT,
        ),
        ErrorKind::WouldBlock => CliError::runtime(
            json,
            "browser_profile_in_use",
            BROWSER_PROFILE_IN_USE_MESSAGE,
            BROWSER_PROFILE_IN_USE_HINT,
        ),
        _ => browser_state_error(json),
    }
}

#[derive(Debug, Serialize)]
struct BrowserCommandOutput {
    schema_version: u8,
    command: &'static str,
    ok: bool,
    instance: BrowserInstanceOutput,
}

#[derive(Debug, Serialize)]
struct BrowserListOutput {
    schema_version: u8,
    command: &'static str,
    ok: bool,
    instances: Vec<BrowserInstanceOutput>,
}

#[derive(Debug, Serialize)]
struct BrowserPruneOutput {
    schema_version: u8,
    command: &'static str,
    ok: bool,
    pruned: Vec<String>,
}

#[derive(Debug, Serialize)]
struct BrowserProfileCommandOutput {
    schema_version: u8,
    command: &'static str,
    ok: bool,
    profile: BrowserProfileOutput,
}

#[derive(Debug, Serialize)]
struct BrowserProfileListOutput {
    schema_version: u8,
    command: &'static str,
    ok: bool,
    profiles: Vec<BrowserProfileOutput>,
}

#[derive(Debug, Serialize)]
struct BrowserProfileDeleteOutput {
    schema_version: u8,
    command: &'static str,
    ok: bool,
    deleted_profile: String,
}

#[derive(Debug, Serialize)]
struct BrowserProfileOutput {
    name: String,
    profile_dir: String,
    attached_instance_id: Option<String>,
    created_at_unix_ms: u128,
    updated_at_unix_ms: u128,
}

#[derive(Debug, Serialize)]
struct BrowserInstanceOutput {
    id: String,
    name: String,
    runtime: &'static str,
    image: String,
    status: &'static str,
    graphics: &'static str,
    gpu_device: Option<String>,
    unsafe_webgpu: bool,
    endpoint: Option<String>,
    cdp_host_port: Option<u16>,
    novnc_url: Option<String>,
    profile_dir: String,
    profile_kind: &'static str,
    profile_name: Option<String>,
    host_gateway: Option<String>,
}

impl BrowserInstanceOutput {
    fn from_record(record: BrowserInstanceRecord) -> Self {
        Self {
            id: record.id,
            name: record.name,
            runtime: record.runtime.as_str(),
            image: record.image,
            status: record.status.as_str(),
            graphics: record.graphics.as_str(),
            gpu_device: record.gpu_device,
            unsafe_webgpu: record.graphics == BrowserGraphicsMode::WebGpu,
            endpoint: record.endpoint,
            cdp_host_port: record.cdp_host_port,
            novnc_url: record.novnc_url,
            profile_dir: record.profile_dir.display().to_string(),
            profile_kind: record.profile_kind.as_str(),
            profile_name: record.profile_name,
            host_gateway: record.host_gateway,
        }
    }
}

#[cfg(test)]
mod tests;
