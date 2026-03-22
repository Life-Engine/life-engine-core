//! Configuration loading and validation for Life Engine Core.
//!
//! Supports three sources with increasing priority:
//! 1. YAML file (`~/.life-engine/config.yaml`)
//! 2. Environment variables with `LIFE_ENGINE_` prefix
//! 3. CLI arguments
//!
//! Sensible defaults are provided for all fields.

use crate::error::CoreError;
use clap::Parser;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Top-level configuration for the Core binary.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CoreConfig {
    /// Core server settings.
    #[serde(default)]
    pub core: CoreSettings,

    /// Authentication settings.
    #[serde(default)]
    pub auth: AuthSettings,

    /// Storage settings.
    #[serde(default)]
    pub storage: StorageSettings,

    /// Plugin settings.
    #[serde(default)]
    pub plugins: PluginSettings,

    /// Network and TLS settings.
    #[serde(default)]
    pub network: NetworkSettings,
}

/// Core server settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoreSettings {
    /// The host address to bind to.
    #[serde(default = "default_host")]
    pub host: String,

    /// The port to listen on.
    #[serde(default = "default_port")]
    pub port: u16,

    /// Log level (trace, debug, info, warn, error).
    #[serde(default = "default_log_level")]
    pub log_level: String,

    /// Log format: "json" or "pretty".
    #[serde(default = "default_log_format")]
    pub log_format: String,

    /// Data directory for persistent state.
    #[serde(default = "default_data_dir")]
    pub data_dir: String,
}

impl Default for CoreSettings {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: default_port(),
            log_level: default_log_level(),
            log_format: default_log_format(),
            data_dir: default_data_dir(),
        }
    }
}

/// Authentication settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthSettings {
    /// The authentication provider to use.
    #[serde(default = "default_auth_provider")]
    pub provider: String,

    /// OIDC configuration (required when provider is "oidc").
    #[serde(default)]
    pub oidc: Option<OidcSettings>,

    /// WebAuthn configuration (required when provider is "webauthn").
    #[serde(default)]
    pub webauthn: Option<WebAuthnSettings>,
}

impl Default for AuthSettings {
    fn default() -> Self {
        Self {
            provider: default_auth_provider(),
            oidc: None,
            webauthn: None,
        }
    }
}

/// WebAuthn-specific configuration for passkey authentication.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebAuthnSettings {
    /// Relying party name (e.g. "Life Engine").
    pub rp_name: String,
    /// Relying party ID (e.g. "localhost" or "example.com").
    pub rp_id: String,
    /// Relying party origin URL (e.g. "https://example.com" or "http://localhost:3750").
    pub rp_origin: String,
    /// Challenge TTL in seconds (default 300 = 5 minutes).
    #[serde(default = "default_webauthn_challenge_ttl")]
    pub challenge_ttl_secs: u64,
}

/// Default WebAuthn challenge TTL: 5 minutes.
fn default_webauthn_challenge_ttl() -> u64 {
    300
}

/// OIDC-specific configuration for Pocket ID integration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OidcSettings {
    /// The OIDC issuer URL (e.g., "http://localhost:3751").
    pub issuer_url: String,
    /// The client ID registered with the identity provider.
    pub client_id: String,
    /// The client secret (for confidential clients).
    #[serde(default)]
    pub client_secret: Option<String>,
    /// Custom JWKS endpoint (derived from issuer_url if not set).
    #[serde(default)]
    pub jwks_uri: Option<String>,
    /// Expected audience claim value.
    #[serde(default)]
    pub audience: Option<String>,
}

/// Storage settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageSettings {
    /// The storage backend to use: "sqlite" (default) or "postgres".
    #[serde(default = "default_storage_backend")]
    pub backend: String,

    /// Whether to enable SQLCipher encryption (SQLite only).
    #[serde(default = "default_encryption")]
    pub encryption: bool,

    /// Argon2 key-derivation parameters (SQLite only).
    #[serde(default)]
    pub argon2: Argon2Settings,

    /// PostgreSQL configuration (required when backend is "postgres").
    #[serde(default)]
    pub postgres: Option<PostgresSettings>,
}

