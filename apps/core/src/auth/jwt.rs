//! JWT parsing and validation types for OIDC integration.
//!
//! Provides types for JWT headers, claims, JWKS caching, and token
//! validation using the `jsonwebtoken` crate. Supports RS256 algorithm.

use crate::auth::types::AuthError;

use jsonwebtoken::{Algorithm, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use thiserror::Error;
use tokio::sync::RwLock;

/// Default JWKS cache TTL in seconds.
const DEFAULT_JWKS_TTL_SECS: u64 = 3600;

/// JWT-specific errors.
#[derive(Debug, Error)]
pub enum JwtError {
    /// The JWT header could not be decoded.
    #[error("header decode failed: {0}")]
    HeaderDecode(String),
    /// The JWT has expired.
    #[error("token expired")]
    Expired,
    /// The JWT signature is invalid.
    #[error("invalid signature")]
    InvalidSignature,
    /// The issuer claim does not match.
    #[error("issuer mismatch")]
    IssuerMismatch,
    /// The audience claim does not match.
    #[error("audience mismatch")]
    AudienceMismatch,
    /// The token is not yet valid (nbf claim).
    #[error("token not yet valid")]
    NotYetValid,
    /// A general validation error.
    #[error("validation error: {0}")]
    Validation(String),
}

/// Shared JWKS cache wrapped in a RwLock for concurrent access.
pub type SharedJwksCache = Arc<RwLock<Option<SyncJwksCache>>>;

/// Create a new shared JWKS cache.
pub fn new_shared_cache() -> SharedJwksCache {
    Arc::new(RwLock::new(None))
}

/// Return the default JWKS cache TTL.
pub fn default_jwks_ttl() -> Duration {
    Duration::from_secs(DEFAULT_JWKS_TTL_SECS)
}

/// A synchronous JWKS cache (non-async methods) used by OidcProvider.
#[derive(Debug)]
pub struct SyncJwksCache {
    /// Cached keys indexed by key ID.
    keys: HashMap<String, JwkKey>,
    /// When the cache was created.
    created_at: Instant,
    /// TTL for the cache.
    ttl: Duration,
}

impl SyncJwksCache {
    /// Create a new synchronous cache from a JWKS response.
    pub fn new(jwks: JwksResponse, ttl: Duration) -> Self {
        let mut keys = HashMap::new();
        for key in jwks.keys {
            if let Some(ref kid) = key.kid {
                keys.insert(kid.clone(), key);
            }
        }
        Self {
            keys,
            created_at: Instant::now(),
            ttl,
        }
    }

    /// Check whether the cache has expired.
    pub fn is_expired(&self) -> bool {
        self.created_at.elapsed() > self.ttl
    }

    /// Look up a key by its key ID.
    pub fn get_key(&self, kid: &str) -> Option<&JwkKey> {
        self.keys.get(kid)
    }

    /// Return the number of cached keys.
    pub fn len(&self) -> usize {
        self.keys.len()
    }
}

/// A decoded JWT header containing the key ID and algorithm.
#[derive(Debug, Clone)]
pub struct JwtHeader {
    /// The key ID used to sign the token.
    pub kid: Option<String>,
    /// The signing algorithm.
    pub alg: Algorithm,
}

/// Standard OIDC JWT claims.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JwtClaims {
    /// Subject -- the user identifier.
    pub sub: String,
    /// Issuer -- the OIDC provider URL.
    pub iss: String,
    /// Audience -- the intended recipient(s).
    #[serde(default)]
    pub aud: Audience,
    /// Expiration time (Unix timestamp).
    #[serde(default)]
    pub exp: Option<u64>,
    /// Issued at (Unix timestamp).
    #[serde(default)]
    pub iat: Option<u64>,
    /// Not before (Unix timestamp).
    #[serde(default)]
    pub nbf: Option<u64>,
}

/// Audience can be a single string or a list of strings.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(untagged)]
pub enum Audience {
    /// A single audience string.
    Single(String),
    /// Multiple audience strings.
    Multiple(Vec<String>),
    /// No audience specified.
    #[default]
    None,
}

impl Audience {
    /// Check if this audience contains the given value.
    pub fn contains(&self, value: &str) -> bool {
        match self {
            Audience::Single(s) => s == value,
            Audience::Multiple(v) => v.iter().any(|s| s == value),
            Audience::None => false,
        }
    }
}

