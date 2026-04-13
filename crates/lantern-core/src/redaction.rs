use url::{Host, Url};

pub const TITLE_TEXT_LIMIT: usize = 120;
pub const DOM_TEXT_LIMIT: usize = 500;
pub const DOM_ATTRIBUTE_VALUE_LIMIT: usize = 200;
pub const DOM_CLASS_TOKEN_LIMIT: usize = 12;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RedactionMode {
    Redacted,
    Unredacted,
}

impl RedactionMode {
    pub fn from_no_redact(no_redact: bool) -> Self {
        if no_redact {
            Self::Unredacted
        } else {
            Self::Redacted
        }
    }
}

pub fn sanitize_title(value: &str, mode: RedactionMode) -> String {
    sanitize_text(value, TITLE_TEXT_LIMIT, mode)
}

pub fn sanitize_dom_text(value: &str, mode: RedactionMode) -> String {
    sanitize_text(value, DOM_TEXT_LIMIT, mode)
}

pub fn sanitize_dom_attribute(
    name: &str,
    value: &str,
    mode: RedactionMode,
) -> Option<(String, String)> {
    let normalized_name = name.trim().to_ascii_lowercase();

    if normalized_name.is_empty()
        || normalized_name.starts_with("on")
        || is_sensitive_attribute_name(&normalized_name)
    {
        return None;
    }

    let sanitized = match normalized_name.as_str() {
        "id" | "role" | "type" | "data-testid" | "data-test" | "data-cy" => {
            sanitize_text(value, DOM_ATTRIBUTE_VALUE_LIMIT, mode)
        }
        "class" => sanitize_class_attribute(value, mode),
        "aria-label" | "aria-labelledby" | "alt" | "title" => {
            sanitize_text(value, DOM_ATTRIBUTE_VALUE_LIMIT, mode)
        }
        "name" => {
            if looks_sensitive(value) {
                return None;
            }
            sanitize_text(value, DOM_ATTRIBUTE_VALUE_LIMIT, mode)
        }
        "href" | "src" => sanitize_url(value, mode)?,
        _ => return None,
    };

    Some((normalized_name, sanitized))
}

pub fn sanitize_text(value: &str, limit: usize, mode: RedactionMode) -> String {
    let normalized = normalize_browser_text(value);

    if mode == RedactionMode::Unredacted {
        return normalized;
    }

    truncate_scalars(&normalized, limit)
}

pub fn truncate_scalars(value: &str, limit: usize) -> String {
    if value.chars().count() <= limit {
        return value.to_owned();
    }

    value.chars().take(limit).collect::<String>() + "..."
}

pub fn sanitize_url(value: &str, mode: RedactionMode) -> Option<String> {
    if mode == RedactionMode::Unredacted {
        return Some(value.to_owned());
    }

    let mut url = Url::parse(value).ok()?;
    let scheme = url.scheme().to_owned();
    let host = display_host(url.host()?);
    let port = url.port();
    let path = sanitized_path(url.path());
    url.set_username("").ok()?;
    url.set_password(None).ok()?;
    url.set_query(None);
    url.set_fragment(None);

    let mut output = format!("{scheme}://{host}");
    if let Some(port) = port.filter(|port| !is_default_port(&scheme, *port)) {
        output.push_str(&format!(":{port}"));
    }
    output.push_str(&path);
    Some(output)
}