impl Default for StorageSettings {
    fn default() -> Self {
        Self {
            backend: default_storage_backend(),
            encryption: default_encryption(),
            argon2: Argon2Settings::default(),
            postgres: None,
        }
    }
}

/// PostgreSQL connection settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostgresSettings {
    /// PostgreSQL host.
    #[serde(default = "default_pg_host")]
    pub host: String,
    /// PostgreSQL port.
    #[serde(default = "default_pg_port")]
    pub port: u16,
    /// Database name.
    #[serde(default = "default_pg_dbname")]
    pub dbname: String,
    /// Username.
    #[serde(default = "default_pg_user")]
    pub user: String,
    /// Password.
    #[serde(default)]
    pub password: String,
    /// Connection pool size.
    #[serde(default = "default_pg_pool_size")]
    pub pool_size: usize,
}

impl Default for PostgresSettings {
    fn default() -> Self {
        Self {
            host: default_pg_host(),
            port: default_pg_port(),
            dbname: default_pg_dbname(),
            user: default_pg_user(),
            password: String::new(),
            pool_size: default_pg_pool_size(),
        }
    }
}

fn default_pg_host() -> String {
    "localhost".into()
}
fn default_pg_port() -> u16 {
    5432
}
fn default_pg_dbname() -> String {
    "life_engine".into()
}
fn default_pg_user() -> String {
    "life_engine".into()
}
fn default_pg_pool_size() -> usize {
    16
}

/// Argon2 key-derivation settings for SQLCipher.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Argon2Settings {
    /// Memory cost in megabytes.
    #[serde(default = "default_argon2_memory_mb")]
    pub memory_mb: u32,

    /// Number of iterations.
    #[serde(default = "default_argon2_iterations")]
    pub iterations: u32,

    /// Degree of parallelism.
    #[serde(default = "default_argon2_parallelism")]
    pub parallelism: u32,
}

impl Default for Argon2Settings {
    fn default() -> Self {
        Self {
            memory_mb: default_argon2_memory_mb(),
            iterations: default_argon2_iterations(),
            parallelism: default_argon2_parallelism(),
        }
    }
}

/// Plugin settings.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PluginSettings {
    /// Directories to scan for plugins.
    #[serde(default)]
    pub paths: Vec<String>,

    /// Whether to auto-enable discovered plugins.
    #[serde(default)]
    pub auto_enable: bool,
}

/// Network, TLS, CORS, and rate limiting settings.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NetworkSettings {
    /// TLS configuration.
    #[serde(default)]
    pub tls: TlsSettings,

    /// CORS configuration.
    #[serde(default)]
    pub cors: CorsSettings,

    /// Rate limiting configuration.
    #[serde(default)]
    pub rate_limit: RateLimitSettings,
}

/// TLS settings.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TlsSettings {
    /// Whether TLS is enabled.
    #[serde(default)]
    pub enabled: bool,

    /// Path to the TLS certificate file.
    #[serde(default)]
    pub cert_path: String,

    /// Path to the TLS private key file.
    #[serde(default)]
    pub key_path: String,
}

/// CORS settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorsSettings {
    /// Allowed origins.
    #[serde(default = "default_cors_origins")]
    pub allowed_origins: Vec<String>,
}

impl Default for CorsSettings {
    fn default() -> Self {
        Self {
            allowed_origins: default_cors_origins(),
        }
    }
}

/// Rate limiting settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitSettings {
    /// Maximum requests per minute.
    #[serde(default = "default_rate_limit")]
    pub requests_per_minute: u32,
}

impl Default for RateLimitSettings {
    fn default() -> Self {
        Self {
            requests_per_minute: default_rate_limit(),
        }
    }
}

