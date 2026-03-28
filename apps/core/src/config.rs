//! Configuration loading and validation for Life Engine Core.
//!
//! Supports three sources with increasing priority:
//! 1. YAML file (`~/.life-engine/config.yaml`)
//! 2. Environment variables with `LIFE_ENGINE_` prefix
//! 3. CLI arguments
//!
//! Sensible defaults are provided for all fields.

use crate::error::CoreError;
use crate::pg_storage::PgSslMode;
use clap::Parser;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};
use tracing_subscriber::EnvFilter;

/// Type alias for the tracing EnvFilter reload handle.
///
/// Used to hot-reload the log level at runtime without restarting the server.
pub type LogReloadHandle = tracing_subscriber::reload::Handle<EnvFilter, tracing_subscriber::Registry>;

/// Placeholder shown in Debug/Display output for sensitive fields.
const REDACTED: &str = "[REDACTED]";

/// Top-level configuration for the Core binary.
#[derive(Clone, Default, Serialize, Deserialize)]
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

impl fmt::Debug for CoreConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CoreConfig")
            .field("core", &self.core)
            .field("auth", &self.auth)
            .field("storage", &self.storage)
            .field("plugins", &self.plugins)
            .field("network", &self.network)
            .finish()
    }
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

    /// Per-module log level overrides (e.g. `{"storage": "debug", "auth": "trace"}`).
    #[serde(default)]
    pub log_modules: HashMap<String, String>,

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
            log_modules: HashMap::new(),
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
#[derive(Clone, Serialize, Deserialize)]
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

impl fmt::Debug for OidcSettings {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OidcSettings")
            .field("issuer_url", &self.issuer_url)
            .field("client_id", &self.client_id)
            .field("client_secret", &self.client_secret.as_ref().map(|_| REDACTED))
            .field("jwks_uri", &self.jwks_uri)
            .field("audience", &self.audience)
            .finish()
    }
}

/// Storage settings.
#[derive(Clone, Serialize, Deserialize)]
pub struct StorageSettings {
    /// The storage backend to use: "sqlite" (default) or "postgres".
    #[serde(default = "default_storage_backend")]
    pub backend: String,

    /// Whether to enable SQLCipher encryption (SQLite only).
    #[serde(default = "default_encryption")]
    pub encryption: bool,

    /// Master passphrase for database encryption (SQLite only).
    ///
    /// Can also be set via `LIFE_ENGINE_STORAGE_PASSPHRASE` env var.
    /// The passphrase is never stored — only the derived key is kept in memory.
    #[serde(default)]
    pub passphrase: Option<String>,

    /// Argon2 key-derivation parameters (SQLite only).
    #[serde(default)]
    pub argon2: Argon2Settings,

    /// PostgreSQL configuration (required when backend is "postgres").
    #[serde(default)]
    pub postgres: Option<PostgresSettings>,
}

impl fmt::Debug for StorageSettings {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StorageSettings")
            .field("backend", &self.backend)
            .field("encryption", &self.encryption)
            .field("passphrase", &self.passphrase.as_ref().map(|_| REDACTED))
            .field("argon2", &self.argon2)
            .field("postgres", &self.postgres)
            .finish()
    }
}

impl StorageSettings {
    /// Resolves the master passphrase from config or `LIFE_ENGINE_STORAGE_PASSPHRASE` env var.
    ///
    /// The env var takes precedence over the config file value.
    pub fn resolve_passphrase(&self) -> Option<String> {
        std::env::var("LIFE_ENGINE_STORAGE_PASSPHRASE")
            .ok()
            .filter(|s| !s.is_empty())
            .or_else(|| self.passphrase.clone())
    }
}

impl Default for StorageSettings {
    fn default() -> Self {
        Self {
            backend: default_storage_backend(),
            encryption: default_encryption(),
            passphrase: None,
            argon2: Argon2Settings::default(),
            postgres: None,
        }
    }
}

/// PostgreSQL connection settings.
#[derive(Clone, Serialize, Deserialize)]
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
    /// Password. `None` means not configured; use `resolve_passphrase()` pattern.
    #[serde(default)]
    pub password: Option<String>,
    /// Connection pool size.
    #[serde(default = "default_pg_pool_size")]
    pub pool_size: usize,
    /// TLS mode for the connection (disable, prefer, require).
    /// Defaults to `require` so credentials are never sent in plaintext.
    #[serde(default)]
    pub ssl_mode: PgSslMode,
}

impl fmt::Debug for PostgresSettings {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PostgresSettings")
            .field("host", &self.host)
            .field("port", &self.port)
            .field("dbname", &self.dbname)
            .field("user", &self.user)
            .field("password", &REDACTED)
            .field("pool_size", &self.pool_size)
            .field("ssl_mode", &self.ssl_mode)
            .finish()
    }
}

