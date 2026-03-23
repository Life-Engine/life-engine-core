//! Token validation handler and Pocket ID provider.

use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, TokenData, Validation};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::debug;
use uuid::Uuid;

use crate::config::AuthConfig;
use crate::error::AuthError;
use crate::types::AuthIdentity;
use crate::AuthProvider;

/// OIDC discovery document (subset of fields we need).
#[derive(Debug, Deserialize)]
struct OidcDiscovery {
    jwks_uri: String,
}

/// A single JSON Web Key from the JWKS endpoint.
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
struct Jwk {
    /// Key ID.
    kid: Option<String>,
    /// Key type (e.g., "RSA", "OKP").
    kty: String,
    /// Algorithm (e.g., "RS256", "EdDSA").
    alg: Option<String>,
    /// RSA modulus (base64url).
    n: Option<String>,
    /// RSA exponent (base64url).
    e: Option<String>,
    /// OKP public key (base64url, for Ed25519).
    x: Option<String>,
    /// OKP curve name (e.g., "Ed25519").
    crv: Option<String>,
}

/// JWKS response from the issuer.
#[derive(Debug, Deserialize)]
struct JwksResponse {
    keys: Vec<Jwk>,
}

/// A cached decoding key with its metadata.
#[derive(Clone)]
struct CachedKey {
    kid: Option<String>,
    algorithm: Algorithm,
    decoding_key: DecodingKey,
}

/// Internal cache for JWKS keys and refresh timing.
struct JwksCache {
    keys: Vec<CachedKey>,
    last_refresh: Instant,
    jwks_uri: Option<String>,
}

/// JWT claims we extract during validation.
#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    /// Subject (user ID).
    sub: Option<String>,
    /// Issuer.
    iss: Option<String>,
    /// Audience (can be string or array).
    aud: Option<serde_json::Value>,
    /// Expiration (unix timestamp).
    exp: Option<u64>,
    /// Scopes (space-separated string or array).
    scope: Option<serde_json::Value>,
}

/// Pocket ID (OIDC) authentication provider.
///
/// Validates JWT bearer tokens against the configured OIDC issuer.
/// JWKS public keys are cached and periodically refreshed.
pub struct PocketIdProvider {
    /// OIDC issuer URL.
    issuer: String,
    /// Expected JWT audience claim (if configured).
    audience: Option<String>,
    /// Seconds between JWKS key refreshes.
    jwks_refresh_interval: u64,
    /// Cached JWKS keys.
    cache: Arc<RwLock<JwksCache>>,
    /// HTTP client for OIDC discovery and JWKS fetching.
    http_client: reqwest::Client,
}

impl PocketIdProvider {
    /// Create a new Pocket ID provider from the auth configuration.
    pub fn new(config: AuthConfig) -> Result<Self, AuthError> {
        let issuer = config.issuer.ok_or_else(|| {
            AuthError::ConfigInvalid("issuer is required for pocket-id provider".to_string())
        })?;

        let cache = Arc::new(RwLock::new(JwksCache {
            keys: Vec::new(),
            last_refresh: Instant::now() - std::time::Duration::from_secs(config.jwks_refresh_interval + 1),
            jwks_uri: None,
        }));

        let http_client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .map_err(|e| AuthError::ConfigInvalid(format!("failed to create HTTP client: {e}")))?;

        Ok(Self {
            issuer,
            audience: config.audience,
            jwks_refresh_interval: config.jwks_refresh_interval,
            cache,
            http_client,
        })
    }

    /// Fetch the OIDC discovery document to get the JWKS URI.
    async fn discover_jwks_uri(&self) -> Result<String, AuthError> {
        let discovery_url = format!(
            "{}/.well-known/openid-configuration",
            self.issuer.trim_end_matches('/')
        );

        debug!(url = %discovery_url, "fetching OIDC discovery document");

        let response = self
            .http_client
            .get(&discovery_url)
            .send()
            .await
            .map_err(|e| AuthError::ProviderUnreachable(format!("OIDC discovery failed: {e}")))?;

        if !response.status().is_success() {
            return Err(AuthError::ProviderUnreachable(format!(
                "OIDC discovery returned status {}",
                response.status()
            )));
        }

        let discovery: OidcDiscovery = response
            .json()
            .await
            .map_err(|e| AuthError::ProviderUnreachable(format!("invalid discovery document: {e}")))?;

        Ok(discovery.jwks_uri)
    }

