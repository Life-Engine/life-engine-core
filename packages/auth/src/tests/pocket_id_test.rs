//! Unit tests for PocketIdProvider JWT validation.
//!
//! Uses a test Ed25519 keypair to generate JWTs and wiremock to mock
//! the OIDC discovery and JWKS endpoints.

use chrono::Utc;
use ed25519_dalek::pkcs8::EncodePrivateKey;
use ed25519_dalek::SigningKey;
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use serde::Serialize;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::config::AuthConfig;
use crate::error::AuthError;
use crate::handlers::validate::PocketIdProvider;
use crate::AuthProvider;

/// JWT claims for test token generation.
#[derive(Debug, Serialize)]
struct TestClaims {
    sub: Option<String>,
    iss: Option<String>,
    aud: Option<String>,
    exp: Option<u64>,
    scope: Option<serde_json::Value>,
}

/// Generate a test Ed25519 keypair.
fn test_keypair() -> (SigningKey, ed25519_dalek::VerifyingKey) {
    let secret = SigningKey::from_bytes(&[
        1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24,
        25, 26, 27, 28, 29, 30, 31, 32,
    ]);
    let public = secret.verifying_key();
    (secret, public)
}

/// Build a JWK JSON object for an Ed25519 public key.
fn ed25519_jwk(public_key: &ed25519_dalek::VerifyingKey, kid: &str) -> serde_json::Value {
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine;
    let x = URL_SAFE_NO_PAD.encode(public_key.as_bytes());
    serde_json::json!({
        "kty": "OKP",
        "crv": "Ed25519",
        "kid": kid,
        "alg": "EdDSA",
        "x": x
    })
}

/// Create a signed JWT using the test Ed25519 key.
fn sign_jwt(signing_key: &SigningKey, claims: &TestClaims, kid: Option<&str>) -> String {
    let mut header = Header::new(Algorithm::EdDSA);
    header.kid = kid.map(|s| s.to_string());

    let pkcs8_der = signing_key
        .to_pkcs8_der()
        .expect("failed to encode signing key as PKCS#8 DER");
    let encoding_key = EncodingKey::from_ed_der(pkcs8_der.as_bytes());
    encode(&header, claims, &encoding_key).expect("failed to encode JWT")
}

/// Set up a mock OIDC server with discovery and JWKS endpoints.
async fn setup_mock_server(
    public_key: &ed25519_dalek::VerifyingKey,
    kid: &str,
) -> MockServer {
    let server = MockServer::start().await;

    let jwks_body = serde_json::json!({
        "keys": [ed25519_jwk(public_key, kid)]
    });

    let discovery_body = serde_json::json!({
        "issuer": server.uri(),
        "jwks_uri": format!("{}/jwks", server.uri())
    });

    Mock::given(method("GET"))
        .and(path("/.well-known/openid-configuration"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&discovery_body))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/jwks"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&jwks_body))
        .mount(&server)
        .await;

    server
}

/// Create a PocketIdProvider pointed at the mock server.
fn create_provider(issuer: &str, audience: Option<&str>) -> PocketIdProvider {
    let config = AuthConfig {
        provider: "pocket-id".to_string(),
        issuer: Some(issuer.to_string()),
        audience: audience.map(|s| s.to_string()),
        jwks_refresh_interval: 3600,
    };
    PocketIdProvider::new(config).expect("failed to create provider")
}

/// Helper to create valid claims with sensible defaults.
fn valid_claims(issuer: &str) -> TestClaims {
    TestClaims {
        sub: Some("user-123".to_string()),
        iss: Some(issuer.to_string()),
        aud: None,
        exp: Some((Utc::now().timestamp() + 3600) as u64),
        scope: Some(serde_json::Value::String("read write".to_string())),
    }
}

// ── Valid token tests ────────────────────────────────────────────────

#[tokio::test]
async fn valid_jwt_returns_auth_identity() {
    let (signing_key, public_key) = test_keypair();
    let server = setup_mock_server(&public_key, "test-key-1").await;
    let provider = create_provider(&server.uri(), None);

    let claims = valid_claims(&server.uri());
    let token = sign_jwt(&signing_key, &claims, Some("test-key-1"));

    let identity = provider
        .validate_token(&token)
        .await
        .expect("should validate successfully");

    assert_eq!(identity.user_id, "user-123");
    assert_eq!(identity.provider, "pocket-id");
    assert_eq!(identity.scopes, vec!["read", "write"]);
}

