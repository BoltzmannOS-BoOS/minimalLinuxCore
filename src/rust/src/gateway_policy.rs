use std::collections::HashSet;
use std::env;
use std::io;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, ToSocketAddrs};

use crate::config;

pub struct ValidatedFetchUrl {
    pub url: String,
    pub host: String,
}

pub struct FetchPolicy {
    allowed_hosts: HashSet<String>,
}

impl FetchPolicy {
    pub fn new(hosts: &[&str]) -> io::Result<Self> {
        let mut allowed_hosts = HashSet::new();
        for host in hosts {
            let normalized = normalize_host(host)?;
            allowed_hosts.insert(normalized);
        }
        Ok(Self { allowed_hosts })
    }

    pub fn from_environment() -> io::Result<Self> {
        let raw = env::var("BOOS_FETCH_ALLOWLIST").unwrap_or_default();
        let hosts: Vec<&str> = raw
            .split(',')
            .map(str::trim)
            .filter(|host| !host.is_empty())
            .collect();
        Self::new(&hosts)
    }

    pub fn validate_url(&self, url: &str) -> io::Result<ValidatedFetchUrl> {
        if self.allowed_hosts.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "FETCH is disabled until BOOS_FETCH_ALLOWLIST is configured",
            ));
        }

        let remainder = url.strip_prefix("https://").ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "FETCH requires HTTPS")
        })?;
        let authority_end = remainder
            .find(|character| matches!(character, '/' | '?' | '#'))
            .unwrap_or(remainder.len());
        let authority = &remainder[..authority_end];
        if authority.is_empty() || authority.contains('@') || authority.starts_with('[') {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "FETCH URL has an invalid authority",
            ));
        }

        let host = match authority.rsplit_once(':') {
            Some((host, "443")) if !host.contains(':') => host,
            Some(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "FETCH permits only the default HTTPS port",
                ));
            }
            None => authority,
        };
        let host = normalize_host(host)?;
        if !self.allowed_hosts.contains(&host) {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "FETCH host is not allowlisted",
            ));
        }

        let clean_end = url
            .find(|character| matches!(character, '?' | '#'))
            .unwrap_or(url.len());
        Ok(ValidatedFetchUrl {
            url: url[..clean_end].to_string(),
            host,
        })
    }

    pub fn require_public_resolution(&self, url: &ValidatedFetchUrl) -> io::Result<()> {
        let addresses: Vec<_> = (url.host.as_str(), 443).to_socket_addrs()?.collect();
        if addresses.is_empty()
            || addresses
                .iter()
                .any(|address| !is_public_ip(address.ip()))
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "FETCH host does not resolve exclusively to public addresses",
            ));
        }
        Ok(())
    }
}

pub fn special_protocol_allowed(peer: IpAddr) -> bool {
    peer.is_loopback()
}

pub fn validate_session_id(session_id: &str) -> io::Result<()> {
    if config::is_valid_runtime_id(session_id) {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "SESSION ID must be a single 1-64 byte ASCII identifier",
        ))
    }
}

fn normalize_host(host: &str) -> io::Result<String> {
    let normalized = host.trim().trim_end_matches('.').to_ascii_lowercase();
    let valid_labels = normalized.len() <= 253
        && normalized.contains('.')
        && normalized.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        });
    if !valid_labels || normalized.parse::<IpAddr>().is_ok() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "FETCH allowlist entries must be public DNS hostnames",
        ));
    }
    Ok(normalized)
}

fn is_public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(address) => is_public_ipv4(address),
        IpAddr::V6(address) => is_public_ipv6(address),
    }
}

fn is_public_ipv4(address: Ipv4Addr) -> bool {
    let [a, b, c, _] = address.octets();
    !(a == 0
        || a == 10
        || a == 127
        || a >= 224
        || (a == 100 && (64..=127).contains(&b))
        || (a == 169 && b == 254)
        || (a == 172 && (16..=31).contains(&b))
        || (a == 192 && b == 0 && c == 0)
        || (a == 192 && b == 0 && c == 2)
        || (a == 192 && b == 168)
        || (a == 198 && (b == 18 || b == 19))
        || (a == 198 && b == 51 && c == 100)
        || (a == 203 && b == 0 && c == 113))
}

fn is_public_ipv6(address: Ipv6Addr) -> bool {
    if let Some(mapped) = address.to_ipv4_mapped() {
        return is_public_ipv4(mapped);
    }
    let segments = address.segments();
    !(address.is_unspecified()
        || address.is_loopback()
        || address.is_multicast()
        || (segments[0] & 0xfe00) == 0xfc00
        || (segments[0] & 0xffc0) == 0xfe80
        || (segments[0] == 0x2001 && segments[1] == 0x0db8))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};

    #[test]
    fn fetch_is_disabled_without_an_explicit_host_allowlist() {
        let policy = FetchPolicy::new(&[]).unwrap();

        assert!(policy.validate_url("https://example.com/docs").is_err());
    }

    #[test]
    fn fetch_allowlist_matches_the_complete_host_and_strips_query_data() {
        let policy = FetchPolicy::new(&["example.com"]).unwrap();

        let validated = policy
            .validate_url("https://example.com/docs?page=secret")
            .unwrap();

        assert_eq!(validated.url, "https://example.com/docs");
        assert_eq!(validated.host, "example.com");
        assert!(policy
            .validate_url("https://example.com.attacker.invalid/docs")
            .is_err());
    }

    #[test]
    fn fetch_allowlist_rejects_private_ip_literals() {
        assert!(FetchPolicy::new(&["127.0.0.1"]).is_err());
        assert!(FetchPolicy::new(&["10.0.2.2"]).is_err());
    }

    #[test]
    fn secret_backed_protocols_are_local_only() {
        assert!(special_protocol_allowed(IpAddr::V4(Ipv4Addr::LOCALHOST)));
        assert!(!special_protocol_allowed(IpAddr::V4(Ipv4Addr::new(
            10, 0, 2, 2
        ))));
    }

    #[test]
    fn session_ids_are_single_bounded_components() {
        assert!(validate_session_id("agent-session_1").is_ok());
        assert!(validate_session_id("../shared").is_err());
        assert!(validate_session_id("").is_err());
    }
}