/// An individual JSON Web Key from a JWKS endpoint.
#[derive(Debug, Clone, Deserialize)]
pub struct JwkKey {
    /// Key type (e.g., "RSA").
    pub kty: String,
    /// Key ID.
    #[serde(default)]
    pub kid: Option<String>,
    /// RSA modulus (Base64url-encoded).
    #[serde(default)]
    pub n: Option<String>,
    /// RSA exponent (Base64url-encoded).
    #[serde(default)]
    pub e: Option<String>,
    /// Key usage (e.g., "sig").
    #[serde(rename = "use", default)]
    pub use_: Option<String>,
    /// Algorithm (e.g., "RS256").
    #[serde(default)]
    pub alg: Option<String>,
}

/// A JWKS response from the provider.
#[derive(Debug, Clone, Deserialize)]
pub struct JwksResponse {
    /// The list of keys.
    pub keys: Vec<JwkKey>,
}

/// Cached JWKS keys with a TTL.
#[derive(Debug)]
pub struct JwksCache {
    /// Cached keys indexed by key ID.
    keys: Arc<RwLock<CachedKeys>>,
    /// TTL for the cache.
    ttl: Duration,
}

/// Internal cached keys state.
#[derive(Debug)]
struct CachedKeys {
    /// Keys indexed by key ID.
    keys: HashMap<String, JwkKey>,
    /// When the cache was last refreshed.
    last_refresh: Option<Instant>,
}

impl JwksCache {
    /// Create a new JWKS cache with the default TTL.
    pub fn new() -> Self {
        Self::with_ttl(Duration::from_secs(DEFAULT_JWKS_TTL_SECS))
    }

    /// Create a new JWKS cache with a custom TTL.
    pub fn with_ttl(ttl: Duration) -> Self {
        Self {
            keys: Arc::new(RwLock::new(CachedKeys {
                keys: HashMap::new(),
                last_refresh: None,
            })),
            ttl,
        }
    }

    /// Update the cache with new keys.
    pub async fn update(&self, jwks: JwksResponse) {
        let mut cached = self.keys.write().await;
        cached.keys.clear();
        for key in jwks.keys {
            if let Some(ref kid) = key.kid {
                cached.keys.insert(kid.clone(), key);
            }
        }
        cached.last_refresh = Some(Instant::now());
    }

    /// Look up a key by its key ID.
    pub async fn get_key(&self, kid: &str) -> Option<JwkKey> {
        let cached = self.keys.read().await;
        cached.keys.get(kid).cloned()
    }

    /// Check whether the cache has expired.
    pub async fn is_expired(&self) -> bool {
        let cached = self.keys.read().await;
        match cached.last_refresh {
            None => true,
            Some(last) => last.elapsed() > self.ttl,
        }
    }

    /// Return the number of cached keys.
    pub async fn key_count(&self) -> usize {
        let cached = self.keys.read().await;
        cached.keys.len()
    }
}

impl Default for JwksCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Decode a JWT header without validating the token.
pub fn decode_jwt_header(token: &str) -> Result<JwtHeader, JwtError> {
    let header = jsonwebtoken::decode_header(token)
        .map_err(|e| JwtError::HeaderDecode(format!("{e}")))?;

    Ok(JwtHeader {
        kid: header.kid,
        alg: header.alg,
    })
}

/// Validate a JWT token and extract claims.
///
/// Verifies the signature using the provided decoding key, then checks
/// issuer and audience claims.
pub fn validate_jwt(
    token: &str,
    key: &DecodingKey,
    issuer: &str,
    audience: Option<&str>,
) -> Result<JwtClaims, AuthError> {
    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_issuer(&[issuer]);

    if let Some(aud) = audience {
        validation.set_audience(&[aud]);
    } else {
        validation.validate_aud = false;
    }

    validation.validate_exp = true;
    validation.validate_nbf = true;

    let token_data = jsonwebtoken::decode::<JwtClaims>(token, key, &validation)
        .map_err(|e| match e.kind() {
            jsonwebtoken::errors::ErrorKind::ExpiredSignature => AuthError::TokenExpired,
            jsonwebtoken::errors::ErrorKind::InvalidIssuer => {
                AuthError::InvalidCredentials
            }
            jsonwebtoken::errors::ErrorKind::InvalidAudience => {
                AuthError::InvalidCredentials
            }
            jsonwebtoken::errors::ErrorKind::ImmatureSignature => {
                AuthError::InvalidCredentials
            }
            _ => AuthError::Internal(format!("JWT validation failed: {e}")),
        })?;

    Ok(token_data.claims)
}