// Default value functions for serde.
fn default_host() -> String {
    "127.0.0.1".into()
}
fn default_port() -> u16 {
    3750
}
fn default_log_level() -> String {
    "info".into()
}
fn default_log_format() -> String {
    "json".into()
}
fn default_data_dir() -> String {
    "~/.life-engine/data".into()
}
fn default_auth_provider() -> String {
    "local-token".into()
}
fn default_storage_backend() -> String {
    "sqlite".into()
}
fn default_encryption() -> bool {
    true
}
fn default_argon2_memory_mb() -> u32 {
    64
}
fn default_argon2_iterations() -> u32 {
    3
}
fn default_argon2_parallelism() -> u32 {
    4
}
fn default_cors_origins() -> Vec<String> {
    vec!["http://localhost:1420".into()]
}
fn default_rate_limit() -> u32 {
    60
}

/// CLI arguments parsed by `clap`.
#[derive(Debug, Parser)]
#[command(name = "life-engine-core", about = "Life Engine Core backend")]
pub struct CliArgs {
    /// Path to configuration file.
    #[arg(long, default_value = "")]
    pub config: String,

    /// Host address to bind to.
    #[arg(long)]
    pub host: Option<String>,

    /// Port to listen on.
    #[arg(long)]
    pub port: Option<u16>,

    /// Log level (trace, debug, info, warn, error).
    #[arg(long)]
    pub log_level: Option<String>,

    /// Log format (json, pretty).
    #[arg(long)]
    pub log_format: Option<String>,

    /// Data directory path.
    #[arg(long)]
    pub data_dir: Option<String>,
}

impl CoreConfig {
    /// Load configuration from file, environment variables, and CLI args.
    ///
    /// Priority: YAML file < env vars < CLI args.
    pub fn load(cli: &CliArgs) -> anyhow::Result<Self> {
        // 1. Start with defaults.
        let mut config = CoreConfig::default();

        // 2. Load from YAML file if it exists.
        let config_path = if cli.config.is_empty() {
            Self::default_config_path()
        } else {
            Some(PathBuf::from(&cli.config))
        };

        if let Some(path) = config_path
            && path.exists()
        {
            config = Self::load_from_yaml(&path)?;
        }

        // 3. Apply environment variable overrides.
        config.apply_env_overrides();

        // 4. Apply CLI argument overrides.
        config.apply_cli_overrides(cli);

        // 5. Validate.
        config.validate()?;

        Ok(config)
    }

    /// Load configuration from a YAML file.
    fn load_from_yaml(path: &Path) -> anyhow::Result<Self> {
        let contents = std::fs::read_to_string(path).map_err(|e| {
            CoreError::Config(format!("failed to read config file {}: {e}", path.display()))
        })?;
        let config: CoreConfig = serde_yaml::from_str(&contents).map_err(|e| {
            CoreError::Config(format!(
                "failed to parse config file {}: {e}",
                path.display()
            ))
        })?;
        Ok(config)
    }

    /// Returns the default config file path (`~/.life-engine/config.yaml`).
    fn default_config_path() -> Option<PathBuf> {
        directories::BaseDirs::new().map(|dirs| dirs.home_dir().join(".life-engine/config.yaml"))
    }

