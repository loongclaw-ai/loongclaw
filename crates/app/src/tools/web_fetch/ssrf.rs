#[cfg(feature = "tool-webfetch")]
use std::net::IpAddr;

#[cfg(feature = "tool-webfetch")]
use ipnet::IpNet;
#[cfg(feature = "tool-webfetch")]
use url::Url;

#[cfg(feature = "tool-webfetch")]
const BLOCKED_HOST_SUFFIXES: &[&str] = &[".local", ".localdomain", ".internal", ".corp", ".home"];

#[cfg(feature = "tool-webfetch")]
pub(crate) struct ValidatedUrl {
    pub url: Url,
    pub normalized_host: String,
}

#[cfg(feature = "tool-webfetch")]
pub(crate) fn validate_url(raw: &str) -> Result<ValidatedUrl, String> {
    let url = Url::parse(raw).map_err(|error| format!("invalid_url: {error}"))?;

    let scheme = url.scheme();
    if scheme != "http" && scheme != "https" {
        return Err(format!(
            "invalid_url_scheme: only http/https are allowed, got `{scheme}`"
        ));
    }

    let host = url
        .host_str()
        .ok_or_else(|| "invalid_url: missing hostname".to_owned())?;
    let normalized_host = normalize_hostname(host)?;

    if normalized_host == "localhost"
        || normalized_host.starts_with("localhost.")
        || normalized_host.contains("xn--")
        || BLOCKED_HOST_SUFFIXES
            .iter()
            .any(|suffix| normalized_host.ends_with(suffix))
    {
        return Err(format!(
            "ssrf_blocked: hostname `{normalized_host}` is blocked"
        ));
    }

    if let Ok(ip) = normalized_host.parse::<IpAddr>()
        && is_special_use_address(ip)
    {
        return Err(format!(
            "ssrf_blocked: address `{normalized_host}` is special-use"
        ));
    }

    Ok(ValidatedUrl {
        url,
        normalized_host,
    })
}

#[cfg(feature = "tool-webfetch")]
pub(crate) fn normalize_hostname(host: &str) -> Result<String, String> {
    let trimmed = host.trim();
    if trimmed.is_empty() {
        return Err("invalid_url: empty hostname".to_owned());
    }
    if !trimmed.is_ascii() {
        return Err("ssrf_blocked: non-ascii hostname is not allowed".to_owned());
    }
    Ok(trimmed
        .trim_end_matches('.')
        .to_ascii_lowercase()
        .to_owned())
}

#[cfg(feature = "tool-webfetch")]
pub(crate) fn is_special_use_address(addr: IpAddr) -> bool {
    blocked_networks().iter().any(|net| net.contains(&addr))
}

#[cfg(feature = "tool-webfetch")]
pub(crate) fn validate_resolved_addresses(addrs: &[IpAddr]) -> Result<(), String> {
    if addrs.is_empty() {
        return Err("dns_resolution_failed: no IP addresses resolved".to_owned());
    }
    if addrs.iter().any(|addr| is_special_use_address(*addr)) {
        return Err("ssrf_blocked: DNS resolved to special-use address".to_owned());
    }
    Ok(())
}

#[cfg(feature = "tool-webfetch")]
fn blocked_networks() -> &'static [IpNet] {
    static NETWORKS: std::sync::OnceLock<Vec<IpNet>> = std::sync::OnceLock::new();
    NETWORKS.get_or_init(|| {
        [
            // IPv4 special-use ranges
            "0.0.0.0/8",
            "10.0.0.0/8",
            "100.64.0.0/10",
            "127.0.0.0/8",
            "169.254.0.0/16",
            "172.16.0.0/12",
            "192.0.0.0/24",
            "192.0.2.0/24",
            "192.168.0.0/16",
            "198.18.0.0/15",
            "198.51.100.0/24",
            "203.0.113.0/24",
            "224.0.0.0/4",
            "240.0.0.0/4",
            "255.255.255.255/32",
            // IPv6 special-use ranges
            "::/128",
            "::1/128",
            "::ffff:0:0/96",
            "64:ff9b::/96",
            "100::/64",
            "2001:db8::/32",
            "fc00::/7",
            "fe80::/10",
            "ff00::/8",
        ]
        .iter()
        .filter_map(|cidr| cidr.parse::<IpNet>().ok())
        .collect()
    })
}

#[cfg(all(test, feature = "tool-webfetch"))]
mod tests {
    use std::net::{IpAddr, Ipv4Addr};

