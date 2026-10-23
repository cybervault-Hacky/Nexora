//! Property-style validator tests (Phase 23G — plan §7: "fuzz/property tests
//! on validators"). Deterministic pseudo-random inputs (small LCG — no new
//! dependencies) sweep every auth validator: they must be total (never
//! panic), and acceptances must imply the documented bounds. Full `cargo-fuzz`
//! targets remain part of the toolchain-gated hardening track; these run in
//! the ordinary test harness.

use nexora_auth::config::GoogleSettings;
use nexora_auth::oauth::{is_loopback_redirect_uri, is_valid_code_verifier, is_valid_state_shape};
use nexora_auth::password::validate_new_password;
use nexora_auth::rate_limit::parse_rate_limit;
use nexora_auth::recovery::is_valid_reset_token_shape;
use nexora_auth::registration::validate_email;
use nexora_auth::session::is_valid_session_token_shape;

/// Deterministic LCG (Numerical Recipes constants) — reproducible corpus.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0
    }
    fn byte(&mut self) -> u8 {
        (self.next() >> 33) as u8
    }
}

const ALPHABET: &[u8] = b"abcXYZ019-._~@.+/% \t\n\x00\"'\\/:&=?#\xC3\xA9\x4E\x2D\x80\xFF";

fn corpus() -> Vec<String> {
    let mut rng = Lcg(2_026_101_923);
    let mut out = vec![
        String::new(),
        " ".to_string(),
        "\u{0}".to_string(),
        "a".repeat(500),
        "é".repeat(50),
        "中".repeat(30),
    ];
    for _ in 0..600 {
        let len = (rng.next() % 160) as usize;
        let mut s = String::new();
        for _ in 0..len {
            let index = (rng.byte() as usize) % ALPHABET.len();
            s.push(ALPHABET[index] as char);
        }
        out.push(s);
    }
    out
}

#[test]
fn validators_are_total_over_the_corpus() {
    let inputs = corpus();
    for input in &inputs {
        // Must not panic — any Ok/Err is acceptable.
        let _ = validate_email(input);
        let _ = validate_new_password(input);
        let _ = is_valid_session_token_shape(input);
        let _ = is_valid_reset_token_shape(input);
        let _ = is_valid_code_verifier(input);
        let _ = is_valid_state_shape(input);
        let _ = is_loopback_redirect_uri(input);
        let _ = parse_rate_limit(input);
        let _ = GoogleSettings::resolve(
            Some(input.as_str()),
            None,
            Some("http://127.0.0.1:1/callback"),
        );
        let _ = GoogleSettings::resolve(
            Some("cid"),
            Some(input.as_str()),
            Some("http://127.0.0.1:1/callback"),
        );
    }
}

#[test]
fn acceptances_imply_the_documented_bounds() {
    for input in corpus() {
        if let Ok(email) = validate_email(&input) {
            assert!(email.len() <= 254, "accepted email too long: {email:?}");
            assert_eq!(
                email,
                email.trim().to_lowercase(),
                "accepted email normalized"
            );
            assert!(email.contains('@'));
        }
        if validate_new_password(&input).is_ok() {
            assert!(
                (8..=128).contains(&input.len()),
                "accepted password outside 8–128: len {}",
                input.len()
            );
        }
        if is_valid_session_token_shape(&input) || is_valid_reset_token_shape(&input) {
            assert_eq!(input.len(), 64);
            assert!(input.chars().all(|c| c.is_ascii_hexdigit()));
        }
        if is_valid_code_verifier(&input) {
            assert!((43..=128).contains(&input.len()));
        }
        if is_valid_state_shape(&input) {
            assert!((32..=128).contains(&input.len()));
        }
        if let Ok(rule) = parse_rate_limit(&input) {
            assert!(rule.max_failures > 0);
            assert!(rule.window_secs > 0);
        }
    }
}

#[test]
fn boundary_values_behave_predictably() {
    // Password policy edges.
    assert!(validate_new_password(&"a".repeat(8)).is_ok());
    assert!(validate_new_password(&"a".repeat(128)).is_ok());
    assert!(validate_new_password(&"a".repeat(7)).is_err());
    assert!(validate_new_password(&"a".repeat(129)).is_err());
    // Token-shape edges.
    assert!(is_valid_session_token_shape(&"f".repeat(64)));
    assert!(!is_valid_session_token_shape(&"f".repeat(63)));
    assert!(is_valid_reset_token_shape(&"A".repeat(64)));
    assert!(!is_valid_reset_token_shape(&"g".repeat(64)));
    // PKCE edges.
    assert!(is_valid_code_verifier(&"a".repeat(43)));
    assert!(!is_valid_code_verifier(&"a".repeat(42)));
    assert!(!is_valid_code_verifier(&"a".repeat(129)));
}
