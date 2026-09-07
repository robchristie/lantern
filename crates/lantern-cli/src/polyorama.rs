//! Narrow consumer of Polyorama's owned evidence. No application hooks belong in CDP.
use std::{collections::BTreeSet, fs::File, io::Read, path::Path};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use lantern_core::{
    cdp::CdpWebSocket,
    redaction::{RedactionMode, sanitize_dom_text, sanitize_url},
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::{
    args::Invocation,
    dispatch::EndpointContext,
    error::CliError,
    output::write_json,
    screenshot::{validate_screenshot_output_path, write_screenshot_file},
    selection::select_page_target,
};

const MAX_BYTES: usize = 8 * 1024 * 1024;
const MAX_ITEMS: usize = 5000;
const HINT: &str = "Use a Polyorama version 1 UI evidence directory, or the current gallery snapshot hook with --timeout-ms.";
// Fixed read-only observation, never interpolate user code or call action/repaint hooks.
const SNAPSHOT: &str = r#"(() => {
 const h = window.__POLYORAMA_GALLERY_HANDLE;
 if (!h || typeof h.snapshot !== 'function') return {unavailable: 'gallery_snapshot_hook_missing'};
 const s = h.snapshot();
 const encoded = JSON.stringify(s);
 if (encoded.length > 2097152) return {unavailable: 'snapshot_size_limit'};
 return {snapshot: s, bootstrap_ready: document.body.classList.contains('ready'),
   viewport: {width: innerWidth, height: innerHeight, device_pixel_ratio: devicePixelRatio},
   url: location.href};
})()"#;

fn error(json: bool) -> CliError {
    CliError::usage(
        json,
        "Invalid, unavailable or oversized Polyorama evidence.",
        HINT,
    )
}

pub(crate) fn validate_invocation(i: &Invocation) -> Result<(), CliError> {
    if i.region_x.is_some()
        || i.region_y.is_some()
        || i.region_width.is_some()
        || i.region_height.is_some()
    {
        return Err(error(i.json));
    }
    if i.evidence_dir.is_some() {
        if i.endpoint.is_some()
            || i.target_id.is_some()
            || i.timeout_ms.is_some()
            || i.has_screenshot_flags()
        {
            return Err(error(i.json));
        }
    } else {
        crate::inspection::validate_wait_timeout(
            i.timeout_ms.ok_or_else(|| error(i.json))?,
            None,
            i.json,
        )?;
        if i.screenshot_overwrite && i.screenshot_output.is_none() {
            return Err(error(i.json));
        }
    }
    Ok(())
}

fn bytes(path: &Path) -> Result<Vec<u8>, ()> {
    if !std::fs::metadata(path).map_err(|_| ())?.is_file() {
        return Err(());
    }
    let file = File::open(path).map_err(|_| ())?;
    if !file.metadata().map_err(|_| ())?.is_file() {
        return Err(());
    }
    let mut data = Vec::new();
    file.take((MAX_BYTES + 1) as u64)
        .read_to_end(&mut data)
        .map_err(|_| ())?;
    if data.len() > MAX_BYTES {
        return Err(());
    }
    Ok(data)
}
fn digest(data: &[u8]) -> String {
    format!("{:x}", Sha256::digest(data))
}
fn read_json(path: &Path) -> Result<(Value, Value), ()> {
    let data = bytes(path)?;
    let value = serde_json::from_slice(&data).map_err(|_| ())?;
    Ok((value, json!({"sha256": digest(&data), "bytes": data.len()})))
}
fn array<'a>(v: &'a Value, key: &str) -> Result<&'a Vec<Value>, ()> {
    let a = v[key].as_array().ok_or(())?;
    if a.len() > MAX_ITEMS {
        return Err(());
    }
    Ok(a)
}
fn number(v: &Value, key: &str) -> Result<f64, ()> {
    v[key].as_f64().filter(|n| n.is_finite()).ok_or(())
}
fn count(v: &Value, key: &str) -> Result<u64, ()> {
    v[key].as_u64().ok_or(())
}
fn string<'a>(v: &'a Value, key: &str) -> Result<&'a str, ()> {
    v[key]
        .as_str()
        .filter(|s| !s.is_empty() && s.len() <= 4096)
        .ok_or(())
}
fn rect(v: &Value) -> Result<(), ()> {
    let x = number(v, "min_x")?;
    let y = number(v, "min_y")?;
    let max_x = number(v, "max_x")?;
    let max_y = number(v, "max_y")?;
    if x > max_x || y > max_y || [x, y, max_x, max_y].iter().any(|n| n.abs() > 1e7) {
        return Err(());
    }
    Ok(())
}
fn version(v: &Value) -> Result<(), ()> {
    if v["schema_version"] != 1 {
        Err(())
    } else {
        Ok(())
    }
}