pub fn is_sensitive_attribute_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    [
        "token",
        "secret",
        "password",
        "passwd",
        "auth",
        "session",
        "cookie",
        "key",
        "jwt",
        "credential",
        "csrf",
        "nonce",
        "signature",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

fn normalize_browser_text(value: &str) -> String {
    let normalized = value
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .chars()
        .map(|ch| {
            if ch.is_ascii_control() && ch != '\n' && ch != '\t' {
                ' '
            } else {
                ch
            }
        })
        .collect::<String>();

    collapse_excess_blank_lines(&normalized)
}

fn collapse_excess_blank_lines(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut consecutive_newlines = 0;

    for ch in value.chars() {
        if ch == '\n' {
            consecutive_newlines += 1;
            if consecutive_newlines <= 3 {
                output.push(ch);
            }
        } else {
            consecutive_newlines = 0;
            output.push(ch);
        }
    }

    output
}

fn sanitize_class_attribute(value: &str, mode: RedactionMode) -> String {
    let normalized = normalize_browser_text(value);
    let bounded_tokens = normalized
        .split_whitespace()
        .take(DOM_CLASS_TOKEN_LIMIT)
        .collect::<Vec<_>>()
        .join(" ");

    if mode == RedactionMode::Unredacted {
        bounded_tokens
    } else {
        truncate_scalars(&bounded_tokens, DOM_ATTRIBUTE_VALUE_LIMIT)
    }
}

fn sanitized_path(path: &str) -> String {
    if path.is_empty() || path == "/" {
        return "/".to_owned();
    }

    let segments = path
        .split('/')
        .filter(|segment| !segment.is_empty())
        .scan(false, |redact_next, segment| {
            let redact = *redact_next || is_sensitive_segment(segment);
            *redact_next = is_sensitive_label(segment);
            Some(if redact { ":redacted" } else { segment })
        })
        .collect::<Vec<_>>();

    format!("/{}", segments.join("/"))
}

fn is_default_port(scheme: &str, port: u16) -> bool {
    matches!(
        (scheme, port),
        ("http", 80) | ("https", 443) | ("ws", 80) | ("wss", 443)
    )
}

fn display_host(host: Host<&str>) -> String {
    match host {
        Host::Domain(domain) => domain.to_owned(),
        Host::Ipv4(addr) => addr.to_string(),
        Host::Ipv6(addr) => format!("[{addr}]"),
    }
}

fn is_sensitive_segment(segment: &str) -> bool {
    segment.chars().count() > 64
        || segment.contains('@')
        || segment.starts_with("eyJ")
        || is_uuid(segment)
        || is_long_hex(segment)
        || is_long_token(segment)
}

fn is_sensitive_label(segment: &str) -> bool {
    matches!(
        segment.to_ascii_lowercase().as_str(),
        "token"
            | "key"
            | "secret"
            | "session"
            | "auth"
            | "password"
            | "passwd"
            | "invite"
            | "reset"
            | "code"
            | "otp"
            | "jwt"
            | "access_token"
            | "refresh_token"
    )
}

fn looks_sensitive(value: &str) -> bool {
    let value = value.trim();
    is_sensitive_label(value) || is_sensitive_segment(value)
}

fn is_uuid(segment: &str) -> bool {
    let bytes = segment.as_bytes();
    bytes.len() == 36
        && [8, 13, 18, 23].iter().all(|index| bytes[*index] == b'-')
        && bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| [8, 13, 18, 23].contains(&index) || byte.is_ascii_hexdigit())
}

fn is_long_hex(segment: &str) -> bool {
    segment.len() >= 24 && segment.chars().all(|ch| ch.is_ascii_hexdigit())
}

fn is_long_token(segment: &str) -> bool {
    segment.len() >= 32
        && segment
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '=' | '+'))
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
    fn unredacted_url_preserves_full_input() {
        let url = sanitize_url(
            "https://user:pass@example.test/path?token=secret#frag",
            RedactionMode::Unredacted,
        );

        assert_eq!(
            url.as_deref(),
            Some("https://user:pass@example.test/path?token=secret#frag")
        );
    }

    #[test]
    fn dom_attribute_policy_keeps_safe_attributes_and_shapes_urls() {
        assert_eq!(
            sanitize_dom_attribute(
                "HREF",
                "https://example.test/reset/abcdabcdabcdabcdabcdabcd?token=secret",
                RedactionMode::Redacted
            ),
            Some((
                "href".to_owned(),
                "https://example.test/reset/:redacted".to_owned()
            ))
        );
        assert_eq!(
            sanitize_dom_attribute("onclick", "steal()", RedactionMode::Redacted),
            None
        );
        assert_eq!(
            sanitize_dom_attribute("data-token", "secret", RedactionMode::Redacted),
            None
        );
    }

    #[test]
    fn class_attribute_is_token_bounded() {
        let classes = (0..20).map(|index| format!("c{index}")).collect::<Vec<_>>();

        let (_, value) =
            sanitize_dom_attribute("class", &classes.join(" "), RedactionMode::Unredacted)
                .expect("class is safe");

        assert_eq!(value.split_whitespace().count(), DOM_CLASS_TOKEN_LIMIT);
    }
}