    /// Apply `LIFE_ENGINE_` environment variable overrides.
    fn apply_env_overrides(&mut self) {
        if let Ok(val) = std::env::var("LIFE_ENGINE_CORE_HOST") {
            self.core.host = val;
        }
        if let Ok(val) = std::env::var("LIFE_ENGINE_CORE_PORT")
            && let Ok(port) = val.parse::<u16>()
        {
            self.core.port = port;
        }
        if let Ok(val) = std::env::var("LIFE_ENGINE_CORE_LOG_LEVEL") {
            self.core.log_level = val;
        }
        if let Ok(val) = std::env::var("LIFE_ENGINE_CORE_LOG_FORMAT") {
            self.core.log_format = val;
        }
        if let Ok(val) = std::env::var("LIFE_ENGINE_CORE_DATA_DIR") {
            self.core.data_dir = val;
        }
        if let Ok(val) = std::env::var("LIFE_ENGINE_AUTH_PROVIDER") {
            self.auth.provider = val;
        }
        // OIDC env var overrides.
        if let Ok(issuer) = std::env::var("LIFE_ENGINE_OIDC_ISSUER_URL") {
            let oidc = self.auth.oidc.get_or_insert(OidcSettings {
                issuer_url: String::new(),
                client_id: String::new(),
                client_secret: None,
                jwks_uri: None,
                audience: None,
            });
            oidc.issuer_url = issuer;
        }
        if let Ok(client_id) = std::env::var("LIFE_ENGINE_OIDC_CLIENT_ID")
            && let Some(ref mut oidc) = self.auth.oidc
        {
            oidc.client_id = client_id;
        }
        if let Ok(secret) = std::env::var("LIFE_ENGINE_OIDC_CLIENT_SECRET")
            && let Some(ref mut oidc) = self.auth.oidc
        {
            oidc.client_secret = Some(secret);
        }
        // WebAuthn env var overrides.
        if let Ok(rp_name) = std::env::var("LIFE_ENGINE_WEBAUTHN_RP_NAME") {
            let wn = self.auth.webauthn.get_or_insert(WebAuthnSettings {
                rp_name: String::new(),
                rp_id: String::new(),
                rp_origin: String::new(),
                challenge_ttl_secs: default_webauthn_challenge_ttl(),
            });
            wn.rp_name = rp_name;
        }
        if let Ok(rp_id) = std::env::var("LIFE_ENGINE_WEBAUTHN_RP_ID")
            && let Some(ref mut wn) = self.auth.webauthn
        {
            wn.rp_id = rp_id;
        }
        if let Ok(rp_origin) = std::env::var("LIFE_ENGINE_WEBAUTHN_RP_ORIGIN")
            && let Some(ref mut wn) = self.auth.webauthn
        {
            wn.rp_origin = rp_origin;
        }
        if let Ok(ttl) = std::env::var("LIFE_ENGINE_WEBAUTHN_CHALLENGE_TTL")
            && let Ok(secs) = ttl.parse::<u64>()
            && let Some(ref mut wn) = self.auth.webauthn
        {
            wn.challenge_ttl_secs = secs;
        }
        if let Ok(val) = std::env::var("LIFE_ENGINE_STORAGE_BACKEND") {
            self.storage.backend = val;
        }
        if let Ok(val) = std::env::var("LIFE_ENGINE_STORAGE_ENCRYPTION")
            && let Ok(b) = val.parse::<bool>()
        {
            self.storage.encryption = b;
        }
        // PostgreSQL env var overrides.
        if let Ok(host) = std::env::var("LIFE_ENGINE_PG_HOST") {
            let pg = self.storage.postgres.get_or_insert(PostgresSettings::default());
            pg.host = host;
        }
        if let Ok(port) = std::env::var("LIFE_ENGINE_PG_PORT")
            && let Ok(p) = port.parse::<u16>()
        {
            let pg = self.storage.postgres.get_or_insert(PostgresSettings::default());
            pg.port = p;
        }
        if let Ok(dbname) = std::env::var("LIFE_ENGINE_PG_DBNAME") {
            let pg = self.storage.postgres.get_or_insert(PostgresSettings::default());
            pg.dbname = dbname;
        }
        if let Ok(user) = std::env::var("LIFE_ENGINE_PG_USER") {
            let pg = self.storage.postgres.get_or_insert(PostgresSettings::default());
            pg.user = user;
        }
        if let Ok(password) = std::env::var("LIFE_ENGINE_PG_PASSWORD") {
            let pg = self.storage.postgres.get_or_insert(PostgresSettings::default());
            pg.password = password;
        }
        if let Ok(val) = std::env::var("LIFE_ENGINE_NETWORK_RATE_LIMIT")
            && let Ok(r) = val.parse::<u32>()
        {
            self.network.rate_limit.requests_per_minute = r;
        }
        // TLS env var overrides.
        if let Ok(val) = std::env::var("LIFE_ENGINE_TLS_ENABLED")
            && let Ok(b) = val.parse::<bool>()
        {
            self.network.tls.enabled = b;
        }
        if let Ok(val) = std::env::var("LIFE_ENGINE_TLS_CERT_PATH") {
            self.network.tls.cert_path = val;
        }
        if let Ok(val) = std::env::var("LIFE_ENGINE_TLS_KEY_PATH") {
            self.network.tls.key_path = val;
        }
        // CORS env var override.
        if let Ok(val) = std::env::var("LIFE_ENGINE_CORS_ORIGINS") {
            self.network.cors.allowed_origins = val
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
        }
    }

