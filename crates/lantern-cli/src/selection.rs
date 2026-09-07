use crate::error::{
    CliError, TARGET_AMBIGUOUS_HINT, TARGET_AMBIGUOUS_MESSAGE, TARGET_ID_NOT_FOUND_HINT,
    TARGET_ID_NOT_FOUND_MESSAGE, TARGET_NOT_FOUND_HINT, TARGET_NOT_FOUND_MESSAGE,
};
use crate::output::{CLI_SCHEMA_VERSION, escape_human, write_json};
use lantern_core::cdp::TargetInfo;
use lantern_core::redaction::{RedactionMode, sanitize_title, sanitize_url};
use serde::Serialize;

pub(crate) fn write_targets(
    targets: Vec<TargetInfo>,
    json: bool,
    no_redact: bool,
) -> Result<(), CliError> {
    let targets = ordered_targets(targets);
    let output_targets: Vec<TargetOutput> = targets
        .into_iter()
        .map(|target| target_output(target, no_redact))
        .collect();

    if json {
        write_json(&TargetsOutput {
            schema_version: CLI_SCHEMA_VERSION,
            command: "targets",
            ok: true,
            targets: output_targets,
        })?;
        return Ok(());
    }

    for target in output_targets {
        let attached = match target.attached {
            Some(true) => "attached",
            Some(false) => "detached",
            None => "attached=null",
        };
        let title = target.title.as_deref().unwrap_or("null");
        let url_shape = target.url_shape.as_deref().unwrap_or("null");

        println!(
            "{} {} {attached} title=\"{}\" url={url_shape}",
            short_target_id(&target.id),
            target.kind,
            escape_human(title)
        );
    }

    Ok(())
}

pub(crate) fn write_page(target: TargetInfo, json: bool, no_redact: bool) -> Result<(), CliError> {
    let target = target_output(target, no_redact);
    let page = PageOutput {
        target_id: target.id,
        title: target.title,
        url_shape: target.url_shape,
        loading_state: None,
    };

    if json {
        write_json(&PageCommandOutput {
            schema_version: CLI_SCHEMA_VERSION,
            command: "page",
            ok: true,
            page,
        })?;
        return Ok(());
    }

    println!(
        "page: {} title=\"{}\" url={} loading={}",
        short_target_id(&page.target_id),
        escape_human(page.title.as_deref().unwrap_or("null")),
        page.url_shape.as_deref().unwrap_or("null"),
        page.loading_state.as_deref().unwrap_or("null")
    );
    Ok(())
}

fn ordered_targets(mut targets: Vec<TargetInfo>) -> Vec<TargetInfo> {
    targets.sort_by_key(|target| (target.kind != "page", target.attached != Some(true)));
    targets
}

pub(crate) fn select_page_target(
    targets: Vec<TargetInfo>,
    target_id: Option<&str>,
) -> Result<TargetInfo, CliError> {
    if let Some(target_id) = target_id {
        return targets
            .into_iter()
            .find(|target| target.id == target_id && target.kind == "page")
            .ok_or_else(|| {
                CliError::runtime(
                    false,
                    "target_not_found",
                    TARGET_ID_NOT_FOUND_MESSAGE,
                    TARGET_ID_NOT_FOUND_HINT,
                )
            });
    }

    let page_targets: Vec<TargetInfo> = targets
        .into_iter()
        .filter(|target| target.kind == "page")
        .collect();

    match page_targets.len() {
        0 => Err(CliError::runtime(
            false,
            "target_not_found",
            TARGET_NOT_FOUND_MESSAGE,
            TARGET_NOT_FOUND_HINT,
        )),
        1 => Ok(page_targets.into_iter().next().expect("one page target")),
        _ => {
            let attached: Vec<TargetInfo> = page_targets
                .iter()
                .filter(|target| target.attached == Some(true))
                .cloned()
                .collect();
            if attached.len() == 1 {
                Ok(attached.into_iter().next().expect("one attached target"))
            } else {
                Err(
                    CliError::usage(false, TARGET_AMBIGUOUS_MESSAGE, TARGET_AMBIGUOUS_HINT)
                        .with_code("target_ambiguous"),
                )
            }
        }
    }
}