/// Decode, build key, and validate a JWT in one step.
///
/// Combines `decoding_key_from_jwk` and `validate_jwt`. Returns the
/// full `TokenData` (including header) rather than just claims.
pub fn decode_and_validate_jwt(
    token: &str,
    jwk: &JwkKey,
    issuer: Option<&str>,
    audience: Option<&str>,
) -> Result<jsonwebtoken::TokenData<JwtClaims>, JwtError> {
    let decoding_key = decoding_key_from_jwk(jwk)
        .map_err(|e| JwtError::Validation(e.to_string()))?;

    let mut validation = Validation::new(Algorithm::RS256);
    if let Some(iss) = issuer {
        validation.set_issuer(&[iss]);
    }
    if let Some(aud) = audience {
        validation.set_audience(&[aud]);
    } else {
        validation.validate_aud = false;
    }
    validation.validate_exp = true;
    validation.validate_nbf = true;

    jsonwebtoken::decode::<JwtClaims>(token, &decoding_key, &validation)
        .map_err(|e| match e.kind() {
            jsonwebtoken::errors::ErrorKind::ExpiredSignature => JwtError::Expired,
            jsonwebtoken::errors::ErrorKind::InvalidIssuer => JwtError::IssuerMismatch,
            jsonwebtoken::errors::ErrorKind::InvalidAudience => JwtError::AudienceMismatch,
            jsonwebtoken::errors::ErrorKind::ImmatureSignature => JwtError::NotYetValid,
            jsonwebtoken::errors::ErrorKind::InvalidSignature => JwtError::InvalidSignature,
            _ => JwtError::Validation(format!("JWT validation failed: {e}")),
        })
}