fn validate_semantic(v: &Value) -> Result<(), ()> {
    if number(v, "pixels_per_point")? <= 0.0 {
        return Err(());
    }
    let root = string(v, "root")?;
    let nodes = array(v, "nodes")?;
    let mut ids = BTreeSet::new();
    for node in nodes {
        if !ids.insert(string(node, "id")?) {
            return Err(());
        }
        if ![
            "application",
            "application_bar",
            "toolbar",
            "button",
            "radio_button",
            "combo_box",
            "slider",
            "tab",
            "splitter",
            "pane",
            "viewport",
            "scroll_area",
            "result_row",
            "thumbnail_cell",
            "status",
            "section",
        ]
        .contains(&string(node, "role")?)
        {
            return Err(());
        }
        if !node["name"].is_string() {
            return Err(());
        }
        rect(&node["rect"])?;
        for key in ["enabled", "focused", "selected"] {
            if !node[key].is_boolean() {
                return Err(());
            }
        }
        for action in array(node, "actions")? {
            if !action.is_string() {
                return Err(());
            }
        }
    }
    if !ids.contains(root) {
        return Err(());
    }
    for node in nodes {
        if !node["parent"].is_null() && !ids.contains(node["parent"].as_str().ok_or(())?) {
            return Err(());
        }
    }
    array(v, "semantic_audit")?;
    Ok(())
}

fn validate_text(v: &Value) -> Result<(), ()> {
    let observations = array(v, "observations")?;
    let mut components = BTreeSet::new();
    let mut failures = BTreeSet::new();
    for o in observations {
        string(&o["component_id"], "kind")?;
        count(&o["component_id"], "instance")?;
        let id = o["component_id"].to_string();
        components.insert(id.clone());
        if !o["layout_error"].is_null() {
            failures.insert(id);
        }
        for key in ["allocated_rect", "painted_rect", "clip_rect"] {
            rect(&o[key])?;
        }
        count(o, "line_count")?;
        if !o["truncated"].is_boolean() {
            return Err(());
        }
    }
    array(v, "audit")?;
    // Older owner snapshots explicitly report unavailable coverage, never an audit pass.
    let c = &v["coverage"];
    if c.is_null() {
        return Ok(());
    }
    let measured = count(c, "measured_components")?;
    let attempted = count(c, "attempted_components")?;
    let successful = count(c, "successful_components")?;
    let failed = count(c, "failed_components")?;
    if successful.checked_add(failed) != Some(attempted)
        || attempted < measured
        || measured != components.len() as u64
        || failed < failures.len() as u64
        || count(c, "observed_native_controls")? > count(c, "native_text_controls")?
    {
        return Err(());
    }
    let exclusions = array(c, "excluded_categories")?;
    if !exclusions.iter().all(Value::is_string)
        || !exclusions.contains(&json!("ordinary_egui_labels"))
    {
        return Err(());
    }
    Ok(())
}

