//! Binding-behaviour tests: loopback by default, explicit opt-in required for
//! anything else. No external network access is used.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use nexora_auth::config::{AuthConfig, ALLOW_NON_LOOPBACK_VAR, DEFAULT_BIND};

#[tokio::test]
async fn default_bind_is_loopback_and_bindable() {
    let config = AuthConfig::resolve(None, None).unwrap();
    assert_eq!(config.bind, DEFAULT_BIND.parse::<SocketAddr>().unwrap());
    assert!(config.bind.ip().is_loopback());
    assert!(!config.allow_non_loopback);

    // Prove loopback binds work locally without any external network.
    let listener =
        tokio::net::TcpListener::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0))
            .await
            .unwrap();
    assert!(listener.local_addr().unwrap().ip().is_loopback());
}

#[tokio::test]
async fn non_loopback_bind_is_restricted_without_opt_in() {
    for raw in ["0.0.0.0:8787", "192.168.0.10:8787", "[::]:8787"] {
        let error = AuthConfig::resolve(Some(raw), None).unwrap_err();
        assert!(
            error.message().contains(ALLOW_NON_LOOPBACK_VAR),
            "case: {raw}"
        );
    }
}

#[tokio::test]
async fn non_loopback_bind_is_allowed_only_with_explicit_opt_in() {
    let refused = AuthConfig::resolve(Some("0.0.0.0:8787"), Some("false")).unwrap_err();
    assert!(refused.message().contains(ALLOW_NON_LOOPBACK_VAR));

    let allowed = AuthConfig::resolve(Some("0.0.0.0:8787"), Some("true")).unwrap();
    assert!(allowed.allow_non_loopback);
    assert!(!allowed.bind.ip().is_loopback());
}