    /// Apply CLI argument overrides (highest priority).
    fn apply_cli_overrides(&mut self, cli: &CliArgs) {
        if let Some(ref host) = cli.host {
            self.core.host.clone_from(host);
        }
        if let Some(port) = cli.port {
            self.core.port = port;
        }
        if let Some(ref level) = cli.log_level {
            self.core.log_level.clone_from(level);
        }
        if let Some(ref format) = cli.log_format {
            self.core.log_format.clone_from(format);
        }
        if let Some(ref dir) = cli.data_dir {
            self.core.data_dir.clone_from(dir);
        }
    }

    /// Validate the configuration, rejecting insecure or invalid values.
    pub fn validate(&self) -> anyhow::Result<()> {
        // Port must not be 0.
        if self.core.port == 0 {
            return Err(CoreError::Config("port must not be 0".into()).into());
        }

        // Log level must be valid.
        let valid_levels = ["trace", "debug", "info", "warn", "error"];
        if !valid_levels.contains(&self.core.log_level.to_lowercase().as_str()) {
            return Err(CoreError::Config(format!(
                "invalid log_level '{}', must be one of: {valid_levels:?}",
                self.core.log_level
            ))
            .into());
        }

        // Log format must be valid.
        let valid_formats = ["json", "pretty"];
        if !valid_formats.contains(&self.core.log_format.to_lowercase().as_str()) {
            return Err(CoreError::Config(format!(
                "invalid log_format '{}', must be one of: {valid_formats:?}",
                self.core.log_format
            ))
            .into());
        }

        // Host must not be empty.
        if self.core.host.is_empty() {
            return Err(CoreError::Config("host must not be empty".into()).into());
        }

        // Data dir must not be empty.
        if self.core.data_dir.is_empty() {
            return Err(CoreError::Config("data_dir must not be empty".into()).into());
        }

        // If TLS is enabled, cert and key paths must be provided.
        if self.network.tls.enabled {
            if self.network.tls.cert_path.is_empty() {
                return Err(
                    CoreError::Config("TLS enabled but cert_path is empty".into()).into(),
                );
            }
            if self.network.tls.key_path.is_empty() {
                return Err(
                    CoreError::Config("TLS enabled but key_path is empty".into()).into(),
                );
            }
        }

        // CORS allowed_origins must not be empty.
        if self.network.cors.allowed_origins.is_empty() {
            return Err(
                CoreError::Config("cors.allowed_origins must not be empty".into()).into(),
            );
        }

        // Rate limit must be > 0.
        if self.network.rate_limit.requests_per_minute == 0 {
            return Err(
                CoreError::Config("rate_limit.requests_per_minute must be > 0".into()).into(),
            );
        }

        Ok(())
    }