fn png_info(data: &[u8]) -> Result<Value, ()> {
    if data.len() > MAX_BYTES {
        return Err(());
    }
    let mut decoder = png::Decoder::new(data);
    decoder.set_limits(png::Limits {
        bytes: 64 * 1024 * 1024,
    });
    let mut reader = decoder.read_info().map_err(|_| ())?;
    if reader.output_buffer_size() > 64 * 1024 * 1024 {
        return Err(());
    }
    let mut buffer = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buffer).map_err(|_| ())?;
    reader.finish().map_err(|_| ())?;
    Ok(
        json!({"width":info.width,"height":info.height,"sha256":digest(data),"bytes":data.len(),"format":"png","pixel_frame":null,"redaction_caveat":"screenshot_contains_visible_page_pixels"}),
    )
}
fn unavailable(reason: &str) -> Value {
    json!({"status":"unavailable", "reason":reason})
}
fn correlation(before: Option<u64>, after: Option<u64>) -> Value {
    json!({"before":before,"after":after,"status":match (before,after) {
        (Some(a),Some(b)) if a == b => "same_observed_frame",
        (Some(_),Some(_)) => "differing",
        _ => "unavailable"
    },"pixel_frame":null,"atomicity":"not_established"})
}
fn report(mode: &str, semantic: Value, text: Value, metadata: Value) -> Value {
    let coverage = if text["coverage"].is_null() {
        unavailable("owner_did_not_supply_coverage")
    } else {
        json!({"status":"known","value":text["coverage"]})
    };
    json!({"schema_version":1,"command":"polyorama","ok":true,"mode":mode,
        "validation":"bounded_contract_valid_not_application_acceptance",
        "source_revision":unavailable("owner_did_not_supply_source_revision"),
        "application_revision":unavailable("owner_did_not_supply_application_revision"),
        "frame_correlation":correlation(None,None),
        "readiness":unavailable("owner_did_not_supply_runtime_readiness"),
        "outstanding_work":unavailable("owner_did_not_supply_outstanding_work"),
        "coordinates":{"semantic_and_text":"egui_points","screenshots":"device_pixels","pixels_per_point":semantic["pixels_per_point"]},
        "semantic_clipping":unavailable("owner_nodes_have_rectangles_but_no_clip_rectangles"),
        "coverage":coverage,"metadata":metadata,"semantic":semantic,"text":text,
        "screenshot":unavailable("not_requested")})
}
fn sanitise(v: &mut Value, mode: RedactionMode) {
    match v {
        Value::String(s) => *s = sanitize_dom_text(s, mode),
        Value::Array(a) => {
            for v in a {
                sanitise(v, mode);
            }
        }
        Value::Object(o) => {
            for v in o.values_mut() {
                sanitise(v, mode);
            }
        }
        _ => {}
    }
}
fn redact_owner_text(v: &mut Value, mode: RedactionMode) {
    match v {
        Value::Array(a) => {
            for v in a {
                redact_owner_text(v, mode);
            }
        }
        Value::Object(o) => {
            for (key, v) in o.iter_mut() {
                if matches!(
                    key.as_str(),
                    "name"
                        | "description"
                        | "disabled_reason"
                        | "layout_error"
                        | "domain_reference"
                ) {
                    sanitise(v, mode);
                } else {
                    redact_owner_text(v, mode);
                }
            }
        }
        _ => {}
    }
}
fn emit(mut output: Value, i: &Invocation) -> Result<(), CliError> {
    redact_owner_text(&mut output, RedactionMode::from_no_redact(i.no_redact));
    output["text_policy"] = json!(if i.no_redact {
        "unredacted"
    } else {
        "owner_free_text_redacted_and_limited_to_500_characters_identifiers_retained"
    });
    write_json(&output)
}

pub(crate) fn run_offline(i: &Invocation) -> Result<(), CliError> {
    let result = (|| -> Result<Value, ()> {
        let dir = i.evidence_dir.as_deref().ok_or(())?;
        let (metadata, mh) = read_json(&dir.join("metadata.json"))?;
        let (semantic, sh) = read_json(&dir.join("semantic.json"))?;
        let (text, th) = read_json(&dir.join("text.json"))?;
        for v in [&metadata, &semantic, &text] {
            version(v)?;
        }
        for key in ["fixture_id", "story", "data_fixture", "fonts", "renderer"] {
            string(&metadata, key)?;
        }
        let width = count(&metadata["viewport"], "width")?;
        let height = count(&metadata["viewport"], "height")?;
        if width == 0 || height == 0 {
            return Err(());
        }
        validate_semantic(&semantic)?;
        validate_text(&text)?;
        let mut output = report("offline_owner_bundle", semantic, text, metadata);
        let visual_path = dir.join("visual.png");
        output["screenshot"] = if visual_path.exists() {
            let mut info = png_info(&bytes(&visual_path)?)?;
            info["viewport_correlation"] =
                json!(if info["width"] == width && info["height"] == height {
                    "same_dimensions"
                } else {
                    "differing"
                });
            info["path"] = json!(visual_path);
            info
        } else {
            unavailable("visual.png_missing")
        };
        output["artefacts"] = json!({"metadata.json":mh,"semantic.json":sh,"text.json":th});
        Ok(output)
    })()
    .map_err(|_| error(i.json))?;
    emit(result, i)
}