#[tokio::test]
async fn valid_jwt_with_audience_returns_identity() {
    let (signing_key, public_key) = test_keypair();
    let server = setup_mock_server(&public_key, "test-key-1").await;
    let provider = create_provider(&server.uri(), Some("life-engine"));

    let mut claims = valid_claims(&server.uri());
    claims.aud = Some("life-engine".to_string());
    let token = sign_jwt(&signing_key, &claims, Some("test-key-1"));

    let identity = provider
        .validate_token(&token)
        .await
        .expect("should validate with correct audience");

    assert_eq!(identity.user_id, "user-123");
}

#[tokio::test]
async fn scope_as_array_is_parsed_correctly() {
    let (signing_key, public_key) = test_keypair();
    let server = setup_mock_server(&public_key, "test-key-1").await;
    let provider = create_provider(&server.uri(), None);

    let mut claims = valid_claims(&server.uri());
    claims.scope = Some(serde_json::json!(["admin", "read", "sync"]));
    let token = sign_jwt(&signing_key, &claims, Some("test-key-1"));

    let identity = provider
        .validate_token(&token)
        .await
        .expect("should parse array scopes");

    assert_eq!(identity.scopes, vec!["admin", "read", "sync"]);
}

#[tokio::test]
async fn no_scope_claim_returns_empty_scopes() {
    let (signing_key, public_key) = test_keypair();
    let server = setup_mock_server(&public_key, "test-key-1").await;
    let provider = create_provider(&server.uri(), None);

    let mut claims = valid_claims(&server.uri());
    claims.scope = None;
    let token = sign_jwt(&signing_key, &claims, Some("test-key-1"));

    let identity = provider
        .validate_token(&token)
        .await
        .expect("should succeed with no scopes");

    assert!(identity.scopes.is_empty());
}

// ── Expired token tests ──────────────────────────────────────────────

#[tokio::test]
async fn expired_jwt_returns_token_expired() {
    let (signing_key, public_key) = test_keypair();
    let server = setup_mock_server(&public_key, "test-key-1").await;
    let provider = create_provider(&server.uri(), None);

    let mut claims = valid_claims(&server.uri());
    claims.exp = Some((Utc::now().timestamp() - 3600) as u64);
    let token = sign_jwt(&signing_key, &claims, Some("test-key-1"));

    let err = provider
        .validate_token(&token)
        .await
        .expect_err("should reject expired token");

    assert!(
        matches!(err, AuthError::TokenExpired),
        "expected TokenExpired, got: {err:?}"
    );
}

// ── Invalid signature tests ──────────────────────────────────────────

#[tokio::test]
async fn invalid_signature_returns_token_invalid() {
    let (_signing_key, public_key) = test_keypair();
    let server = setup_mock_server(&public_key, "test-key-1").await;
    let provider = create_provider(&server.uri(), None);

    // Sign with a different key.
    let different_key = SigningKey::from_bytes(&[
        99, 98, 97, 96, 95, 94, 93, 92, 91, 90, 89, 88, 87, 86, 85, 84, 83, 82, 81, 80, 79, 78,
        77, 76, 75, 74, 73, 72, 71, 70, 69, 68,
    ]);

    let claims = valid_claims(&server.uri());
    let token = sign_jwt(&different_key, &claims, Some("test-key-1"));

    let err = provider
        .validate_token(&token)
        .await
        .expect_err("should reject invalid signature");

    assert!(
        matches!(err, AuthError::TokenInvalid(_)),
        "expected TokenInvalid, got: {err:?}"
    );
}

// ── Wrong issuer tests ───────────────────────────────────────────────

#[tokio::test]
async fn wrong_issuer_returns_token_invalid() {
    let (signing_key, public_key) = test_keypair();
    let server = setup_mock_server(&public_key, "test-key-1").await;
    let provider = create_provider(&server.uri(), None);

    let mut claims = valid_claims(&server.uri());
    claims.iss = Some("https://wrong-issuer.example.com".to_string());
    let token = sign_jwt(&signing_key, &claims, Some("test-key-1"));

    let err = provider
        .validate_token(&token)
        .await
        .expect_err("should reject wrong issuer");

    assert!(
        matches!(err, AuthError::TokenInvalid(_)),
        "expected TokenInvalid, got: {err:?}"
    );
}

// ── Wrong audience tests ─────────────────────────────────────────────

#[tokio::test]
async fn wrong_audience_returns_token_invalid() {
    let (signing_key, public_key) = test_keypair();
    let server = setup_mock_server(&public_key, "test-key-1").await;
    let provider = create_provider(&server.uri(), Some("life-engine"));

    let mut claims = valid_claims(&server.uri());
    claims.aud = Some("wrong-audience".to_string());
    let token = sign_jwt(&signing_key, &claims, Some("test-key-1"));

    let err = provider
        .validate_token(&token)
        .await
        .expect_err("should reject wrong audience");

    assert!(
        matches!(err, AuthError::TokenInvalid(_)),
        "expected TokenInvalid, got: {err:?}"
    );
}

