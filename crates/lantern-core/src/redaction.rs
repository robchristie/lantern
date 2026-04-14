use url::{Host, Url};

pub const TITLE_TEXT_LIMIT: usize = 120;
pub const CONSOLE_MESSAGE_LIMIT: usize = 500;
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
    if mode == RedactionMode::Unredacted {
        return sanitize_text(value, DOM_TEXT_LIMIT, mode);
    }

    sanitize_browser_snippet(value, DOM_TEXT_LIMIT, mode)
}

pub fn sanitize_console_message(value: &str, mode: RedactionMode) -> String {
    if mode == RedactionMode::Unredacted {
        return sanitize_text(value, CONSOLE_MESSAGE_LIMIT, mode);
    }

    sanitize_browser_snippet(value, CONSOLE_MESSAGE_LIMIT, mode)
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

fn sanitize_browser_snippet(value: &str, limit: usize, mode: RedactionMode) -> String {
    let normalized = sanitize_text(value, usize::MAX, mode);
    let redacted = redact_sensitive_message_tokens(&redact_url_tokens(&normalized));
    truncate_scalars(&redacted, limit)
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

fn redact_url_tokens(value: &str) -> String {
    value
        .split_whitespace()
        .map(redact_url_token)
        .collect::<Vec<_>>()
        .join(" ")
}

fn redact_url_token(token: &str) -> String {
    let Some(start) = find_url_start(token) else {
        return token.to_owned();
    };
    let (prefix, rest) = token.split_at(start);
    let suffix_len = url_token_suffix_len(rest);
    let core_end = rest.len().saturating_sub(suffix_len);
    let (core, suffix) = rest.split_at(core_end);

    match sanitize_url(core, RedactionMode::Redacted) {
        Some(shape) => format!("{prefix}{shape}{suffix}"),
        None => format!("{prefix}:redacted-url{suffix}"),
    }
}

fn find_url_start(token: &str) -> Option<usize> {
    match (token.find("http://"), token.find("https://")) {
        (Some(http), Some(https)) => Some(http.min(https)),
        (Some(http), None) => Some(http),
        (None, Some(https)) => Some(https),
        (None, None) => None,
    }
}

fn url_token_suffix_len(token: &str) -> usize {
    token
        .chars()
        .rev()
        .take_while(|ch| matches!(ch, '.' | ',' | ';' | ':' | ')' | ']' | '}' | '"' | '\''))
        .map(char::len_utf8)
        .sum::<usize>()
}

fn redact_sensitive_message_tokens(value: &str) -> String {
    let mut redact_next_bearer_value = false;

    value
        .split_whitespace()
        .map(|token| {
            let lower = token.to_ascii_lowercase();
            let bare = trim_token_punctuation(&lower);

            if redact_next_bearer_value {
                redact_next_bearer_value = false;
                return redact_value_token(token);
            }

            if bare == "bearer" {
                redact_next_bearer_value = true;
                return token.to_owned();
            }

            if contains_sensitive_assignment(&lower) {
                redact_assignment_token(token)
            } else if looks_like_standalone_secret(bare) {
                redact_value_token(token)
            } else {
                token.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn contains_sensitive_assignment(lower: &str) -> bool {
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
        "authorization",
        "csrf",
        "nonce",
        "signature",
        "access_token",
        "refresh_token",
    ]
    .iter()
    .any(|needle| {
        lower.starts_with(&format!("{needle}="))
            || lower.starts_with(&format!("{needle}:"))
            || lower.contains(&format!("_{needle}="))
            || lower.contains(&format!("-{needle}="))
    })
}

fn redact_assignment_token(token: &str) -> String {
    if let Some(index) = token.find('=') {
        let (key, _) = token.split_at(index + 1);
        return format!("{key}:redacted");
    }

    if let Some(index) = token.find(':') {
        let (key, _) = token.split_at(index);
        return format!("{key}:redacted");
    }

    ":redacted".to_owned()
}

fn redact_value_token(token: &str) -> String {
    let suffix_len = token
        .chars()
        .rev()
        .take_while(|ch| matches!(ch, '.' | ',' | ';' | ':' | ')' | ']' | '}' | '"' | '\''))
        .map(char::len_utf8)
        .sum::<usize>();
    let core_end = token.len().saturating_sub(suffix_len);
    let (_, suffix) = token.split_at(core_end);
    format!(":redacted{suffix}")
}

fn trim_token_punctuation(value: &str) -> &str {
    value.trim_matches(|ch: char| {
        !ch.is_ascii_alphanumeric() && !matches!(ch, '_' | '-' | '=' | '+')
    })
}

fn looks_like_standalone_secret(value: &str) -> bool {
    value.starts_with("eyj")
        || looks_like_jwt_token(value)
        || is_long_hex(value)
        || is_long_token(value)
}

fn looks_like_jwt_token(value: &str) -> bool {
    let mut segments = value.split('.');
    let Some(first) = segments.next() else {
        return false;
    };
    let Some(second) = segments.next() else {
        return false;
    };

    !first.is_empty()
        && !second.is_empty()
        && value.chars().count() >= 24
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '=' | '.'))
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
    fn console_messages_redact_urls_and_sensitive_assignments() {
        let message = sanitize_console_message(
            "failed https://user:pass@example.test/reset/abcdabcdabcdabcdabcdabcd?token=secret#frag token=secret",
            RedactionMode::Redacted,
        );

        assert_eq!(
            message,
            "failed https://example.test/reset/:redacted token=:redacted"
        );
    }

    #[test]
    fn console_messages_redact_embedded_url_assignments() {
        let message = sanitize_console_message(
            "failed url=https://user:pass@example.test/reset/abcdabcdabcdabcdabcdabcd?token=secret#frag next=(https://example.test/private/session/abc123?secret=yes),",
            RedactionMode::Redacted,
        );

        assert_eq!(
            message,
            "failed url=https://example.test/reset/:redacted next=(https://example.test/private/session/:redacted),"
        );
        assert!(!message.contains("user:pass"));
        assert!(!message.contains("token=secret"));
        assert!(!message.contains("#frag"));
    }

    #[test]
    fn console_messages_redact_bearer_and_jwt_like_values() {
        let message = sanitize_console_message(
            "Authorization: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.deadbeef",
            RedactionMode::Redacted,
        );

        assert_eq!(message, "Authorization:redacted Bearer :redacted");
        assert!(!message.contains("eyJ"));
        assert!(!message.contains("deadbeef"));
    }

    #[test]
    fn dom_text_redacts_url_assignments_and_credential_like_values_by_default() {
        let text = sanitize_dom_text(
            "Profile token=secret https://example.test/session/abcdef123456abcdef123456abcdef12?debug=1 bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9",
            RedactionMode::Redacted,
        );

        assert_eq!(
            text,
            "Profile token=:redacted https://example.test/session/:redacted bearer :redacted"
        );
    }

    #[test]
    fn dom_text_redacts_standalone_jwt_like_values_with_periods() {
        let text = sanitize_dom_text(
            "session eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.payload.signature done",
            RedactionMode::Redacted,
        );

        assert_eq!(text, "session :redacted done");
        assert!(!text.contains("eyJ"));
        assert!(!text.contains("payload"));
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