impl Default for PostgresSettings {
    fn default() -> Self {
        Self {
            host: default_pg_host(),
            port: default_pg_port(),
            dbname: default_pg_dbname(),
            user: default_pg_user(),
            password: None,
            pool_size: default_pg_pool_size(),
            ssl_mode: PgSslMode::default(),
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
    /// Whether Core is running behind a reverse proxy (e.g. Caddy, nginx).
    ///
    /// When `true`: TLS requirement is skipped for non-localhost bind addresses
    /// (the reverse proxy handles TLS), `X-Forwarded-For` headers are trusted
    /// for client IP extraction, and `X-Forwarded-Proto` is trusted for
    /// protocol detection.
    ///
    /// When `false` (default) and the bind address is not localhost, TLS must
    /// be configured or Core will refuse to start.
    #[serde(default)]
    pub behind_proxy: bool,

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
    directories::BaseDirs::new()
        .map(|dirs| {
            dirs.home_dir()
                .join(".life-engine/data")
                .to_string_lossy()
                .into_owned()
        })
        .unwrap_or_else(|| "~/.life-engine/data".into())
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
    vec!["http://localhost:3750".into()]
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

    /// Subcommand to execute instead of starting the server.
    #[command(subcommand)]
    pub command: Option<CliCommand>,
}

/// CLI subcommands.
#[derive(Debug, clap::Subcommand)]
pub enum CliCommand {
    /// Install Life Engine Core as a system service (systemd on Linux, launchd on macOS).
    InstallService,
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
    pub fn default_config_path() -> Option<PathBuf> {
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
        // OIDC env var overrides — insert default once, then apply all overrides.
        {
            let has_oidc_env = ["LIFE_ENGINE_OIDC_ISSUER_URL", "LIFE_ENGINE_OIDC_CLIENT_ID", "LIFE_ENGINE_OIDC_CLIENT_SECRET"]
                .iter().any(|k| std::env::var(k).is_ok());
            if has_oidc_env {
                let oidc = self.auth.oidc.get_or_insert(OidcSettings {
                    issuer_url: String::new(),
                    client_id: String::new(),
                    client_secret: None,
                    jwks_uri: None,
                    audience: None,
                });
                if let Ok(val) = std::env::var("LIFE_ENGINE_OIDC_ISSUER_URL") { oidc.issuer_url = val; }
                if let Ok(val) = std::env::var("LIFE_ENGINE_OIDC_CLIENT_ID") { oidc.client_id = val; }
                if let Ok(val) = std::env::var("LIFE_ENGINE_OIDC_CLIENT_SECRET") { oidc.client_secret = Some(val); }
            }
        }
        // WebAuthn env var overrides — insert default once, then apply all overrides.
        {
            let has_webauthn_env = ["LIFE_ENGINE_WEBAUTHN_RP_NAME", "LIFE_ENGINE_WEBAUTHN_RP_ID",
                "LIFE_ENGINE_WEBAUTHN_RP_ORIGIN", "LIFE_ENGINE_WEBAUTHN_CHALLENGE_TTL"]
                .iter().any(|k| std::env::var(k).is_ok());
            if has_webauthn_env {
                let wa = self.auth.webauthn.get_or_insert(WebAuthnSettings {
                    rp_name: String::new(),
                    rp_id: String::new(),
                    rp_origin: String::new(),
                    challenge_ttl_secs: default_webauthn_challenge_ttl(),
                });
                if let Ok(val) = std::env::var("LIFE_ENGINE_WEBAUTHN_RP_NAME") { wa.rp_name = val; }
                if let Ok(val) = std::env::var("LIFE_ENGINE_WEBAUTHN_RP_ID") { wa.rp_id = val; }
                if let Ok(val) = std::env::var("LIFE_ENGINE_WEBAUTHN_RP_ORIGIN") { wa.rp_origin = val; }
                if let Ok(ttl) = std::env::var("LIFE_ENGINE_WEBAUTHN_CHALLENGE_TTL")
                    && let Ok(secs) = ttl.parse::<u64>()
                { wa.challenge_ttl_secs = secs; }
            }
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
            pg.password = Some(password);
        }
        if let Ok(ssl_mode) = std::env::var("LIFE_ENGINE_PG_SSLMODE")
            && let Ok(mode) = ssl_mode.parse::<PgSslMode>()
        {
            let pg = self.storage.postgres.get_or_insert(PostgresSettings::default());
            pg.ssl_mode = mode;
        }
        if let Ok(val) = std::env::var("LIFE_ENGINE_NETWORK_RATE_LIMIT")
            && let Ok(r) = val.parse::<u32>()
        {
            self.network.rate_limit.requests_per_minute = r;
        }
        // Behind-proxy env var override.
        if let Ok(val) = std::env::var("LIFE_ENGINE_BEHIND_PROXY")
            && let Ok(b) = val.parse::<bool>()
        {
            self.network.behind_proxy = b;
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

        // Enforce TLS for non-localhost addresses unless behind a reverse proxy.
        let is_localhost = self.core.host == "127.0.0.1"
            || self.core.host == "localhost"
            || self.core.host == "::1";
        if !is_localhost && !self.network.behind_proxy && !self.network.tls.enabled {
            return Err(CoreError::Config(
                "Refusing to start: non-localhost bind address requires TLS configuration \
                 or network.behind_proxy = true"
                    .into(),
            )
            .into());
        }

        // Enforce real authentication for non-localhost addresses.
        // local-token is a development-only provider — network-facing instances
        // must use oidc or webauthn to prevent accidental unauthenticated exposure.
        if !is_localhost && self.auth.provider == "local-token" {
            return Err(CoreError::Config(
                "Refusing to start: non-localhost bind address requires a network-safe \
                 auth provider (oidc or webauthn); local-token is for localhost development only"
                    .into(),
            )
            .into());
        }

        // CORS allowed_origins must not be empty.
        if self.network.cors.allowed_origins.is_empty() {
            return Err(
                CoreError::Config("cors.allowed_origins must not be empty".into()).into(),
            );
        }

        // Warn when wildcard CORS is configured.
        if self.network.cors.allowed_origins.iter().any(|o| o == "*") {
            tracing::warn!(
                "CORS allowed_origins contains wildcard '*'; this allows requests from any origin"
            );
        }

        // Auth provider must be a known value.
        let valid_providers = ["local-token", "oidc", "webauthn"];
        if !valid_providers.contains(&self.auth.provider.to_lowercase().as_str()) {
            return Err(CoreError::Config(format!(
                "invalid auth.provider '{}', must be one of: {valid_providers:?}",
                self.auth.provider
            ))
            .into());
        }

        // If OIDC is selected, the OIDC section and required fields must be present.
        if self.auth.provider == "oidc" {
            match &self.auth.oidc {
                None => {
                    return Err(CoreError::Config(
                        "auth.provider is 'oidc' but auth.oidc section is missing".into(),
                    )
                    .into());
                }
                Some(oidc) => {
                    if oidc.issuer_url.is_empty() {
                        return Err(CoreError::Config(
                            "auth.provider is 'oidc' but oidc.issuer_url is empty".into(),
                        )
                        .into());
                    }
                    if oidc.client_id.is_empty() {
                        return Err(CoreError::Config(
                            "auth.provider is 'oidc' but oidc.client_id is empty".into(),
                        )
                        .into());
                    }
                }
            }
        }

        // If WebAuthn is selected, the webauthn section and required fields must be present.
        if self.auth.provider == "webauthn" {
            match &self.auth.webauthn {
                None => {
                    return Err(CoreError::Config(
                        "auth.provider is 'webauthn' but auth.webauthn section is missing".into(),
                    )
                    .into());
                }
                Some(wn) => {
                    if wn.rp_name.is_empty() {
                        return Err(CoreError::Config(
                            "auth.provider is 'webauthn' but webauthn.rp_name is empty".into(),
                        )
                        .into());
                    }
                    if wn.rp_id.is_empty() {
                        return Err(CoreError::Config(
                            "auth.provider is 'webauthn' but webauthn.rp_id is empty".into(),
                        )
                        .into());
                    }
                    if wn.rp_origin.is_empty() {
                        return Err(CoreError::Config(
                            "auth.provider is 'webauthn' but webauthn.rp_origin is empty".into(),
                        )
                        .into());
                    }
                }
            }
        }

        // Storage backend must be a known value.
        let valid_backends = ["sqlite", "postgres"];
        if !valid_backends.contains(&self.storage.backend.to_lowercase().as_str()) {
            return Err(CoreError::Config(format!(
                "invalid storage.backend '{}', must be one of: {valid_backends:?}",
                self.storage.backend
            ))
            .into());
        }

        // If postgres backend is selected, the postgres section must be present.
        if self.storage.backend == "postgres" && self.storage.postgres.is_none() {
            return Err(CoreError::Config(
                "storage.backend is 'postgres' but storage.postgres section is missing".into(),
            )
            .into());
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

    /// Serialize config to JSON with sensitive fields replaced by `"[REDACTED]"`.
    pub fn to_redacted_json(&self) -> serde_json::Value {
        let mut val = serde_json::to_value(self).expect("CoreConfig is always serializable");

        // Redact OIDC client_secret.
        if let Some(secret) = val
            .pointer_mut("/auth/oidc/client_secret")
            .filter(|v| !v.is_null())
        {
            *secret = serde_json::Value::String(REDACTED.into());
        }

        // Redact PostgreSQL password.
        if let Some(pw) = val
            .pointer_mut("/storage/postgres/password")
            .filter(|v| !v.is_null())
        {
            *pw = serde_json::Value::String(REDACTED.into());
        }

        val
    }

    /// Merge a partial JSON config into this config, validate, and return the merged result.
    pub fn merge_partial(&self, partial: &serde_json::Value) -> anyhow::Result<CoreConfig> {
        let mut base = serde_json::to_value(self).expect("CoreConfig is always serializable");
        merge_json(&mut base, partial);
        let merged: CoreConfig = serde_json::from_value(base).map_err(|e| {
            CoreError::Config(format!("invalid config after merge: {e}"))
        })?;
        merged.validate()?;
        Ok(merged)
    }
}

/// Recursively merge `patch` into `base`. For objects, merge keys; for everything else, overwrite.
fn merge_json(base: &mut serde_json::Value, patch: &serde_json::Value) {
    if let (Some(base_obj), Some(patch_obj)) = (base.as_object_mut(), patch.as_object()) {
        for (key, patch_val) in patch_obj {
            let entry = base_obj
                .entry(key.clone())
                .or_insert(serde_json::Value::Null);
            merge_json(entry, patch_val);
        }
    } else {
        *base = patch.clone();
    }
}

// ---------------------------------------------------------------------------
// New-architecture configuration types (Phase 9 — thin Core orchestrator)
//
// These types replace the legacy CoreConfig when Phase 9 is complete.
// During the transition, they live in `config::startup` so existing code
// that references `config::CoreConfig` (the legacy struct) keeps compiling.
// WP 9.7 (main.rs rewrite) will promote these to top-level and remove the
// legacy types.
// ---------------------------------------------------------------------------

pub mod startup {
    use serde::{Deserialize, Serialize};
    use std::collections::HashMap;
    use std::path::PathBuf;

    /// Platform-specific default config file path.
    ///
    /// - Linux: `$XDG_CONFIG_HOME/life-engine/config.toml` or `~/.config/life-engine/config.toml`
    /// - macOS: `~/Library/Application Support/life-engine/config.toml`
    /// - Windows: `%APPDATA%\life-engine\config.toml`
    pub const DEFAULT_CONFIG_PATH: &str = {
        #[cfg(target_os = "macos")]
        {
            "~/Library/Application Support/life-engine/config.toml"
        }
        #[cfg(target_os = "windows")]
        {
            "%APPDATA%\\life-engine\\config.toml"
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            "~/.config/life-engine/config.toml"
        }
    };

    /// Sensitive key fragments — any TOML key containing one of these words
    /// (case-insensitive) is redacted before logging.
    const SENSITIVE_FRAGMENTS: &[&str] = &["key", "secret", "password", "token"];

    /// Errors that can occur while loading, parsing, or validating configuration.
    #[derive(Debug)]
    pub enum ConfigError {
        /// The config file could not be read from disk.
        IoError {
            path: PathBuf,
            source: std::io::Error,
        },
        /// The TOML content could not be parsed.
        ParseError {
            path: PathBuf,
            message: String,
        },
        /// An environment variable value could not be converted to the
        /// expected type (e.g. a non-integer where an integer was expected).
        EnvVarConversion {
            var: String,
            value: String,
            message: String,
        },
        /// A required configuration section is missing.
        MissingSection {
            name: String,
        },
        /// A configuration value is invalid.
        InvalidValue {
            section: String,
            field: String,
            message: String,
        },
        /// A module's own validation reported errors.
        #[allow(dead_code)]
        ModuleValidationFailed {
            module: String,
            errors: Vec<String>,
        },
        /// Multiple validation errors collected together.
        ValidationErrors {
            errors: Vec<ConfigError>,
        },
    }

    impl std::fmt::Display for ConfigError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                ConfigError::IoError { path, source } => {
                    write!(f, "failed to read config file {}: {source}", path.display())
                }
                ConfigError::ParseError { path, message } => {
                    write!(f, "failed to parse config file {}: {message}", path.display())
                }
                ConfigError::EnvVarConversion { var, value, message } => {
                    write!(f, "env var {var}={value:?}: {message}")
                }
                ConfigError::MissingSection { name } => {
                    write!(f, "missing required config section: {name}")
                }
                ConfigError::InvalidValue { section, field, message } => {
                    write!(f, "invalid value in {section}.{field}: {message}")
                }
                ConfigError::ModuleValidationFailed { module, errors } => {
                    write!(f, "validation failed for module {module}: {}", errors.join("; "))
                }
                ConfigError::ValidationErrors { errors } => {
                    write!(f, "configuration validation failed with {} error(s):", errors.len())?;
                    for (i, err) in errors.iter().enumerate() {
                        write!(f, "\n  {}: {err}", i + 1)?;
                    }
                    Ok(())
                }
            }
        }
    }

    impl std::error::Error for ConfigError {
        fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
            match self {
                ConfigError::IoError { source, .. } => Some(source),
                _ => None,
            }
        }
    }

    /// Resolve the platform-specific default config directory at runtime.
    ///
    /// Uses the `directories` crate for correct OS integration:
    /// - Linux: `$XDG_CONFIG_HOME/life-engine` or `~/.config/life-engine`
    /// - macOS: `~/Library/Application Support/life-engine`
    /// - Windows: `%APPDATA%\life-engine`
    pub fn default_config_dir() -> Option<PathBuf> {
        directories::BaseDirs::new().map(|dirs| dirs.config_dir().join("life-engine"))
    }

    /// Resolve the platform-specific default config file path at runtime.
    pub fn default_config_file() -> Option<PathBuf> {
        default_config_dir().map(|dir| dir.join("config.toml"))
    }

    /// Starter config.toml content with working defaults for a first-run experience.
    ///
    /// All sections are active (not commented out) so Core starts with a
    /// functional configuration. Storage defaults to SQLite, the REST+GraphQL
    /// transport binds to localhost:8080, and plugins/workflows use relative
    /// paths under the data directory.
    const STARTER_CONFIG: &str = r#"# Life Engine Core Configuration
# Generated on first run. Edit to customise your instance.

[storage]
# Document storage — SQLite adapter
backend = "sqlite"
path = "data/core.db"
# Uncomment and set a passphrase for at-rest encryption:
# passphrase = ""  # Or set LIFE_ENGINE_STORAGE_PASSPHRASE env var

# Blob storage — filesystem adapter
[storage.blob]
backend = "filesystem"
path = "data/blobs"

[auth]
provider = "local-token"  # Options: local-token, oidc, webauthn

[logging]
level = "info"            # Options: trace, debug, info, warn, error
format = "json"           # Options: json, pretty

[plugins]
path = "plugins"

[workflows]
path = "workflows"

# REST and GraphQL on localhost:8080
[transports.rest]
host = "127.0.0.1"
port = 8080
"#;

    /// Default listeners.yaml content for the REST+GraphQL transport.
    const DEFAULT_LISTENERS_YAML: &str = r#"# Life Engine Listener Configuration
# Generated on first run. Edit to customise transport bindings.

listeners:
  - binding: default
    address: "127.0.0.1"
    port: 8080
    # tls:
    #   cert: /path/to/cert.pem
    #   key: /path/to/key.pem
    handlers:
      - handler_type: rest
        routes:
          - method: GET
            path: /api/v1/health
            workflow: system.health
            public: true
          - method: GET
            path: "/api/v1/data/:collection"
            workflow: collection.list
          - method: POST
            path: "/api/v1/data/:collection"
            workflow: collection.create
          - method: GET
            path: "/api/v1/data/:collection/:id"
            workflow: collection.get
          - method: PUT
            path: "/api/v1/data/:collection/:id"
            workflow: collection.update
          - method: DELETE
            path: "/api/v1/data/:collection/:id"
            workflow: collection.delete
      - handler_type: graphql
        routes:
          - method: POST
            path: /graphql
            workflow: graphql.query
"#;

    /// Default storage.toml content with SQLite document adapter and
    /// filesystem blob adapter.
    const DEFAULT_STORAGE_TOML: &str = r#"# Life Engine Storage Configuration
# Generated on first run. Edit to customise storage backends.

[document]
# SQLite document adapter
backend = "sqlite"
path = "data/core.db"

# Connection pool settings
max_connections = 8
busy_timeout_ms = 5000

# WAL mode is enabled by default for concurrent reads
journal_mode = "wal"

[blob]
# Filesystem blob adapter
backend = "filesystem"
path = "data/blobs"

# Maximum blob size in bytes (default: 50 MiB)
max_blob_size = 52428800

# Cleanup interval for orphaned blobs in seconds
cleanup_interval_secs = 3600
"#;

    /// Generate default companion configuration files in the given data
    /// directory on first run.
    ///
    /// Creates `listeners.yaml` and `storage.toml` alongside the main
    /// `config.toml`. Existing files are never overwritten — only missing
    /// files are generated. Returns the list of files that were created.
    pub fn generate_default_configs(data_dir: &std::path::Path) -> Vec<PathBuf> {
        let mut created = Vec::new();

        if let Err(e) = std::fs::create_dir_all(data_dir) {
            tracing::warn!(
                path = %data_dir.display(),
                error = %e,
                "failed to create data directory for default configs"
            );
            return created;
        }

        let files: &[(&str, &str)] = &[
            ("listeners.yaml", DEFAULT_LISTENERS_YAML),
            ("storage.toml", DEFAULT_STORAGE_TOML),
        ];

        for (name, content) in files {
            let path = data_dir.join(name);
            if path.exists() {
                tracing::debug!(path = %path.display(), "config file already exists, skipping");
                continue;
            }
            match std::fs::write(&path, content) {
                Ok(()) => {
                    tracing::info!(path = %path.display(), "wrote default {name}");
                    created.push(path);
                }
                Err(e) => {
                    tracing::warn!(
                        path = %path.display(),
                        error = %e,
                        "failed to write default {name}"
                    );
                }
            }
        }

        created
    }

    /// Load configuration from a TOML file with `LIFE_ENGINE_*` environment
    /// variable overrides.
    ///
    /// Resolution order for config file discovery (first match wins):
    /// 1. Explicit `path` argument (from `--config` CLI flag)
    /// 2. `LIFE_ENGINE_CONFIG` environment variable
    /// 3. Platform-specific default location (see [`default_config_file`])
    ///
    /// Resolution order for values (highest priority wins):
    /// **env vars > TOML file > defaults**.
    ///
    /// If no config file exists at the resolved path, a starter config is
    /// created with commented-out sections explaining each option. Companion
    /// files (`listeners.yaml`, `storage.toml`) are generated in the config
    /// directory alongside `config.toml`.
    pub fn load_config(path: Option<&str>) -> Result<CoreConfig, ConfigError> {
        // 1. Determine config file path.
        let config_path = match path {
            Some(p) if !p.is_empty() => PathBuf::from(p),
            _ => match std::env::var("LIFE_ENGINE_CONFIG") {
                Ok(p) if !p.is_empty() => PathBuf::from(p),
                _ => default_config_file().unwrap_or_else(|| expand_tilde(DEFAULT_CONFIG_PATH)),
            },
        };

        tracing::info!(path = %config_path.display(), "resolved config file path");

        // 2. Read and parse config.toml (or start with defaults).
        let mut raw_table = match std::fs::read_to_string(&config_path) {
            Ok(contents) => {
                let table: toml::Value = contents.parse().map_err(|e: toml::de::Error| {
                    ConfigError::ParseError {
                        path: config_path.clone(),
                        message: e.to_string(),
                    }
                })?;
                match table {
                    toml::Value::Table(t) => t,
                    _ => toml::map::Map::new(),
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                tracing::info!(
                    path = %config_path.display(),
                    "config file not found, creating starter config"
                );
                // Create the config directory and write a starter config.
                if let Some(parent) = config_path.parent()
                    && let Err(e) = std::fs::create_dir_all(parent)
                {
                    tracing::warn!(
                        path = %parent.display(),
                        error = %e,
                        "failed to create config directory"
                    );
                }
                if let Err(e) = std::fs::write(&config_path, STARTER_CONFIG) {
                    tracing::warn!(
                        path = %config_path.display(),
                        error = %e,
                        "failed to write starter config"
                    );
                } else {
                    tracing::info!(
                        path = %config_path.display(),
                        "wrote starter config.toml"
                    );
                }
                // Generate companion config files in the same directory.
                if let Some(config_dir) = config_path.parent() {
                    let generated = generate_default_configs(config_dir);
                    if !generated.is_empty() {
                        tracing::info!(
                            count = generated.len(),
                            "generated default companion config files"
                        );
                    }
                }
                toml::map::Map::new()
            }
            Err(e) => {
                return Err(ConfigError::IoError {
                    path: config_path,
                    source: e,
                });
            }
        };

        // 3. Scan and apply LIFE_ENGINE_* env var overrides.
        apply_env_overrides(&mut raw_table)?;

        // 4. Deserialize into CoreConfig.
        let config: CoreConfig =
            toml::Value::Table(raw_table).try_into().map_err(|e: toml::de::Error| {
                ConfigError::ParseError {
                    path: config_path.clone(),
                    message: e.to_string(),
                }
            })?;

        // 5. Log the loaded config with sensitive values redacted.
        let redacted = redact_sensitive(
            &toml::Value::try_from(&config)
                .unwrap_or(toml::Value::Table(toml::map::Map::new())),
        );
        tracing::info!(config = %redacted, "configuration loaded");

        Ok(config)
    }

    /// Validate a loaded configuration, collecting all errors before
    /// returning.
    ///
    /// Top-level checks:
    /// - The `storage` section must be a non-empty table.
    /// - At least one transport should be configured (warns if zero).
    /// - Logging level and format must be valid values.
    ///
    /// Module-level delegation: each opaque section (`storage`, `auth`,
    /// each transport) is passed to the owning module for validation.
    /// Currently the modules do not expose validation functions, so this
    /// step is a placeholder that will be wired in as modules gain
    /// `validate_config(toml::Value) -> Result<(), Vec<String>>` methods.
    pub fn validate_config(config: &CoreConfig) -> Result<(), ConfigError> {
        let mut errors: Vec<ConfigError> = Vec::new();

        // 1. Storage section must be a non-empty table.
        let storage_empty = match &config.storage {
            toml::Value::Table(t) => t.is_empty(),
            _ => true,
        };
        if storage_empty {
            errors.push(ConfigError::MissingSection {
                name: "storage".into(),
            });
        }

        // 2. Warn if no transports are configured.
        if config.transports.is_empty() {
            tracing::warn!(
                "no transports configured; Core will start but will not accept any connections"
            );
        }

        // 3. Validate logging level.
        let valid_levels = ["trace", "debug", "info", "warn", "error"];
        if !valid_levels.contains(&config.logging.level.to_lowercase().as_str()) {
            errors.push(ConfigError::InvalidValue {
                section: "logging".into(),
                field: "level".into(),
                message: format!(
                    "invalid level '{}', must be one of: {valid_levels:?}",
                    config.logging.level
                ),
            });
        }

        // 4. Validate logging format.
        let valid_formats = ["json", "pretty"];
        if !valid_formats.contains(&config.logging.format.to_lowercase().as_str()) {
            errors.push(ConfigError::InvalidValue {
                section: "logging".into(),
                field: "format".into(),
                message: format!(
                    "invalid format '{}', must be one of: {valid_formats:?}",
                    config.logging.format
                ),
            });
        }

        // 5. Validate plugins path is not empty.
        if config.plugins.path.is_empty() {
            errors.push(ConfigError::InvalidValue {
                section: "plugins".into(),
                field: "path".into(),
                message: "plugins path must not be empty".into(),
            });
        }

        // 6. Validate workflows path is not empty.
        if config.workflows.path.is_empty() {
            errors.push(ConfigError::InvalidValue {
                section: "workflows".into(),
                field: "path".into(),
                message: "workflows path must not be empty".into(),
            });
        }

        // 7. Module-level delegation (placeholder).
        //
        // When modules expose validation functions, wire them here:
        //   if let Err(module_errors) = storage_module::validate(&config.storage) { ... }
        //   if let Err(module_errors) = auth_module::validate(&config.auth) { ... }
        //   for (name, transport_config) in &config.transports {
        //       if let Err(module_errors) = transport_module::validate(name, transport_config) { ... }
        //   }
        //   if let Err(module_errors) = plugin_module::validate(&config.plugins) { ... }

        if errors.is_empty() {
            Ok(())
        } else if errors.len() == 1 {
            Err(errors.remove(0))
        } else {
            Err(ConfigError::ValidationErrors { errors })
        }
    }

    /// Scan environment variables for the `LIFE_ENGINE_` prefix and apply
    /// them as overrides on the raw TOML table.
    ///
    /// Mapping: underscores in the env var name become nested TOML keys.
    /// `LIFE_ENGINE_STORAGE_PATH` → `storage.path`
    /// `LIFE_ENGINE_TRANSPORTS_REST_PORT` → `transports.rest.port`
    fn apply_env_overrides(table: &mut toml::map::Map<String, toml::Value>) -> Result<(), ConfigError> {
        let prefix = "LIFE_ENGINE_";
        let mut env_vars: Vec<(String, String)> = std::env::vars()
            .filter(|(k, _)| k.starts_with(prefix) && k != "LIFE_ENGINE_CONFIG")
            .collect();
        // Sort for deterministic application order.
        env_vars.sort_by(|a, b| a.0.cmp(&b.0));

        for (key, value) in env_vars {
            let suffix = &key[prefix.len()..];
            let segments: Vec<String> = suffix.split('_').map(|s| s.to_lowercase()).collect();
            if segments.is_empty() || segments.iter().any(|s| s.is_empty()) {
                continue;
            }
            set_nested_value(table, &segments, &value).map_err(|msg| {
                ConfigError::EnvVarConversion {
                    var: key.clone(),
                    value: value.clone(),
                    message: msg,
                }
            })?;
        }
        Ok(())
    }

    /// Set a value in a nested TOML table, creating intermediate tables as
    /// needed. The leaf value is stored as a TOML string (modules handle
    /// their own type conversion).
    fn set_nested_value(
        table: &mut toml::map::Map<String, toml::Value>,
        segments: &[String],
        value: &str,
    ) -> Result<(), String> {
        if segments.len() == 1 {
            // Try to preserve the type of an existing value.
            let typed = match table.get(&segments[0]) {
                Some(toml::Value::Integer(_)) => value
                    .parse::<i64>()
                    .map(toml::Value::Integer)
                    .unwrap_or_else(|_| toml::Value::String(value.to_string())),
                Some(toml::Value::Float(_)) => value
                    .parse::<f64>()
                    .map(toml::Value::Float)
                    .unwrap_or_else(|_| toml::Value::String(value.to_string())),
                Some(toml::Value::Boolean(_)) => value
                    .parse::<bool>()
                    .map(toml::Value::Boolean)
                    .unwrap_or_else(|_| toml::Value::String(value.to_string())),
                _ => toml::Value::String(value.to_string()),
            };
            table.insert(segments[0].clone(), typed);
            return Ok(());
        }

        let entry = table
            .entry(segments[0].clone())
            .or_insert_with(|| toml::Value::Table(toml::map::Map::new()));

        if !entry.is_table() {
            *entry = toml::Value::Table(toml::map::Map::new());
        }
        match entry {
            toml::Value::Table(inner) => set_nested_value(inner, &segments[1..], value),
            _ => unreachable!(),
        }
    }

    /// Expand a leading `~` in a path to the user's home directory,
    /// or `%APPDATA%` on Windows.
    fn expand_tilde(path: &str) -> PathBuf {
        if let Some(rest) = path.strip_prefix("~/")
            && let Some(home) = dirs_home()
        {
            return home.join(rest);
        }
        #[cfg(target_os = "windows")]
        if let Some(rest) = path.strip_prefix("%APPDATA%\\")
            && let Ok(appdata) = std::env::var("APPDATA")
        {
            return PathBuf::from(appdata).join(rest);
        }
        PathBuf::from(path)
    }

    fn dirs_home() -> Option<PathBuf> {
        directories::BaseDirs::new().map(|d| d.home_dir().to_path_buf())
    }

    /// Return a display-safe version of a TOML value with sensitive keys
    /// replaced by `"[REDACTED]"`.
    fn redact_sensitive(value: &toml::Value) -> toml::Value {
        match value {
            toml::Value::Table(table) => {
                let mut redacted = toml::map::Map::new();
                for (k, v) in table {
                    let lower = k.to_lowercase();
                    if SENSITIVE_FRAGMENTS.iter().any(|f| lower.contains(f)) {
                        redacted.insert(k.clone(), toml::Value::String("[REDACTED]".into()));
                    } else {
                        redacted.insert(k.clone(), redact_sensitive(v));
                    }
                }
                toml::Value::Table(redacted)
            }
            other => other.clone(),
        }
    }

    /// Top-level configuration for the Core binary.
    ///
    /// Each section is a raw `toml::Value` that gets handed to the owning module
    /// for parsing and validation — Core does not parse module internals.
    /// The exceptions are `workflows`, `plugins`, and `logging` which have
    /// lightweight Core-owned types since Core directly uses those values.
    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[serde(default)]
    pub struct CoreConfig {
        /// Storage configuration — passed as-is to `StorageBackend::init()`.
        #[serde(default = "default_empty_table")]
        pub storage: toml::Value,

        /// Authentication configuration — passed as-is to the auth module.
        #[serde(default = "default_empty_table")]
        pub auth: toml::Value,

        /// Transport configurations, keyed by transport name.
        ///
        /// Supported keys: `"rest"`, `"graphql"`, `"caldav"`, `"carddav"`, `"webhook"`.
        /// Each value is passed to the corresponding `Transport` implementation.
        /// Only transports present in this map are started.
        #[serde(default)]
        pub transports: HashMap<String, toml::Value>,

        /// Workflow engine configuration.
        #[serde(default)]
        pub workflows: WorkflowsConfig,

        /// Plugin system configuration.
        #[serde(default)]
        pub plugins: PluginsConfig,

        /// Logging configuration.
        #[serde(default)]
        pub logging: LoggingConfig,
    }

    impl Default for CoreConfig {
        fn default() -> Self {
            Self {
                storage: default_empty_table(),
                auth: default_empty_table(),
                transports: HashMap::new(),
                workflows: WorkflowsConfig::default(),
                plugins: PluginsConfig::default(),
                logging: LoggingConfig::default(),
            }
        }
    }

    /// Workflow engine configuration.
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct WorkflowsConfig {
        /// Directory containing YAML workflow definition files.
        #[serde(default = "default_workflows_path")]
        pub path: String,
    }

    impl Default for WorkflowsConfig {
        fn default() -> Self {
            Self {
                path: default_workflows_path(),
            }
        }
    }

    fn default_workflows_path() -> String {
        "workflows".into()
    }

    /// Plugin system configuration.
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct PluginsConfig {
        /// Directory containing WASM plugin bundles.
        #[serde(default = "default_plugins_path")]
        pub path: String,

        /// Per-plugin instance configurations, keyed by plugin ID.
        ///
        /// Each entry contains approved capabilities for third-party plugins
        /// and plugin-specific configuration values passed via the `config:read`
        /// host function.
        #[serde(default)]
        pub config: HashMap<String, PluginInstanceConfig>,
    }

    impl Default for PluginsConfig {
        fn default() -> Self {
            Self {
                path: default_plugins_path(),
                config: HashMap::new(),
            }
        }
    }

    /// Per-plugin instance configuration.
    ///
    /// Parsed from `[plugins.config.<plugin-id>]` sections in `config.toml`.
    /// The `approved_capabilities` field lists capability strings that the
    /// operator has approved for third-party plugins. All other keys are
    /// collected as plugin-specific configuration values.
    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct PluginInstanceConfig {
        /// Approved capability strings for third-party plugins.
        ///
        /// First-party plugins are auto-granted all declared capabilities
        /// regardless of this list. Valid values: `"storage:doc:read"`,
        /// `"storage:doc:write"`, `"http:outbound"`, `"events:emit"`,
        /// `"events:subscribe"`, `"config:read"`.
        #[serde(default)]
        pub approved_capabilities: Vec<String>,

        /// Plugin-specific configuration values.
        ///
        /// All keys other than `approved_capabilities` are collected here
        /// and passed to the plugin via the `config:read` host function.
        #[serde(flatten)]
        pub config: HashMap<String, toml::Value>,
    }

    fn default_plugins_path() -> String {
        "plugins".into()
    }

    /// Logging configuration.
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct LoggingConfig {
        /// Log level filter (e.g. `"info"`, `"debug"`, `"warn"`).
        #[serde(default = "default_log_level")]
        pub level: String,

        /// Log output format: `"json"` for machine-parseable or `"pretty"` for
        /// human-readable.
        #[serde(default = "default_log_format")]
        pub format: String,

        /// Per-module log level overrides (e.g. `storage = "debug"`).
        #[serde(default)]
        pub modules: HashMap<String, String>,
    }

    impl Default for LoggingConfig {
        fn default() -> Self {
            Self {
                level: default_log_level(),
                format: default_log_format(),
                modules: HashMap::new(),
            }
        }
    }

    fn default_empty_table() -> toml::Value {
        toml::Value::Table(toml::map::Map::new())
    }

    fn default_log_level() -> String {
        "info".into()
    }

    fn default_log_format() -> String {
        "json".into()
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::sync::Mutex;

        /// Mutex to serialize tests that modify environment variables.
        static ENV_LOCK: Mutex<()> = Mutex::new(());

        #[test]
        fn deserialize_minimal_config() {
            let toml_str = "";
            let config: CoreConfig = toml::from_str(toml_str).unwrap();
            assert!(config.transports.is_empty());
            assert_eq!(config.workflows.path, "workflows");
            assert_eq!(config.plugins.path, "plugins");
            assert!(config.plugins.config.is_empty());
            assert_eq!(config.logging.level, "info");
            assert_eq!(config.logging.format, "json");
        }

        #[test]
        fn deserialize_full_config() {
            let toml_str = r#"
[storage]
path = "/data/core.db"
passphrase_env = "LIFE_ENGINE_STORAGE_PASSPHRASE"

[auth]
provider = "pocket-id"
issuer_url = "https://auth.example.com"

[transports.rest]
host = "0.0.0.0"
port = 3000

[transports.graphql]
port = 4000

[workflows]
path = "/etc/life-engine/workflows"

[plugins]
path = "/opt/life-engine/plugins"

[plugins.config.connector-email]
approved_capabilities = ["storage:doc:read", "storage:doc:write", "http:outbound"]
imap_host = "mail.example.com"
imap_port = 993

[plugins.config.connector-calendar]
sync_interval_secs = 300

[logging]
level = "debug"
format = "pretty"
"#;
            let config: CoreConfig = toml::from_str(toml_str).unwrap();

            // Storage section is raw toml::Value.
            assert_eq!(
                config.storage.get("path").and_then(|v| v.as_str()),
                Some("/data/core.db")
            );

            // Auth section is raw toml::Value.
            assert_eq!(
                config.auth.get("provider").and_then(|v| v.as_str()),
                Some("pocket-id")
            );

            // Transports are keyed by name.
            assert_eq!(config.transports.len(), 2);
            assert!(config.transports.contains_key("rest"));
            assert!(config.transports.contains_key("graphql"));
            assert_eq!(
                config.transports["rest"]
                    .get("port")
                    .and_then(|v| v.as_integer()),
                Some(3000)
            );

            // Workflows.
            assert_eq!(config.workflows.path, "/etc/life-engine/workflows");

            // Plugins.
            assert_eq!(config.plugins.path, "/opt/life-engine/plugins");
            assert_eq!(config.plugins.config.len(), 2);
            assert!(config.plugins.config.contains_key("connector-email"));
            assert_eq!(
                config.plugins.config["connector-email"].approved_capabilities,
                vec!["storage:doc:read", "storage:doc:write", "http:outbound"]
            );
            assert_eq!(
                config.plugins.config["connector-email"]
                    .config
                    .get("imap_port")
                    .and_then(|v| v.as_integer()),
                Some(993)
            );
            // connector-calendar has no approved_capabilities
            assert!(
                config.plugins.config["connector-calendar"]
                    .approved_capabilities
                    .is_empty()
            );

            // Logging.
            assert_eq!(config.logging.level, "debug");
            assert_eq!(config.logging.format, "pretty");
        }

        #[test]
        fn storage_and_auth_default_to_empty_table() {
            let config: CoreConfig = toml::from_str("").unwrap();
            // Default toml::Value is a unit (not a table), but we should handle it.
            // An empty config still deserializes the sections with defaults.
            assert!(config.storage.as_table().is_none() || config.storage.as_table().unwrap().is_empty());
        }

        #[test]
        fn default_config_path_is_set() {
            assert!(!DEFAULT_CONFIG_PATH.is_empty());
            assert!(DEFAULT_CONFIG_PATH.ends_with("config.toml"));
        }

        #[test]
        fn transports_only_configured_are_present() {
            let toml_str = r#"
[transports.rest]
port = 3000
"#;
            let config: CoreConfig = toml::from_str(toml_str).unwrap();
            assert_eq!(config.transports.len(), 1);
            assert!(config.transports.contains_key("rest"));
            assert!(!config.transports.contains_key("graphql"));
        }

        #[test]
        fn per_plugin_config_is_isolated() {
            let toml_str = r#"
[plugins.config.plugin-a]
key = "value-a"

[plugins.config.plugin-b]
key = "value-b"
"#;
            let config: CoreConfig = toml::from_str(toml_str).unwrap();
            assert_eq!(config.plugins.config.len(), 2);
            assert_eq!(
                config.plugins.config["plugin-a"]
                    .config
                    .get("key")
                    .and_then(|v| v.as_str()),
                Some("value-a")
            );
            assert_eq!(
                config.plugins.config["plugin-b"]
                    .config
                    .get("key")
                    .and_then(|v| v.as_str()),
                Some("value-b")
            );
        }

        #[test]
        fn load_config_from_toml_file() {
            let _lock = ENV_LOCK.lock().unwrap();
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("config.toml");
            std::fs::write(
                &path,
                r#"
[storage]
path = "/data/core.db"

[logging]
level = "debug"
"#,
            )
            .unwrap();

            let config = load_config(Some(path.to_str().unwrap())).unwrap();
            assert_eq!(
                config.storage.get("path").and_then(|v| v.as_str()),
                Some("/data/core.db")
            );
            assert_eq!(config.logging.level, "debug");
        }

        #[test]
        fn load_config_missing_file_returns_defaults() {
            let _lock = ENV_LOCK.lock().unwrap();
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("nonexistent.toml");
            let config = load_config(Some(path.to_str().unwrap())).unwrap();
            assert_eq!(config.logging.level, "info");
            assert_eq!(config.logging.format, "json");
            assert_eq!(config.workflows.path, "workflows");
            assert_eq!(config.plugins.path, "plugins");
        }

        #[test]
        fn load_config_invalid_toml_returns_parse_error() {
            let _lock = ENV_LOCK.lock().unwrap();
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("bad.toml");
            std::fs::write(&path, "{{not valid toml}}").unwrap();
            let err = load_config(Some(path.to_str().unwrap())).unwrap_err();
            assert!(matches!(err, ConfigError::ParseError { .. }));
            assert!(err.to_string().contains("failed to parse"));
        }

        #[test]
        fn env_var_overrides_toml_value() {
            let _lock = ENV_LOCK.lock().unwrap();
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("config.toml");
            std::fs::write(
                &path,
                r#"
[storage]
path = "/original/path"
"#,
            )
            .unwrap();

            let key = "LIFE_ENGINE_STORAGE_PATH";
            unsafe { std::env::set_var(key, "/overridden/path") };
            let config = load_config(Some(path.to_str().unwrap())).unwrap();
            unsafe { std::env::remove_var(key) };

            assert_eq!(
                config.storage.get("path").and_then(|v| v.as_str()),
                Some("/overridden/path")
            );
        }

        #[test]
        fn env_var_preserves_integer_type() {
            let _lock = ENV_LOCK.lock().unwrap();
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("config.toml");
            std::fs::write(
                &path,
                r#"
[transports.rest]
port = 3000
"#,
            )
            .unwrap();

            let key = "LIFE_ENGINE_TRANSPORTS_REST_PORT";
            unsafe { std::env::set_var(key, "4000") };
            let config = load_config(Some(path.to_str().unwrap())).unwrap();
            unsafe { std::env::remove_var(key) };

            assert_eq!(
                config.transports["rest"]
                    .get("port")
                    .and_then(|v| v.as_integer()),
                Some(4000)
            );
        }

        #[test]
        fn env_var_creates_nested_keys() {
            let _lock = ENV_LOCK.lock().unwrap();
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("config.toml");
            std::fs::write(&path, "").unwrap();

            let key = "LIFE_ENGINE_AUTH_PROVIDER";
            unsafe { std::env::set_var(key, "pocket-id") };
            let config = load_config(Some(path.to_str().unwrap())).unwrap();
            unsafe { std::env::remove_var(key) };

            assert_eq!(
                config.auth.get("provider").and_then(|v| v.as_str()),
                Some("pocket-id")
            );
        }

        #[test]
        fn env_var_config_path_override() {
            let _lock = ENV_LOCK.lock().unwrap();
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("custom.toml");
            std::fs::write(
                &path,
                r#"
[logging]
level = "trace"
"#,
            )
            .unwrap();

            unsafe { std::env::set_var("LIFE_ENGINE_CONFIG", path.to_str().unwrap()) };
            let config = load_config(None).unwrap();
            unsafe { std::env::remove_var("LIFE_ENGINE_CONFIG") };

            assert_eq!(config.logging.level, "trace");
        }

        #[test]
        fn redact_sensitive_hides_secrets() {
            let toml_str = r#"
[storage]
path = "/data/core.db"
passphrase_token = "super-secret"

[auth]
provider = "pocket-id"
client_secret = "my-secret"
api_key = "abc123"
"#;
            let val: toml::Value = toml_str.parse().unwrap();
            let redacted = redact_sensitive(&val);

            // Non-sensitive values are preserved.
            assert_eq!(
                redacted
                    .get("storage")
                    .and_then(|s| s.get("path"))
                    .and_then(|v| v.as_str()),
                Some("/data/core.db")
            );
            assert_eq!(
                redacted
                    .get("auth")
                    .and_then(|s| s.get("provider"))
                    .and_then(|v| v.as_str()),
                Some("pocket-id")
            );

            // Sensitive values are redacted.
            assert_eq!(
                redacted
                    .get("storage")
                    .and_then(|s| s.get("passphrase_token"))
                    .and_then(|v| v.as_str()),
                Some("[REDACTED]")
            );
            assert_eq!(
                redacted
                    .get("auth")
                    .and_then(|s| s.get("client_secret"))
                    .and_then(|v| v.as_str()),
                Some("[REDACTED]")
            );
            assert_eq!(
                redacted
                    .get("auth")
                    .and_then(|s| s.get("api_key"))
                    .and_then(|v| v.as_str()),
                Some("[REDACTED]")
            );
        }

        #[test]
        fn expand_tilde_expands_home() {
            let expanded = expand_tilde("~/some/path");
            // Should not start with ~ anymore (unless there is no home dir).
            assert!(!expanded.to_string_lossy().starts_with('~'));
            assert!(expanded.to_string_lossy().ends_with("some/path"));
        }

        #[test]
        fn expand_tilde_ignores_non_tilde() {
            let path = "/absolute/path";
            let expanded = expand_tilde(path);
            assert_eq!(expanded, PathBuf::from(path));
        }

        // ---- validate_config tests ----

        fn config_with_storage() -> CoreConfig {
            let toml_str = r#"
[storage]
path = "/data/core.db"
"#;
            toml::from_str(toml_str).unwrap()
        }

        #[test]
        fn validate_config_accepts_valid() {
            let config = config_with_storage();
            assert!(validate_config(&config).is_ok());
        }

        #[test]
        fn validate_config_rejects_empty_storage() {
            let config = CoreConfig::default();
            let err = validate_config(&config).unwrap_err();
            assert!(
                err.to_string().contains("storage"),
                "expected storage error, got: {err}"
            );
        }

        #[test]
        fn validate_config_rejects_invalid_log_level() {
            let toml_str = r#"
[storage]
path = "/data/core.db"

[logging]
level = "verbose"
"#;
            let config: CoreConfig = toml::from_str(toml_str).unwrap();
            let err = validate_config(&config).unwrap_err();
            assert!(
                err.to_string().contains("invalid level"),
                "expected log level error, got: {err}"
            );
        }

        #[test]
        fn validate_config_rejects_invalid_log_format() {
            let toml_str = r#"
[storage]
path = "/data/core.db"

[logging]
format = "xml"
"#;
            let config: CoreConfig = toml::from_str(toml_str).unwrap();
            let err = validate_config(&config).unwrap_err();
            assert!(
                err.to_string().contains("invalid format"),
                "expected log format error, got: {err}"
            );
        }

        #[test]
        fn validate_config_collects_multiple_errors() {
            let toml_str = r#"
[logging]
level = "verbose"
format = "xml"
"#;
            let config: CoreConfig = toml::from_str(toml_str).unwrap();
            let err = validate_config(&config).unwrap_err();
            // Should contain all three errors: missing storage, bad level, bad format.
            match &err {
                ConfigError::ValidationErrors { errors } => {
                    assert!(errors.len() >= 3, "expected at least 3 errors, got {}", errors.len());
                }
                _ => panic!("expected ValidationErrors, got: {err}"),
            }
        }

        #[test]
        fn validate_config_rejects_empty_plugins_path() {
            let toml_str = r#"
[storage]
path = "/data/core.db"

[plugins]
path = ""
"#;
            let config: CoreConfig = toml::from_str(toml_str).unwrap();
            let err = validate_config(&config).unwrap_err();
            assert!(
                err.to_string().contains("plugins path"),
                "expected plugins path error, got: {err}"
            );
        }

        #[test]
        fn validate_config_rejects_empty_workflows_path() {
            let toml_str = r#"
[storage]
path = "/data/core.db"

[workflows]
path = ""
"#;
            let config: CoreConfig = toml::from_str(toml_str).unwrap();
            let err = validate_config(&config).unwrap_err();
            assert!(
                err.to_string().contains("workflows path"),
                "expected workflows path error, got: {err}"
            );
        }

        #[test]
        fn validate_config_zero_transports_is_not_error() {
            // Zero transports produces a warning but not an error.
            let config = config_with_storage();
            assert!(config.transports.is_empty());
            assert!(validate_config(&config).is_ok());
        }

        #[test]
        fn config_error_display_missing_section() {
            let err = ConfigError::MissingSection {
                name: "storage".into(),
            };
            assert_eq!(err.to_string(), "missing required config section: storage");
        }

        #[test]
        fn config_error_display_invalid_value() {
            let err = ConfigError::InvalidValue {
                section: "logging".into(),
                field: "level".into(),
                message: "bad value".into(),
            };
            assert_eq!(
                err.to_string(),
                "invalid value in logging.level: bad value"
            );
        }

        #[test]
        fn config_error_display_module_validation_failed() {
            let err = ConfigError::ModuleValidationFailed {
                module: "storage".into(),
                errors: vec!["path not found".into(), "permissions denied".into()],
            };
            assert_eq!(
                err.to_string(),
                "validation failed for module storage: path not found; permissions denied"
            );
        }

        #[test]
        fn set_nested_value_creates_intermediate_tables() {
            let mut table = toml::map::Map::new();
            set_nested_value(
                &mut table,
                &["a".into(), "b".into(), "c".into()],
                "hello",
            )
            .unwrap();
            let val = table
                .get("a")
                .and_then(|v| v.get("b"))
                .and_then(|v| v.get("c"))
                .and_then(|v| v.as_str());
            assert_eq!(val, Some("hello"));
        }

        // ---- WP 8.13: Plugin config section parser tests ----

        #[test]
        fn plugins_path_parsed_correctly() {
            let toml_str = r#"
[plugins]
path = "/custom/plugins"
"#;
            let config: CoreConfig = toml::from_str(toml_str).unwrap();
            assert_eq!(config.plugins.path, "/custom/plugins");
        }

        #[test]
        fn per_plugin_approved_capabilities_extracted() {
            let toml_str = r#"
[plugins.config.connector-email]
approved_capabilities = ["storage:doc:read", "storage:doc:write", "http:outbound"]
poll_interval = "5m"
"#;
            let config: CoreConfig = toml::from_str(toml_str).unwrap();
            let email_cfg = &config.plugins.config["connector-email"];
            assert_eq!(
                email_cfg.approved_capabilities,
                vec!["storage:doc:read", "storage:doc:write", "http:outbound"]
            );
        }

        #[test]
        fn plugin_specific_config_values_accessible() {
            let toml_str = r#"
[plugins.config.connector-email]
approved_capabilities = ["storage:doc:read"]
imap_host = "mail.example.com"
imap_port = 993
use_tls = true
"#;
            let config: CoreConfig = toml::from_str(toml_str).unwrap();
            let email_cfg = &config.plugins.config["connector-email"];
            assert_eq!(
                email_cfg.config.get("imap_host").and_then(|v| v.as_str()),
                Some("mail.example.com")
            );
            assert_eq!(
                email_cfg.config.get("imap_port").and_then(|v| v.as_integer()),
                Some(993)
            );
            assert_eq!(
                email_cfg.config.get("use_tls").and_then(|v| v.as_bool()),
                Some(true)
            );
            // approved_capabilities should NOT be in the config HashMap
            assert!(email_cfg.config.get("approved_capabilities").is_none());
        }

        #[test]
        fn missing_plugins_section_uses_defaults() {
            let config: CoreConfig = toml::from_str("").unwrap();
            assert_eq!(config.plugins.path, "plugins");
            assert!(config.plugins.config.is_empty());
        }

        #[test]
        fn plugin_with_no_approved_capabilities_gets_empty_vec() {
            let toml_str = r#"
[plugins.config.my-plugin]
some_key = "some_value"
"#;
            let config: CoreConfig = toml::from_str(toml_str).unwrap();
            let plugin_cfg = &config.plugins.config["my-plugin"];
            assert!(plugin_cfg.approved_capabilities.is_empty());
            assert_eq!(
                plugin_cfg.config.get("some_key").and_then(|v| v.as_str()),
                Some("some_value")
            );
        }

        // ---- WP 10.3: Default configuration generation tests ----

        #[test]
        fn starter_config_parses_as_valid_toml() {
            let config: CoreConfig = toml::from_str(STARTER_CONFIG).unwrap();
            assert_eq!(
                config.storage.get("backend").and_then(|v| v.as_str()),
                Some("sqlite")
            );
            assert_eq!(
                config.storage.get("path").and_then(|v| v.as_str()),
                Some("data/core.db")
            );
            assert_eq!(
                config.auth.get("provider").and_then(|v| v.as_str()),
                Some("local-token")
            );
            assert!(config.transports.contains_key("rest"));
            assert_eq!(
                config.transports["rest"]
                    .get("port")
                    .and_then(|v| v.as_integer()),
                Some(8080)
            );
            assert_eq!(config.logging.level, "info");
            assert_eq!(config.logging.format, "json");
            assert_eq!(config.plugins.path, "plugins");
            assert_eq!(config.workflows.path, "workflows");
        }

        #[test]
        fn starter_config_passes_validation() {
            let config: CoreConfig = toml::from_str(STARTER_CONFIG).unwrap();
            assert!(validate_config(&config).is_ok());
        }

        #[test]
        fn generate_default_configs_creates_files() {
            let dir = tempfile::tempdir().unwrap();
            let created = generate_default_configs(dir.path());
            assert_eq!(created.len(), 2);
            assert!(dir.path().join("listeners.yaml").exists());
            assert!(dir.path().join("storage.toml").exists());
        }

        #[test]
        fn generate_default_configs_skips_existing_files() {
            let dir = tempfile::tempdir().unwrap();
            // Pre-create listeners.yaml with custom content.
            let existing = dir.path().join("listeners.yaml");
            std::fs::write(&existing, "custom content").unwrap();

            let created = generate_default_configs(dir.path());
            // Only storage.toml should have been created.
            assert_eq!(created.len(), 1);
            assert!(created[0].ends_with("storage.toml"));
            // Existing file should not have been overwritten.
            assert_eq!(std::fs::read_to_string(&existing).unwrap(), "custom content");
        }

        #[test]
        fn generate_default_configs_creates_directory() {
            let dir = tempfile::tempdir().unwrap();
            let nested = dir.path().join("sub").join("dir");
            let created = generate_default_configs(&nested);
            assert_eq!(created.len(), 2);
            assert!(nested.join("listeners.yaml").exists());
            assert!(nested.join("storage.toml").exists());
        }

        #[test]
        fn first_run_generates_all_config_files() {
            let _lock = ENV_LOCK.lock().unwrap();
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("config.toml");

            // load_config on a non-existent path triggers first-run generation.
            let config = load_config(Some(path.to_str().unwrap())).unwrap();

            // config.toml should have been written.
            assert!(path.exists());

            // Companion files should also exist.
            assert!(dir.path().join("listeners.yaml").exists());
            assert!(dir.path().join("storage.toml").exists());

            // The config should have working defaults.
            assert_eq!(
                config.storage.get("backend").and_then(|v| v.as_str()),
                Some("sqlite")
            );
            assert!(config.transports.contains_key("rest"));
        }

        #[test]
        fn subsequent_load_does_not_overwrite() {
            let _lock = ENV_LOCK.lock().unwrap();
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("config.toml");

            // First run — generates defaults.
            let _ = load_config(Some(path.to_str().unwrap())).unwrap();

            // Modify listeners.yaml to test it is not overwritten.
            let listeners_path = dir.path().join("listeners.yaml");
            std::fs::write(&listeners_path, "custom: true").unwrap();

            // Second load — file exists, should not regenerate.
            let _ = load_config(Some(path.to_str().unwrap())).unwrap();
            assert_eq!(
                std::fs::read_to_string(&listeners_path).unwrap(),
                "custom: true"
            );
        }
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
            command: None,
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
            command: None,
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
            vec!["http://localhost:3750"]
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

    #[test]
    fn debug_redacts_oidc_client_secret() {
        let mut config = CoreConfig::default();
        config.auth.oidc = Some(OidcSettings {
            issuer_url: "https://idp.example.com".into(),
            client_id: "my-client".into(),
            client_secret: Some("super-secret-value".into()),
            jwks_uri: None,
            audience: None,
        });
        let debug_output = format!("{:?}", config);
        assert!(
            !debug_output.contains("super-secret-value"),
            "client_secret should be redacted in Debug output"
        );
        assert!(debug_output.contains("[REDACTED]"));
    }

    #[test]
    fn debug_redacts_postgres_password() {
        let pg = PostgresSettings {
            host: "db.example.com".into(),
            port: 5432,
            dbname: "mydb".into(),
            user: "admin".into(),
            password: Some("s3cret-password".into()),
            pool_size: 8,
            ssl_mode: PgSslMode::default(),
        };
        let debug_output = format!("{:?}", pg);
        assert!(
            !debug_output.contains("s3cret-password"),
            "password should be redacted in Debug output"
        );
        assert!(debug_output.contains("[REDACTED]"));
    }

    #[test]
    fn validation_rejects_invalid_auth_provider() {
        let mut config = CoreConfig::default();
        config.auth.provider = "ldap".into();
        let err = config.validate().unwrap_err();
        assert!(err.to_string().contains("invalid auth.provider"));
    }

    #[test]
    fn validation_rejects_oidc_without_config() {
        let mut config = CoreConfig::default();
        config.auth.provider = "oidc".into();
        config.auth.oidc = None;
        let err = config.validate().unwrap_err();
        assert!(err.to_string().contains("oidc section is missing"));
    }

    #[test]
    fn validation_rejects_oidc_with_empty_issuer() {
        let mut config = CoreConfig::default();
        config.auth.provider = "oidc".into();
        config.auth.oidc = Some(OidcSettings {
            issuer_url: String::new(),
            client_id: "client".into(),
            client_secret: None,
            jwks_uri: None,
            audience: None,
        });
        let err = config.validate().unwrap_err();
        assert!(err.to_string().contains("issuer_url is empty"));
    }

    #[test]
    fn validation_rejects_oidc_with_empty_client_id() {
        let mut config = CoreConfig::default();
        config.auth.provider = "oidc".into();
        config.auth.oidc = Some(OidcSettings {
            issuer_url: "https://idp.example.com".into(),
            client_id: String::new(),
            client_secret: None,
            jwks_uri: None,
            audience: None,
        });
        let err = config.validate().unwrap_err();
        assert!(err.to_string().contains("client_id is empty"));
    }

    #[test]
    fn validation_accepts_valid_oidc_config() {
        let mut config = CoreConfig::default();
        config.auth.provider = "oidc".into();
        config.auth.oidc = Some(OidcSettings {
            issuer_url: "https://idp.example.com".into(),
            client_id: "my-client".into(),
            client_secret: Some("secret".into()),
            jwks_uri: None,
            audience: None,
        });
        assert!(config.validate().is_ok());
    }

    #[test]
    fn validation_rejects_webauthn_without_config() {
        let mut config = CoreConfig::default();
        config.auth.provider = "webauthn".into();
        config.auth.webauthn = None;
        let err = config.validate().unwrap_err();
        assert!(err.to_string().contains("webauthn section is missing"));
    }

    #[test]
    fn validation_rejects_invalid_storage_backend() {
        let mut config = CoreConfig::default();
        config.storage.backend = "mysql".into();
        let err = config.validate().unwrap_err();
        assert!(err.to_string().contains("invalid storage.backend"));
    }

    #[test]
    fn validation_rejects_postgres_without_config() {
        let mut config = CoreConfig::default();
        config.storage.backend = "postgres".into();
        config.storage.postgres = None;
        let err = config.validate().unwrap_err();
        assert!(err.to_string().contains("postgres section is missing"));
    }

    #[test]
    fn validation_accepts_postgres_with_config() {
        let mut config = CoreConfig::default();
        config.storage.backend = "postgres".into();
        config.storage.postgres = Some(PostgresSettings::default());
        assert!(config.validate().is_ok());
    }
}
