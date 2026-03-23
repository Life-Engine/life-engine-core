//! Auth configuration.

use serde::Deserialize;

/// Authentication configuration deserialized from the `[auth]` TOML section.
#[derive(Debug, Clone, Deserialize)]
pub struct AuthConfig {
    /// Authentication provider: "pocket-id" or "api-key".
    #[serde(default = "default_provider")]
    pub provider: String,

    /// OIDC issuer URL. Required when provider is "pocket-id".
    pub issuer: Option<String>,

    /// Expected JWT audience claim.
    pub audience: Option<String>,

    /// Seconds between JWKS key refresh. Defaults to 3600.
    #[serde(default = "default_jwks_refresh_interval")]
    pub jwks_refresh_interval: u64,
}

fn default_provider() -> String {
    "pocket-id".to_string()
}

fn default_jwks_refresh_interval() -> u64 {
    3600
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            provider: default_provider(),
            issuer: None,
            audience: None,
            jwks_refresh_interval: default_jwks_refresh_interval(),
        }
    }
}

impl AuthConfig {
    /// Validates the configuration. Returns an error message if invalid.
    pub fn validate(&self) -> Result<(), String> {
        if self.provider == "pocket-id" && self.issuer.is_none() {
            return Err("issuer is required when provider is \"pocket-id\"".to_string());
        }
        Ok(())
    }
}