/// Build a `DecodingKey` from a JWK RSA key.
pub fn decoding_key_from_jwk(jwk: &JwkKey) -> Result<DecodingKey, AuthError> {
    let n = jwk
        .n
        .as_ref()
        .ok_or_else(|| AuthError::Internal("JWK missing RSA modulus (n)".into()))?;
    let e = jwk
        .e
        .as_ref()
        .ok_or_else(|| AuthError::Internal("JWK missing RSA exponent (e)".into()))?;

    DecodingKey::from_rsa_components(n, e)
        .map_err(|e| AuthError::Internal(format!("failed to build RSA decoding key: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use jsonwebtoken::{encode, EncodingKey, Header};

    /// Generate a test RSA key pair for signing/verifying JWTs.
    fn test_rsa_keys() -> (EncodingKey, DecodingKey) {
        use rsa::pkcs8::{EncodePrivateKey, EncodePublicKey};
        use rsa::RsaPrivateKey;

        let mut rng = rand::thread_rng();
        let private_key = RsaPrivateKey::new(&mut rng, 2048).unwrap();
        let public_key = private_key.to_public_key();

        let private_pem = private_key
            .to_pkcs8_pem(rsa::pkcs8::LineEnding::LF)
            .unwrap();
        let public_pem = public_key
            .to_public_key_pem(rsa::pkcs8::LineEnding::LF)
            .unwrap();

        let encoding = EncodingKey::from_rsa_pem(private_pem.as_bytes()).unwrap();
        let decoding = DecodingKey::from_rsa_pem(public_pem.as_bytes()).unwrap();
        (encoding, decoding)
    }

    /// Create a test JWT with the given claims.
    fn create_test_jwt(claims: &JwtClaims, encoding_key: &EncodingKey) -> String {
        let mut header = Header::new(Algorithm::RS256);
        header.kid = Some("test-kid-1".into());
        encode(&header, claims, encoding_key).unwrap()
    }

    fn valid_claims() -> JwtClaims {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        JwtClaims {
            sub: "user-123".into(),
            iss: "https://auth.example.com".into(),
            aud: Audience::Single("life-engine".into()),
            exp: Some(now + 3600),
            iat: Some(now),
            nbf: Some(now - 10),
        }
    }

    // --- Header decode tests ---

    #[test]
    fn decode_jwt_header_extracts_kid_and_alg() {
        let (encoding_key, _) = test_rsa_keys();
        let claims = valid_claims();
        let token = create_test_jwt(&claims, &encoding_key);

        let header = decode_jwt_header(&token).unwrap();
        assert_eq!(header.kid, Some("test-kid-1".into()));
        assert_eq!(header.alg, Algorithm::RS256);
    }

    #[test]
    fn decode_jwt_header_invalid_token() {
        let result = decode_jwt_header("not.a.jwt");
        assert!(result.is_err());
    }

    // --- JWT validation tests ---

    #[test]
    fn validate_jwt_valid_token() {
        let (encoding_key, decoding_key) = test_rsa_keys();
        let claims = valid_claims();
        let token = create_test_jwt(&claims, &encoding_key);

        let result = validate_jwt(
            &token,
            &decoding_key,
            "https://auth.example.com",
            Some("life-engine"),
        );
        let validated = result.unwrap();
        assert_eq!(validated.sub, "user-123");
        assert_eq!(validated.iss, "https://auth.example.com");
    }

    #[test]
    fn validate_jwt_expired_token_rejected() {
        let (encoding_key, decoding_key) = test_rsa_keys();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let claims = JwtClaims {
            sub: "user-456".into(),
            iss: "https://auth.example.com".into(),
            aud: Audience::Single("life-engine".into()),
            exp: Some(now - 3600),
            iat: Some(now - 7200),
            nbf: Some(now - 7200),
        };
        let token = create_test_jwt(&claims, &encoding_key);

        let err = validate_jwt(
            &token,
            &decoding_key,
            "https://auth.example.com",
            Some("life-engine"),
        )
        .unwrap_err();
        assert!(matches!(err, AuthError::TokenExpired));
    }

    #[test]
    fn validate_jwt_wrong_issuer_rejected() {
        let (encoding_key, decoding_key) = test_rsa_keys();
        let claims = valid_claims();
        let token = create_test_jwt(&claims, &encoding_key);

        let err = validate_jwt(
            &token,
            &decoding_key,
            "https://wrong-issuer.example.com",
            Some("life-engine"),
        )
        .unwrap_err();
        assert!(matches!(err, AuthError::InvalidCredentials));
    }

    #[test]
    fn validate_jwt_wrong_audience_rejected() {
        let (encoding_key, decoding_key) = test_rsa_keys();
        let claims = valid_claims();
        let token = create_test_jwt(&claims, &encoding_key);

        let err = validate_jwt(
            &token,
            &decoding_key,
            "https://auth.example.com",
            Some("wrong-audience"),
        )
        .unwrap_err();
        assert!(matches!(err, AuthError::InvalidCredentials));
    }

    #[test]
    fn validate_jwt_future_nbf_rejected() {
        let (encoding_key, decoding_key) = test_rsa_keys();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let claims = JwtClaims {
            sub: "user-789".into(),
            iss: "https://auth.example.com".into(),
            aud: Audience::Single("life-engine".into()),
            exp: Some(now + 7200),
            iat: Some(now),
            nbf: Some(now + 3600),
        };
        let token = create_test_jwt(&claims, &encoding_key);

        let err = validate_jwt(
            &token,
            &decoding_key,
            "https://auth.example.com",
            Some("life-engine"),
        )
        .unwrap_err();
        assert!(matches!(err, AuthError::InvalidCredentials));
    }

    #[test]
    fn validate_jwt_without_audience_check() {
        let (encoding_key, decoding_key) = test_rsa_keys();
        let claims = valid_claims();
        let token = create_test_jwt(&claims, &encoding_key);

        let result = validate_jwt(
            &token,
            &decoding_key,
            "https://auth.example.com",
            None,
        );
        assert!(result.is_ok());
    }

    // --- JWKS cache tests ---

    #[tokio::test]
    async fn jwks_cache_stores_and_retrieves_keys() {
        let cache = JwksCache::new();
        let jwks = JwksResponse {
            keys: vec![JwkKey {
                kty: "RSA".into(),
                kid: Some("key-1".into()),
                n: Some("modulus".into()),
                e: Some("exponent".into()),
                use_: Some("sig".into()),
                alg: Some("RS256".into()),
            }],
        };

        cache.update(jwks).await;
        let key = cache.get_key("key-1").await;
        assert!(key.is_some());
        let key = key.unwrap();
        assert_eq!(key.kty, "RSA");
        assert_eq!(key.n.as_deref(), Some("modulus"));
    }

    #[tokio::test]
    async fn jwks_cache_missing_key_returns_none() {
        let cache = JwksCache::new();
        let key = cache.get_key("nonexistent").await;
        assert!(key.is_none());
    }

    #[tokio::test]
    async fn jwks_cache_ttl_expiry() {
        let cache = JwksCache::with_ttl(Duration::from_millis(50));

        assert!(cache.is_expired().await);

        let jwks = JwksResponse {
            keys: vec![JwkKey {
                kty: "RSA".into(),
                kid: Some("key-1".into()),
                n: Some("n".into()),
                e: Some("e".into()),
                use_: None,
                alg: None,
            }],
        };
        cache.update(jwks).await;
        assert!(!cache.is_expired().await);

        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(cache.is_expired().await);
    }

    #[tokio::test]
    async fn jwks_cache_key_count() {
        let cache = JwksCache::new();
        assert_eq!(cache.key_count().await, 0);

        let jwks = JwksResponse {
            keys: vec![
                JwkKey {
                    kty: "RSA".into(),
                    kid: Some("key-1".into()),
                    n: Some("n1".into()),
                    e: Some("e1".into()),
                    use_: None,
                    alg: None,
                },
                JwkKey {
                    kty: "RSA".into(),
                    kid: Some("key-2".into()),
                    n: Some("n2".into()),
                    e: Some("e2".into()),
                    use_: None,
                    alg: None,
                },
            ],
        };
        cache.update(jwks).await;
        assert_eq!(cache.key_count().await, 2);
    }

    #[tokio::test]
    async fn jwks_cache_default() {
        let cache = JwksCache::default();
        assert!(cache.is_expired().await);
    }

    #[tokio::test]
    async fn jwks_cache_ignores_keys_without_kid() {
        let cache = JwksCache::new();
        let jwks = JwksResponse {
            keys: vec![JwkKey {
                kty: "RSA".into(),
                kid: None,
                n: Some("n".into()),
                e: Some("e".into()),
                use_: None,
                alg: None,
            }],
        };
        cache.update(jwks).await;
        assert_eq!(cache.key_count().await, 0);
    }

    // --- Audience tests ---

    #[test]
    fn audience_contains_single() {
        let aud = Audience::Single("test".into());
        assert!(aud.contains("test"));
        assert!(!aud.contains("other"));
    }

    #[test]
    fn audience_contains_multiple() {
        let aud = Audience::Multiple(vec!["a".into(), "b".into()]);
        assert!(aud.contains("a"));
        assert!(aud.contains("b"));
        assert!(!aud.contains("c"));
    }

    #[test]
    fn audience_contains_none() {
        let aud = Audience::None;
        assert!(!aud.contains("anything"));
    }

    // --- Decoding key tests ---

    #[test]
    fn decoding_key_from_jwk_missing_n() {
        let jwk = JwkKey {
            kty: "RSA".into(),
            kid: Some("k".into()),
            n: None,
            e: Some("AQAB".into()),
            use_: None,
            alg: None,
        };
        match decoding_key_from_jwk(&jwk) {
            Err(e) => assert!(e.to_string().contains("modulus")),
            Ok(_) => panic!("expected error for missing modulus"),
        }
    }

    #[test]
    fn decoding_key_from_jwk_missing_e() {
        let jwk = JwkKey {
            kty: "RSA".into(),
            kid: Some("k".into()),
            n: Some("AQAB".into()),
            e: None,
            use_: None,
            alg: None,
        };
        match decoding_key_from_jwk(&jwk) {
            Err(e) => assert!(e.to_string().contains("exponent")),
            Ok(_) => panic!("expected error for missing exponent"),
        }
    }

    // --- Claims serialization ---

    #[test]
    fn jwt_claims_serialization_roundtrip() {
        let claims = valid_claims();
        let json = serde_json::to_string(&claims).unwrap();
        let deserialized: JwtClaims = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.sub, claims.sub);
        assert_eq!(deserialized.iss, claims.iss);
    }

    // --- JwtError display ---

    #[test]
    fn jwt_error_display() {
        let err = JwtError::HeaderDecode("bad token".into());
        assert_eq!(err.to_string(), "header decode failed: bad token");
    }
}