fn target_output(target: TargetInfo, no_redact: bool) -> TargetOutput {
    let mode = RedactionMode::from_no_redact(no_redact);
    TargetOutput {
        id: target.id,
        kind: target.kind,
        title: target.title.map(|title| sanitize_title(&title, mode)),
        url_shape: target.url.and_then(|url| sanitize_url(&url, mode)),
        attached: target.attached,
    }
}

pub(crate) fn short_target_id(id: &str) -> &str {
    id.get(..8).unwrap_or(id)
}

#[derive(Debug, Serialize)]
struct TargetsOutput {
    schema_version: u8,
    command: &'static str,
    ok: bool,
    targets: Vec<TargetOutput>,
}

#[derive(Debug, Serialize)]
struct TargetOutput {
    id: String,
    #[serde(rename = "type")]
    kind: String,
    title: Option<String>,
    url_shape: Option<String>,
    attached: Option<bool>,
}

#[derive(Debug, Serialize)]
struct PageCommandOutput {
    schema_version: u8,
    command: &'static str,
    ok: bool,
    page: PageOutput,
}

#[derive(Debug, Serialize)]
struct PageOutput {
    target_id: String,
    title: Option<String>,
    url_shape: Option<String>,
    loading_state: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_shape_omits_query_and_redacts_sensitive_path_segments() {
        let url = sanitize_url(
            "https://user:pass@example.test/reset/4a7f9c0e2d1b4c6a8e9f0123456789ab?token=secret#frag",
            RedactionMode::Redacted,
        );

        assert_eq!(url.as_deref(), Some("https://example.test/reset/:redacted"));
    }

    #[test]
    fn page_selection_uses_single_attached_page_when_multiple_pages_exist() {
        let selected = select_page_target(
            vec![
                target("PAGE_A", "page", Some(false)),
                target("PAGE_B", "page", Some(true)),
                target("WORKER", "service_worker", Some(true)),
            ],
            None,
        )
        .expect("one attached page should be selected");

        assert_eq!(selected.id, "PAGE_B");
    }

    #[test]
    fn page_selection_uses_exact_target_id_when_provided() {
        let selected = select_page_target(
            vec![
                target("PAGE_A", "page", Some(false)),
                target("PAGE_B", "page", Some(true)),
            ],
            Some("PAGE_A"),
        )
        .expect("requested page target should be selected");

        assert_eq!(selected.id, "PAGE_A");
    }

    #[test]
    fn page_selection_rejects_exact_target_id_for_non_page_target() {
        let error = select_page_target(
            vec![
                target("PAGE_A", "page", Some(true)),
                target("WORKER", "service_worker", Some(true)),
            ],
            Some("WORKER"),
        )
        .expect_err("non-page target id should not match");

        assert_eq!(error.exit_code, 1);
        assert_eq!(error.code, "target_not_found");
        assert_eq!(error.message, TARGET_ID_NOT_FOUND_MESSAGE);
    }

    #[test]
    fn page_selection_reports_ambiguous_pages_as_usage_error() {
        let error = select_page_target(
            vec![
                target("PAGE_A", "page", Some(false)),
                target("PAGE_B", "page", Some(false)),
            ],
            None,
        )
        .expect_err("multiple detached pages are ambiguous");

        assert_eq!(error.exit_code, 2);
        assert_eq!(error.code, "target_ambiguous");
    }

    fn target(id: &str, kind: &str, attached: Option<bool>) -> TargetInfo {
        TargetInfo {
            id: id.to_owned(),
            kind: kind.to_owned(),
            title: Some(format!("Title {id}")),
            url: Some(format!("https://example.test/{id}")),
            attached,
            browser_context_id: None,
            web_socket_debugger_url: None,
        }
    }
}