    use super::{is_special_use_address, validate_resolved_addresses, validate_url};

    fn parse_ip(raw: &str) -> IpAddr {
        let parsed = raw.parse::<IpAddr>();
        assert!(parsed.is_ok(), "invalid test IP literal: {raw}");
        parsed.ok().unwrap_or(IpAddr::V4(Ipv4Addr::UNSPECIFIED))
    }

    #[test]
    fn validate_url_rejects_non_http_schemes() {
        let err = validate_url("file:///etc/passwd")
            .err()
            .unwrap_or_else(|| "missing expected error".to_owned());
        assert!(err.contains("invalid_url_scheme"));
    }

    #[test]
    fn validate_url_rejects_localhost_like_hosts() {
        let cases = [
            "http://localhost",
            "http://localhost.localdomain",
            "http://internal.service.local",
            "https://node.corp/path",
        ];

        for case in cases {
            let err = validate_url(case)
                .err()
                .unwrap_or_else(|| "missing expected error".to_owned());
            assert!(
                err.contains("ssrf_blocked"),
                "expected ssrf_blocked for `{case}`, got `{err}`"
            );
        }
    }

    #[test]
    fn validate_url_rejects_special_use_ip_literals() {
        let err = validate_url("https://127.0.0.1/test")
            .err()
            .unwrap_or_else(|| "missing expected error".to_owned());
        assert!(err.contains("ssrf_blocked"));
    }

    #[test]
    fn validate_url_accepts_public_host() {
        let validated = validate_url("https://example.com/");
        assert!(validated.is_ok(), "expected valid URL");
        if let Ok(validated) = validated {
            assert_eq!(validated.url.scheme(), "https");
            assert_eq!(validated.normalized_host, "example.com");
        }
    }

    #[test]
    fn special_use_ipv4_ranges_are_blocked() {
        let blocked = [
            "0.1.2.3",
            "10.0.0.1",
            "100.64.1.1",
            "127.0.0.1",
            "169.254.1.1",
            "172.16.1.1",
            "192.0.0.1",
            "192.0.2.12",
            "192.168.1.1",
            "198.18.0.10",
            "198.51.100.5",
            "203.0.113.8",
            "224.0.0.1",
            "240.0.0.1",
            "255.255.255.255",
        ];
        for ip in blocked {
            assert!(
                is_special_use_address(parse_ip(ip)),
                "expected blocked IPv4 address `{ip}`"
            );
        }
    }

    #[test]
    fn special_use_ipv6_ranges_are_blocked() {
        let blocked = [
            "::",
            "::1",
            "::ffff:127.0.0.1",
            "64:ff9b::1",
            "100::1",
            "2001:db8::1",
            "fc00::1",
            "fd00::1",
            "fe80::1",
            "ff00::1",
        ];
        for ip in blocked {
            assert!(
                is_special_use_address(parse_ip(ip)),
                "expected blocked IPv6 address `{ip}`"
            );
        }
    }

    #[test]
    fn ipv4_mapped_ipv6_bypass_is_blocked() {
        assert!(is_special_use_address(parse_ip("::ffff:127.0.0.1")));
    }

    #[test]
    fn dns_rebinding_like_resolution_is_blocked_when_any_ip_is_private() {
        let resolved = [parse_ip("93.184.216.34"), parse_ip("127.0.0.1")];
        let err = validate_resolved_addresses(&resolved)
            .err()
            .unwrap_or_else(|| "missing expected error".to_owned());
        assert!(err.contains("ssrf_blocked"));
    }

    #[test]
    fn dns_resolution_all_public_is_allowed() {
        let resolved = [parse_ip("93.184.216.34"), parse_ip("1.1.1.1")];
        let result = validate_resolved_addresses(&resolved);
        assert!(result.is_ok());
    }

    #[test]
    fn unicode_homograph_hostname_is_rejected() {
        let err = validate_url("https://exаmple.com")
            .err()
            .unwrap_or_else(|| "missing expected error".to_owned());
        assert!(err.contains("ssrf_blocked"));
    }

    #[test]
    fn public_addresses_are_allowed() {
        let allowed = ["1.1.1.1", "8.8.8.8", "2606:4700:4700::1111"];
        for ip in allowed {
            assert!(
                !is_special_use_address(parse_ip(ip)),
                "expected public address `{ip}` to be allowed"
            );
        }
    }
}