    /// Fetch and parse JWKS keys from the issuer.
    async fn fetch_jwks(&self, jwks_uri: &str) -> Result<Vec<CachedKey>, AuthError> {
        debug!(uri = %jwks_uri, "fetching JWKS keys");

        let response = self
            .http_client
            .get(jwks_uri)
            .send()
            .await
            .map_err(|e| AuthError::ProviderUnreachable(format!("JWKS fetch failed: {e}")))?;

        if !response.status().is_success() {
            return Err(AuthError::ProviderUnreachable(format!(
                "JWKS endpoint returned status {}",
                response.status()
            )));
        }

        let jwks: JwksResponse = response
            .json()
            .await
            .map_err(|e| AuthError::ProviderUnreachable(format!("invalid JWKS response: {e}")))?;

        let mut cached_keys = Vec::new();
        for jwk in &jwks.keys {
            if let Some(cached) = Self::parse_jwk(jwk) {
                cached_keys.push(cached);
            }
        }

        debug!(count = cached_keys.len(), "cached JWKS keys");
        Ok(cached_keys)
    }

    /// Parse a single JWK into a CachedKey, if supported.
    fn parse_jwk(jwk: &Jwk) -> Option<CachedKey> {
        match jwk.kty.as_str() {
            "OKP" => {
                // Ed25519 key
                if jwk.crv.as_deref() != Some("Ed25519") {
                    return None;
                }
                let x = jwk.x.as_ref()?;
                let decoding_key = DecodingKey::from_ed_der(
                    &base64_url_decode(x).ok()?,
                );
                Some(CachedKey {
                    kid: jwk.kid.clone(),
                    algorithm: Algorithm::EdDSA,
                    decoding_key,
                })
            }
            "RSA" => {
                // RSA key (RS256)
                let n = jwk.n.as_ref()?;
                let e = jwk.e.as_ref()?;
                let decoding_key =
                    DecodingKey::from_rsa_components(n, e).ok()?;
                Some(CachedKey {
                    kid: jwk.kid.clone(),
                    algorithm: Algorithm::RS256,
                    decoding_key,
                })
            }
            _ => None,
        }
    }

    /// Refresh JWKS keys if the cache has expired.
    async fn refresh_keys_if_needed(&self) -> Result<(), AuthError> {
        let needs_refresh = {
            let cache = self.cache.read().await;
            cache.last_refresh.elapsed().as_secs() >= self.jwks_refresh_interval
        };

        if needs_refresh {
            self.refresh_keys().await?;
        }

        Ok(())
    }

    /// Force-refresh JWKS keys from the issuer.
    async fn refresh_keys(&self) -> Result<(), AuthError> {
        let jwks_uri = {
            let cache = self.cache.read().await;
            cache.jwks_uri.clone()
        };

        let jwks_uri = match jwks_uri {
            Some(uri) => uri,
            None => {
                let uri = self.discover_jwks_uri().await?;
                let mut cache = self.cache.write().await;
                cache.jwks_uri = Some(uri.clone());
                uri
            }
        };

        let keys = self.fetch_jwks(&jwks_uri).await?;
        let mut cache = self.cache.write().await;
        cache.keys = keys;
        cache.last_refresh = Instant::now();
        Ok(())
    }

    /// Find a cached key matching the given key ID.
    async fn find_key(&self, kid: Option<&str>) -> Option<CachedKey> {
        let cache = self.cache.read().await;

        if let Some(kid) = kid {
            // Match by key ID.
            cache
                .keys
                .iter()
                .find(|k| k.kid.as_deref() == Some(kid))
                .cloned()
        } else if cache.keys.len() == 1 {
            // If there's only one key and no kid specified, use it.
            cache.keys.first().cloned()
        } else {
            None
        }
    }

