use std::net::IpAddr;

use url::{Host, Url};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndpointSource {
    Flag,
    Environment,
}

impl EndpointSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Flag => "flag",
            Self::Environment => "env",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedEndpoint {
    pub source: EndpointSource,
    pub display: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EndpointResolutionError {
    Missing,
    Invalid {
        source: EndpointSource,
        reason: EndpointInvalidReason,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndpointInvalidReason {
    Parse,
    UnsupportedScheme,
    Credentials,
    QueryOrFragment,
    MissingPort,
    MissingHost,
    NonLocalHost,
}

pub fn resolve_endpoint(
    flag_endpoint: Option<&str>,
    env_endpoint: Option<&str>,
) -> Result<ResolvedEndpoint, EndpointResolutionError> {
    match (flag_endpoint, env_endpoint) {
        (Some(endpoint), _) => validate_endpoint(EndpointSource::Flag, endpoint),
        (None, Some(endpoint)) => validate_endpoint(EndpointSource::Environment, endpoint),
        (None, None) => Err(EndpointResolutionError::Missing),
    }
}

fn validate_endpoint(
    source: EndpointSource,
    endpoint: &str,
) -> Result<ResolvedEndpoint, EndpointResolutionError> {
    let url = Url::parse(endpoint).map_err(|_| invalid(source, EndpointInvalidReason::Parse))?;

    if url.scheme() != "http" {
        return Err(invalid(source, EndpointInvalidReason::UnsupportedScheme));
    }

    if has_userinfo(endpoint) || !url.username().is_empty() || url.password().is_some() {
        return Err(invalid(source, EndpointInvalidReason::Credentials));
    }

    if url.query().is_some() || url.fragment().is_some() {
        return Err(invalid(source, EndpointInvalidReason::QueryOrFragment));
    }

    let host = url
        .host()
        .ok_or_else(|| invalid(source, EndpointInvalidReason::MissingHost))?;

    if !is_local_host(&host) {
        return Err(invalid(source, EndpointInvalidReason::NonLocalHost));
    }

    let port = url
        .port()
        .ok_or_else(|| invalid(source, EndpointInvalidReason::MissingPort))?;

    Ok(ResolvedEndpoint {
        source,
        display: display_endpoint(&url, host, port),
    })
}

fn invalid(source: EndpointSource, reason: EndpointInvalidReason) -> EndpointResolutionError {
    EndpointResolutionError::Invalid { source, reason }
}

fn has_userinfo(endpoint: &str) -> bool {
    let Some((_, after_scheme)) = endpoint.split_once("://") else {
        return false;
    };
    let authority_end = after_scheme
        .find(['/', '?', '#'])
        .unwrap_or(after_scheme.len());
    after_scheme[..authority_end].contains('@')
}

fn is_local_host(host: &Host<&str>) -> bool {
    match host {
        Host::Domain(domain) => domain.eq_ignore_ascii_case("localhost"),
        Host::Ipv4(addr) => IpAddr::V4(*addr).is_loopback(),
        Host::Ipv6(addr) => IpAddr::V6(*addr).is_loopback(),
    }
}

fn display_endpoint(url: &Url, host: Host<&str>, port: u16) -> String {
    let host = match host {
        Host::Domain(domain) => domain.to_owned(),
        Host::Ipv4(addr) => addr.to_string(),
        Host::Ipv6(addr) => format!("[{addr}]"),
    };

    let path = match url.path() {
        "" | "/" => "",
        path => path,
    };

    format!("http://{host}:{port}{path}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_missing_when_no_supported_source_is_set() {
        assert_eq!(
            resolve_endpoint(None, None),
            Err(EndpointResolutionError::Missing)
        );
    }

    #[test]
    fn flag_endpoint_wins_over_environment_endpoint() {
        let endpoint = resolve_endpoint(
            Some("http://127.0.0.1:9222/from-flag"),
            Some("http://localhost:9333/from-env"),
        )
        .expect("endpoint should resolve");

        assert_eq!(endpoint.source, EndpointSource::Flag);
        assert_eq!(endpoint.display, "http://127.0.0.1:9222/from-flag");
    }

    #[test]
    fn uses_environment_endpoint_when_flag_is_absent() {
        let endpoint =
            resolve_endpoint(None, Some("http://localhost:9222")).expect("endpoint should resolve");

        assert_eq!(endpoint.source, EndpointSource::Environment);
        assert_eq!(endpoint.display, "http://localhost:9222");
    }

    #[test]
    fn accepts_loopback_ipv6_with_path_prefix() {
        let endpoint = resolve_endpoint(None, Some("http://[::1]:9222/devtools"))
            .expect("endpoint should resolve");

        assert_eq!(endpoint.source, EndpointSource::Environment);
        assert_eq!(endpoint.display, "http://[::1]:9222/devtools");
    }

    #[test]
    fn rejects_unsupported_endpoint_shapes() {
        let cases = [
            (
                "not-a-url",
                EndpointInvalidReason::Parse,
                "plain strings are invalid",
            ),
            (
                "https://127.0.0.1:9222",
                EndpointInvalidReason::UnsupportedScheme,
                "https is out of scope",
            ),
            (
                "ws://127.0.0.1:9222",
                EndpointInvalidReason::UnsupportedScheme,
                "websocket endpoints are out of scope",
            ),
            (
                "http://user:pass@127.0.0.1:9222",
                EndpointInvalidReason::Credentials,
                "credentials must not be accepted",
            ),
            (
                "http://127.0.0.1:9222/json?x=1",
                EndpointInvalidReason::QueryOrFragment,
                "queries are invalid for a base endpoint",
            ),
            (
                "http://127.0.0.1:9222/json#target",
                EndpointInvalidReason::QueryOrFragment,
                "fragments are invalid for a base endpoint",
            ),
            (
                "http://example.com:9222",
                EndpointInvalidReason::NonLocalHost,
                "remote hosts are out of scope",
            ),
            (
                "http://0.0.0.0:9222",
                EndpointInvalidReason::NonLocalHost,
                "wildcard bind addresses are not client endpoints",
            ),
            (
                "http://127.0.0.1",
                EndpointInvalidReason::MissingPort,
                "CDP endpoint port must be explicit",
            ),
            (
                "unix:///tmp/chrome.sock",
                EndpointInvalidReason::UnsupportedScheme,
                "unix sockets are out of scope",
            ),
        ];

        for (endpoint, reason, label) in cases {
            assert_eq!(
                resolve_endpoint(Some(endpoint), None),
                Err(EndpointResolutionError::Invalid {
                    source: EndpointSource::Flag,
                    reason,
                }),
                "{label}"
            );
        }
    }
}
