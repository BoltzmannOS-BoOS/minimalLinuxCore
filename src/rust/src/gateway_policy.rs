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