    /// Validate a JWT token and return the decoded claims.
    async fn decode_and_validate(&self, token: &str) -> Result<TokenData<Claims>, AuthError> {
        let header = decode_header(token)
            .map_err(|e| AuthError::TokenInvalid(format!("malformed JWT header: {e}")))?;

        let kid = header.kid.as_deref();

        // Try to find the key in cache first.
        let mut key = self.find_key(kid).await;

        // If not found, refresh keys and try again.
        if key.is_none() {
            debug!(kid = ?kid, "key not in cache, refreshing JWKS");
            self.refresh_keys().await?;
            key = self.find_key(kid).await;
        }

        let cached_key = key.ok_or_else(|| {
            AuthError::TokenInvalid(format!(
                "no matching key found for kid: {}",
                kid.unwrap_or("<none>")
            ))
        })?;

        let mut validation = Validation::new(cached_key.algorithm);

        // Configure issuer validation.
        validation.set_issuer(&[&self.issuer]);

        // Configure audience validation.
        if let Some(ref aud) = self.audience {
            validation.set_audience(&[aud]);
        } else {
            validation.validate_aud = false;
        }

        // Require exp claim.
        validation.set_required_spec_claims(&["exp", "sub", "iss"]);

        decode::<Claims>(token, &cached_key.decoding_key, &validation).map_err(|e| {
            match e.kind() {
                jsonwebtoken::errors::ErrorKind::ExpiredSignature => AuthError::TokenExpired,
                jsonwebtoken::errors::ErrorKind::InvalidIssuer => {
                    AuthError::TokenInvalid("invalid issuer".to_string())
                }
                jsonwebtoken::errors::ErrorKind::InvalidAudience => {
                    AuthError::TokenInvalid("invalid audience".to_string())
                }
                _ => AuthError::TokenInvalid(format!("JWT validation failed: {e}")),
            }
        })
    }

    /// Extract scopes from the claims.
    fn extract_scopes(claims: &Claims) -> Vec<String> {
        match &claims.scope {
            Some(serde_json::Value::String(s)) => {
                s.split_whitespace().map(|s| s.to_string()).collect()
            }
            Some(serde_json::Value::Array(arr)) => arr
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect(),
            _ => Vec::new(),
        }
    }
}

#[async_trait]
impl AuthProvider for PocketIdProvider {
    async fn validate_token(&self, token: &str) -> Result<AuthIdentity, AuthError> {
        // Ensure keys are loaded / refreshed.
        self.refresh_keys_if_needed().await?;

        let token_data = self.decode_and_validate(token).await?;
        let claims = token_data.claims;

        let scopes = Self::extract_scopes(&claims);

        let user_id = claims
            .sub
            .ok_or_else(|| AuthError::TokenInvalid("missing sub claim".to_string()))?;

        Ok(AuthIdentity {
            user_id,
            provider: "pocket-id".to_string(),
            scopes,
            authenticated_at: chrono::Utc::now(),
        })
    }

    async fn validate_key(&self, _key: &str) -> Result<AuthIdentity, AuthError> {
        // Pocket ID provider delegates key validation to the API key provider.
        Err(AuthError::KeyInvalid)
    }

    async fn revoke_key(&self, _key_id: Uuid) -> Result<(), AuthError> {
        // Pocket ID provider delegates key management to the API key provider.
        Err(AuthError::KeyInvalid)
    }
}

/// Decode a base64url-encoded string (no padding) to bytes.
fn base64_url_decode(input: &str) -> Result<Vec<u8>, AuthError> {
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine;
    URL_SAFE_NO_PAD
        .decode(input)
        .map_err(|e| AuthError::TokenInvalid(format!("base64url decode failed: {e}")))
}