fn observe(socket: &mut CdpWebSocket, json_mode: bool) -> Result<Value, CliError> {
    let response = socket
        .call(
            "Runtime.evaluate",
            Some(json!({"expression":SNAPSHOT,"returnByValue":true})),
        )
        .map_err(|e| CliError::from_cdp(e, json_mode))?;
    if response.get("exceptionDetails").is_some() {
        return Err(error(json_mode));
    }
    let value = response["result"]["value"].clone();
    if serde_json::to_vec(&value)
        .map_err(|_| error(json_mode))?
        .len()
        > MAX_BYTES
    {
        return Err(error(json_mode));
    }
    Ok(value)
}
fn live_parts(observation: &Value) -> Result<(Value, Value), ()> {
    let s = &observation["snapshot"];
    count(s, "frame")?;
    string(s, "story")?;
    let mut semantic = s["ui_snapshot"].clone();
    count(&semantic, "frame")?;
    let text = json!({"observations":s["text"],"audit":s["text_audit"],"coverage":s["text_audit_coverage"]});
    validate_semantic(&semantic)?;
    validate_text(&text)?;
    if let Some(o) = semantic.as_object_mut() {
        for key in ["text", "text_audit", "text_audit_coverage"] {
            o.remove(key);
        }
    }
    Ok((semantic, text))
}
pub(crate) fn run_live(context: EndpointContext) -> Result<(), CliError> {
    let i = &context.invocation;
    if let Some(path) = i.screenshot_output.as_deref() {
        validate_screenshot_output_path(path, i.screenshot_overwrite, i.json)?;
    }
    let page = select_page_target(
        context
            .client
            .targets()
            .map_err(|e| CliError::from_cdp(e, i.json))?,
        i.target_id.as_deref(),
    )
    .map_err(|e| e.with_json(i.json))?;
    let mut socket = CdpWebSocket::connect_until(
        page.web_socket_debugger_url
            .as_deref()
            .ok_or_else(|| error(i.json))?,
        context.budget.expect("validated timeout").end(),
    )
    .map_err(|e| CliError::from_cdp(e, i.json))?;
    let browser = context
        .client
        .browser_version()
        .map_err(|e| CliError::from_cdp(e, i.json))?;
    let before = observe(&mut socket, i.json)?;
    if let Some(reason) = before["unavailable"].as_str() {
        let mut output = report(
            "live_gallery_snapshot",
            Value::Null,
            Value::Null,
            Value::Null,
        );
        output["ok"] = json!(false);
        output["snapshot"] = unavailable(reason);
        return emit(output, i);
    }
    let (semantic, text) = live_parts(&before).map_err(|_| error(i.json))?;
    let mut output = report(
        "live_gallery_snapshot",
        semantic,
        text,
        json!({"story":before["snapshot"]["story"],"configuration":before["snapshot"]["configuration"],"viewport":before["viewport"]}),
    );
    output["browser"] =
        json!({"product":browser.browser,"protocol_version":browser.protocol_version});
    output["text_correlation"] = json!({"observations":if before["snapshot"]["text"] == before["snapshot"]["ui_snapshot"]["text"] {"same"}else{"differing"},"coverage":if before["snapshot"]["text_audit_coverage"] == before["snapshot"]["ui_snapshot"]["text_audit_coverage"] {"same"}else{"differing"}});
    output["page"] = json!({"target_id":page.id,"url_shape":before["url"].as_str().and_then(|u| sanitize_url(u,RedactionMode::from_no_redact(i.no_redact)))});
    output["owner_frames"] = json!({"application":before["snapshot"]["frame"],"semantic":before["snapshot"]["ui_snapshot"]["frame"],"status":if before["snapshot"]["frame"] == before["snapshot"]["ui_snapshot"]["frame"] {"same"}else{"differing"}});
    output["readiness"] = json!({"status":"partial","bootstrap_ready":before["bootstrap_ready"],"frame_observed":before["snapshot"]["frame"],"application_ready":null});
    output["snapshot_sha256"] = json!(digest(
        &serde_json::to_vec(&before["snapshot"]).map_err(|_| error(i.json))?
    ));
    if let Some(path) = i.screenshot_output.as_deref() {
        let capture = socket
            .call(
                "Page.captureScreenshot",
                Some(json!({"format":"png","fromSurface":true,"captureBeyondViewport":false})),
            )
            .map_err(|e| CliError::from_cdp(e, i.json))?;
        let data = STANDARD
            .decode(capture["data"].as_str().ok_or_else(|| error(i.json))?)
            .map_err(|_| error(i.json))?;
        let mut info = png_info(&data).map_err(|_| error(i.json))?;
        // Persist already captured pixels even when the trailing observation is unavailable.
        info["overwritten"] = json!(write_screenshot_file(
            path,
            &data,
            i.screenshot_overwrite,
            i.json
        )?);
        info["path"] = json!(path);
        let viewport = &before["viewport"];
        let expected = viewport["device_pixel_ratio"]
            .as_f64()
            .zip(viewport["width"].as_f64())
            .zip(viewport["height"].as_f64());
        info["viewport_correlation"] = json!(match expected {
            Some(((scale, width), height))
                if info["width"].as_f64() == Some(width * scale)
                    && info["height"].as_f64() == Some(height * scale) =>
                "same_dimensions",
            Some(_) => "differing",
            None => "unavailable",
        });
        output["screenshot"] = info;
        match observe(&mut socket, i.json) {
            Ok(after) if live_parts(&after).is_ok() => {
                output["frame_correlation"] = correlation(
                    before["snapshot"]["frame"].as_u64(),
                    after["snapshot"]["frame"].as_u64(),
                );
                output["after_snapshot_sha256"] = json!(digest(
                    &serde_json::to_vec(&after["snapshot"]).map_err(|_| error(i.json))?
                ));
                output["story_correlation"] = json!(if before["snapshot"]["story"]
                    == after["snapshot"]["story"]
                {
                    "same"
                } else {
                    "differing"
                });
            }
            _ => {
                output["frame_correlation"] =
                    correlation(before["snapshot"]["frame"].as_u64(), None);
            }
        }
    }
    output["evidence_loss"] =
        serde_json::to_value(socket.evidence_loss()).map_err(|_| error(i.json))?;
    emit(output, i)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn geometry() -> Value {
        json!({"min_x":0,"min_y":0,"max_x":20,"max_y":10})
    }
    fn text() -> Value {
        json!({"observations":[{"component_id":{"kind":"action_button","instance":1},"allocated_rect":geometry(),"painted_rect":geometry(),"clip_rect":geometry(),"line_count":1,"truncated":false}],"audit":[],"coverage":{"measured_components":1,"attempted_components":1,"successful_components":1,"failed_components":0,"native_text_controls":2,"observed_native_controls":0,"excluded_categories":["ordinary_egui_labels","native_button_text"]}})
    }
    fn semantic() -> Value {
        json!({"pixels_per_point":1,"root":"application","nodes":[{"id":"application","parent":null,"role":"application","name":"Example","rect":geometry(),"enabled":true,"focused":false,"selected":false,"actions":[]}],"semantic_audit":[]})
    }
    #[test]
    fn coverage_keeps_failures_and_exclusions_without_claiming_a_pass() {
        let mut v = text();
        assert!(validate_text(&v).is_ok());
        v["coverage"]["attempted_components"] = json!(2);
        v["coverage"]["failed_components"] = json!(1);
        assert!(validate_text(&v).is_ok());
        let result = report("test", semantic(), v.clone(), json!({}));
        assert_eq!(result["coverage"]["value"]["failed_components"], 1);
        assert_eq!(
            result["validation"],
            "bounded_contract_valid_not_application_acceptance"
        );
        v["coverage"]["successful_components"] = json!(2);
        assert!(validate_text(&v).is_err());
        v["coverage"] = Value::Null;
        assert!(validate_text(&v).is_ok());
        assert_eq!(
            report("test", semantic(), v, json!({}))["coverage"]["status"],
            "unavailable"
        );
    }
    #[test]
    fn rejects_invalid_coverage_and_clipping_geometry() {
        for (field, value) in [
            ("measured_components", 0),
            ("attempted_components", 0),
            ("observed_native_controls", 3),
        ] {
            let mut v = text();
            v["coverage"][field] = json!(value);
            assert!(validate_text(&v).is_err());
        }
        let mut v = text();
        v["observations"][0]["clip_rect"]["max_x"] = json!(-1);
        assert!(validate_text(&v).is_err());
        let mut v = text();
        v["coverage"]["excluded_categories"] = json!([]);
        assert!(validate_text(&v).is_err());
        let mut v = text();
        v["observations"][0]["layout_error"] = json!("failed");
        assert!(validate_text(&v).is_err());
    }
    #[test]
    fn rejects_duplicate_missing_or_unbounded_semantic_nodes() {
        assert!(validate_semantic(&semantic()).is_ok());
        let mut v = semantic();
        let duplicate = v["nodes"][0].clone();
        v["nodes"].as_array_mut().unwrap().push(duplicate);
        assert!(validate_semantic(&v).is_err());
        let mut v = semantic();
        v["nodes"][0]["parent"] = json!("missing");
        assert!(validate_semantic(&v).is_err());
        let mut v = semantic();
        v["pixels_per_point"] = json!(0);
        assert!(validate_semantic(&v).is_err());
        let mut v = semantic();
        v["nodes"] = json!(vec![Value::Null; MAX_ITEMS + 1]);
        assert!(validate_semantic(&v).is_err());
        assert!(version(&json!({"schema_version":2})).is_err());
    }
    #[test]
    fn correlation_never_infers_atomic_capture_or_missing_identity() {
        assert_eq!(correlation(Some(4), Some(5))["status"], "differing");
        assert_eq!(
            correlation(Some(4), Some(4))["status"],
            "same_observed_frame"
        );
        assert!(correlation(Some(4), Some(4))["pixel_frame"].is_null());
        assert_eq!(correlation(Some(4), None)["status"], "unavailable");
        assert_eq!(
            report("test", semantic(), text(), json!({}))["source_revision"]["status"],
            "unavailable"
        );
    }
    fn png() -> Vec<u8> {
        let mut out = Vec::new();
        {
            let mut writer = png::Encoder::new(&mut out, 1, 1).write_header().unwrap();
            writer.write_image_data(&[1]).unwrap();
        }
        out
    }
    #[test]
    fn fully_decodes_png_and_rejects_truncation_corruption_and_size_limits() {
        let data = png();
        assert_eq!(png_info(&data).unwrap()["width"], 1);
        for n in [0, 8, 24, data.len() / 2, data.len() - 8] {
            assert!(png_info(&data[..n]).is_err(), "accepted truncation {n}");
        }
        let mut corrupt = data.clone();
        corrupt[40] ^= 0xff;
        assert!(png_info(&corrupt).is_err());
        assert!(png_info(&vec![0; MAX_BYTES + 1]).is_err());
    }
    #[test]
    fn malformed_truncated_and_oversized_files_are_rejected() {
        let dir =
            std::env::temp_dir().join(format!("lantern-polyorama-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("input.json");
        for data in [b"{\"nodes\":".as_slice(), b"not json"] {
            std::fs::write(&path, data).unwrap();
            assert!(read_json(&path).is_err());
        }
        std::fs::write(&path, vec![b' '; MAX_BYTES + 1]).unwrap();
        assert!(read_json(&path).is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn free_text_redaction_preserves_correlation_identifiers() {
        let mut v = json!({"sha256":"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef", "semantic":{"nodes":[{"id":"polyorama.dock.tab.1","name":"token=very-secret-value"}]}});
        let original = v["sha256"].clone();
        redact_owner_text(&mut v, RedactionMode::Redacted);
        assert_eq!(v["sha256"], original);
        assert_eq!(v["semantic"]["nodes"][0]["id"], "polyorama.dock.tab.1");
        assert!(
            !v["semantic"]["nodes"][0]["name"]
                .as_str()
                .unwrap()
                .contains("very-secret-value")
        );
    }
    #[test]
    fn fixed_hook_cannot_dispatch_internal_actions() {
        for action in [
            "select_story",
            "set_configuration",
            "request_test_repaint",
            "dispatch",
            "eval(",
        ] {
            assert!(!SNAPSHOT.contains(action));
        }
    }
}