// ── JWKS refresh tests ───────────────────────────────────────────────

#[tokio::test]
async fn jwks_refresh_when_key_not_in_cache() {
    let (signing_key, public_key) = test_keypair();
    let server = MockServer::start().await;

    // Initially serve an empty JWKS (no keys).
    let discovery_body = serde_json::json!({
        "issuer": server.uri(),
        "jwks_uri": format!("{}/jwks", server.uri())
    });

    Mock::given(method("GET"))
        .and(path("/.well-known/openid-configuration"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&discovery_body))
        .mount(&server)
        .await;

    // First call returns empty keys, second call returns the real key.
    // wiremock serves mocks in reverse order of mounting (last mounted first),
    // but with up(1) we limit the first response to 1 request.
    let jwk = ed25519_jwk(&public_key, "new-key-1");

    Mock::given(method("GET"))
        .and(path("/jwks"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({ "keys": [jwk] })),
        )
        .mount(&server)
        .await;

    let provider = create_provider(&server.uri(), None);

    let claims = valid_claims(&server.uri());
    let token = sign_jwt(&signing_key, &claims, Some("new-key-1"));

    // The provider should discover JWKS, find the key, and validate.
    let identity = provider
        .validate_token(&token)
        .await
        .expect("should validate after JWKS refresh");

    assert_eq!(identity.user_id, "user-123");
}

// ── Unreachable issuer tests ─────────────────────────────────────────

#[tokio::test]
async fn unreachable_issuer_returns_provider_unreachable() {
    // Point at a non-existent server.
    let provider = create_provider("http://127.0.0.1:1", None);

    let (signing_key, _public_key) = test_keypair();
    let claims = TestClaims {
        sub: Some("user-123".to_string()),
        iss: Some("http://127.0.0.1:1".to_string()),
        aud: None,
        exp: Some((Utc::now().timestamp() + 3600) as u64),
        scope: None,
    };
    let token = sign_jwt(&signing_key, &claims, Some("any-key"));

    let err = provider
        .validate_token(&token)
        .await
        .expect_err("should fail when issuer unreachable");

    assert!(
        matches!(err, AuthError::ProviderUnreachable(_)),
        "expected ProviderUnreachable, got: {err:?}"
    );

    // Verify severity is Retryable.
    use life_engine_traits::{EngineError, Severity};
    assert_eq!(err.severity(), Severity::Retryable);
}

// ── Malformed token tests ────────────────────────────────────────────

#[tokio::test]
async fn malformed_token_returns_token_invalid() {
    let (_signing_key, public_key) = test_keypair();
    let server = setup_mock_server(&public_key, "test-key-1").await;
    let provider = create_provider(&server.uri(), None);

    let err = provider
        .validate_token("not-a-valid-jwt")
        .await
        .expect_err("should reject malformed token");

    assert!(
        matches!(err, AuthError::TokenInvalid(_)),
        "expected TokenInvalid, got: {err:?}"
    );
}

// ── Missing sub claim tests ──────────────────────────────────────────

#[tokio::test]
async fn missing_sub_claim_returns_token_invalid() {
    let (signing_key, public_key) = test_keypair();
    let server = setup_mock_server(&public_key, "test-key-1").await;
    let provider = create_provider(&server.uri(), None);

    let mut claims = valid_claims(&server.uri());
    claims.sub = None;
    let token = sign_jwt(&signing_key, &claims, Some("test-key-1"));

    let err = provider
        .validate_token(&token)
        .await
        .expect_err("should reject token without sub");

    assert!(
        matches!(err, AuthError::TokenInvalid(_)),
        "expected TokenInvalid, got: {err:?}"
    );
}

// ── Provider delegates key operations ────────────────────────────────

#[tokio::test]
async fn validate_key_returns_key_invalid() {
    let (_signing_key, public_key) = test_keypair();
    let server = setup_mock_server(&public_key, "test-key-1").await;
    let provider = create_provider(&server.uri(), None);

    let err = provider
        .validate_key("some-api-key")
        .await
        .expect_err("pocket-id should not handle API keys");

    assert!(
        matches!(err, AuthError::KeyInvalid),
        "expected KeyInvalid, got: {err:?}"
    );
}

#[tokio::test]
async fn revoke_key_returns_key_invalid() {
    let (_signing_key, public_key) = test_keypair();
    let server = setup_mock_server(&public_key, "test-key-1").await;
    let provider = create_provider(&server.uri(), None);

    let err = provider
        .revoke_key(uuid::Uuid::new_v4())
        .await
        .expect_err("pocket-id should not handle key revocation");

    assert!(
        matches!(err, AuthError::KeyInvalid),
        "expected KeyInvalid, got: {err:?}"
    );
}