    /// Resolve the bind address as `host:port`.
    pub fn bind_address(&self) -> String {
        format!("{}:{}", self.core.host, self.core.port)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn defaults_are_valid() {
        let config = CoreConfig::default();
        assert!(config.validate().is_ok());
        assert_eq!(config.core.host, "127.0.0.1");
        assert_eq!(config.core.port, 3750);
        assert_eq!(config.core.log_level, "info");
        assert_eq!(config.core.log_format, "json");
        assert_eq!(config.auth.provider, "local-token");
        assert!(config.storage.encryption);
        assert_eq!(config.storage.argon2.memory_mb, 64);
        assert_eq!(config.storage.argon2.iterations, 3);
        assert_eq!(config.storage.argon2.parallelism, 4);
        assert!(!config.plugins.auto_enable);
        assert!(config.plugins.paths.is_empty());
        assert!(!config.network.tls.enabled);
        assert_eq!(config.network.rate_limit.requests_per_minute, 60);
    }

    #[test]
    fn bind_address_format() {
        let config = CoreConfig::default();
        assert_eq!(config.bind_address(), "127.0.0.1:3750");
    }

    #[test]
    fn yaml_loading() {
        let yaml = r#"
core:
  host: "0.0.0.0"
  port: 8080
  log_level: "debug"
  log_format: "pretty"
  data_dir: "/tmp/life-engine"
auth:
  provider: "oidc"
storage:
  encryption: false
plugins:
  paths:
    - "/opt/plugins"
  auto_enable: true
network:
  rate_limit:
    requests_per_minute: 120
"#;
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(yaml.as_bytes()).unwrap();

        let config =
            CoreConfig::load_from_yaml(file.path()).expect("should parse valid YAML config");

        assert_eq!(config.core.host, "0.0.0.0");
        assert_eq!(config.core.port, 8080);
        assert_eq!(config.core.log_level, "debug");
        assert_eq!(config.core.log_format, "pretty");
        assert_eq!(config.core.data_dir, "/tmp/life-engine");
        assert_eq!(config.auth.provider, "oidc");
        assert!(!config.storage.encryption);
        assert_eq!(config.plugins.paths, vec!["/opt/plugins"]);
        assert!(config.plugins.auto_enable);
        assert_eq!(config.network.rate_limit.requests_per_minute, 120);
    }

    #[test]
    fn partial_yaml_uses_defaults_for_missing_fields() {
        let yaml = r#"
core:
  port: 9090
"#;
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(yaml.as_bytes()).unwrap();

        let config = CoreConfig::load_from_yaml(file.path()).expect("should parse partial YAML");
        assert_eq!(config.core.port, 9090);
        // Defaults for unspecified fields.
        assert_eq!(config.core.host, "127.0.0.1");
        assert_eq!(config.core.log_level, "info");
    }

    #[test]
    fn invalid_yaml_returns_error() {
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(b"{{not yaml}}").unwrap();

        let result = CoreConfig::load_from_yaml(file.path());
        assert!(result.is_err());
    }

    #[test]
    fn validation_rejects_port_zero() {
        let mut config = CoreConfig::default();
        config.core.port = 0;
        let err = config.validate().unwrap_err();
        assert!(err.to_string().contains("port must not be 0"));
    }

    #[test]
    fn validation_rejects_invalid_log_level() {
        let mut config = CoreConfig::default();
        config.core.log_level = "verbose".into();
        let err = config.validate().unwrap_err();
        assert!(err.to_string().contains("invalid log_level"));
    }

    #[test]
    fn validation_rejects_invalid_log_format() {
        let mut config = CoreConfig::default();
        config.core.log_format = "xml".into();
        let err = config.validate().unwrap_err();
        assert!(err.to_string().contains("invalid log_format"));
    }

    #[test]
    fn validation_rejects_empty_host() {
        let mut config = CoreConfig::default();
        config.core.host = String::new();
        let err = config.validate().unwrap_err();
        assert!(err.to_string().contains("host must not be empty"));
    }

    #[test]
    fn validation_rejects_empty_data_dir() {
        let mut config = CoreConfig::default();
        config.core.data_dir = String::new();
        let err = config.validate().unwrap_err();
        assert!(err.to_string().contains("data_dir must not be empty"));
    }

    #[test]
    fn validation_rejects_tls_without_cert() {
        let mut config = CoreConfig::default();
        config.network.tls.enabled = true;
        config.network.tls.cert_path = String::new();
        config.network.tls.key_path = "/path/to/key".into();
        let err = config.validate().unwrap_err();
        assert!(err.to_string().contains("cert_path is empty"));
    }

    #[test]
    fn validation_rejects_tls_without_key() {
        let mut config = CoreConfig::default();
        config.network.tls.enabled = true;
        config.network.tls.cert_path = "/path/to/cert".into();
        config.network.tls.key_path = String::new();
        let err = config.validate().unwrap_err();
        assert!(err.to_string().contains("key_path is empty"));
    }

    #[test]
    fn validation_rejects_zero_rate_limit() {
        let mut config = CoreConfig::default();
        config.network.rate_limit.requests_per_minute = 0;
        let err = config.validate().unwrap_err();
        assert!(err
            .to_string()
            .contains("requests_per_minute must be > 0"));
    }

    #[test]
    fn env_override_port() {
        let mut config = CoreConfig::default();
        // Simulate env var by setting directly.
        config.core.port = 9999;
        assert_eq!(config.core.port, 9999);
    }

    #[test]
    fn cli_overrides_take_precedence() {
        let mut config = CoreConfig::default();
        let cli = CliArgs {
            config: String::new(),
            host: Some("0.0.0.0".into()),
            port: Some(4000),
            log_level: Some("debug".into()),
            log_format: Some("pretty".into()),
            data_dir: Some("/custom/data".into()),
        };
        config.apply_cli_overrides(&cli);
        assert_eq!(config.core.host, "0.0.0.0");
        assert_eq!(config.core.port, 4000);
        assert_eq!(config.core.log_level, "debug");
        assert_eq!(config.core.log_format, "pretty");
        assert_eq!(config.core.data_dir, "/custom/data");
    }

    #[test]
    fn cli_none_values_do_not_override() {
        let mut config = CoreConfig::default();
        let cli = CliArgs {
            config: String::new(),
            host: None,
            port: None,
            log_level: None,
            log_format: None,
            data_dir: None,
        };
        config.apply_cli_overrides(&cli);
        assert_eq!(config.core.host, "127.0.0.1");
        assert_eq!(config.core.port, 3750);
    }

    #[test]
    fn default_cors_origins_are_correct() {
        let config = CoreConfig::default();
        assert_eq!(
            config.network.cors.allowed_origins,
            vec!["http://localhost:1420"]
        );
    }

    #[test]
    fn cors_env_override_parses_comma_separated() {
        let mut config = CoreConfig::default();
        // Simulate what apply_env_overrides does for LIFE_ENGINE_CORS_ORIGINS.
        let val = "https://a.com, https://b.com , https://c.com";
        config.network.cors.allowed_origins = val
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        assert_eq!(
            config.network.cors.allowed_origins,
            vec!["https://a.com", "https://b.com", "https://c.com"]
        );
    }

    #[test]
    fn validation_rejects_empty_cors_origins() {
        let mut config = CoreConfig::default();
        config.network.cors.allowed_origins = vec![];
        let err = config.validate().unwrap_err();
        assert!(err
            .to_string()
            .contains("cors.allowed_origins must not be empty"));
    }

    #[test]
    fn yaml_with_custom_cors_origins() {
        let yaml = r#"
network:
  cors:
    allowed_origins:
      - "https://app.example.com"
      - "https://admin.example.com"
"#;
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(yaml.as_bytes()).unwrap();

        let config = CoreConfig::load_from_yaml(file.path()).unwrap();
        assert_eq!(
            config.network.cors.allowed_origins,
            vec!["https://app.example.com", "https://admin.example.com"]
        );
    }

    #[test]
    fn cors_wildcard_origin_validates() {
        let mut config = CoreConfig::default();
        config.network.cors.allowed_origins = vec!["*".into()];
        assert!(config.validate().is_ok());
    }

    #[test]
    fn serialization_roundtrip() {
        let config = CoreConfig::default();
        let yaml = serde_yaml::to_string(&config).expect("serialize");
        let restored: CoreConfig = serde_yaml::from_str(&yaml).expect("deserialize");
        assert_eq!(config.core.port, restored.core.port);
        assert_eq!(config.core.host, restored.core.host);
    }
}
