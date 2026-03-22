/** Core server settings. */
export interface CoreSettings {
  host: string;
  port: number;
  log_level: string;
  log_format: string;
  data_dir: string;
}

/** OIDC-specific configuration for identity provider integration. */
export interface OidcSettings {
  issuer_url: string;
  client_id: string;
  /** Redacted by the API — always "[REDACTED]" when present. */
  client_secret: string | null;
  jwks_uri: string | null;
  audience: string | null;
}

/** WebAuthn-specific configuration for passkey authentication. */
export interface WebAuthnSettings {
  rp_name: string;
  rp_id: string;
  rp_origin: string;
  challenge_ttl_secs: number;
}

/** Authentication settings. */
export interface AuthSettings {
  provider: string;
  oidc: OidcSettings | null;
  webauthn: WebAuthnSettings | null;
}

/** Argon2 key-derivation settings for SQLCipher. */
export interface Argon2Settings {
  memory_mb: number;
  iterations: number;
  parallelism: number;
}

/** PostgreSQL connection settings. */
export interface PostgresSettings {
  host: string;
  port: number;
  dbname: string;
  user: string;
  /** Redacted by the API — always "[REDACTED]" when present. */
  password: string;
  pool_size: number;
  ssl_mode: "Disable" | "Prefer" | "Require";
}

/** Storage settings. */
export interface StorageSettings {
  backend: string;
  encryption: boolean;
  argon2: Argon2Settings;
  postgres: PostgresSettings | null;
}

/** Plugin settings. */
export interface PluginSettings {
  paths: string[];
  auto_enable: boolean;
}

/** TLS settings. */
export interface TlsSettings {
  enabled: boolean;
  cert_path: string;
  key_path: string;
}

/** CORS settings. */
export interface CorsSettings {
  allowed_origins: string[];
}

/** Rate limiting settings. */
export interface RateLimitSettings {
  requests_per_minute: number;
}

/** Network, TLS, CORS, and rate limiting settings. */
export interface NetworkSettings {
  tls: TlsSettings;
  cors: CorsSettings;
  rate_limit: RateLimitSettings;
}

/** Top-level configuration for Life Engine Core. */
export interface CoreConfig {
  core: CoreSettings;
  auth: AuthSettings;
  storage: StorageSettings;
  plugins: PluginSettings;
  network: NetworkSettings;
}

/** System information returned by GET /api/system/info. */
export interface SystemInfo {
  version: string;
  plugins_loaded: number;
  storage: string;
  uptime_seconds: number;
}

/** Plugin status information returned by GET /api/system/plugins. */
export interface PluginInfo {
  id: string;
  name: string;
  version: string;
  status: "registered" | "loaded" | "failed" | "unloaded";
}

/** Health check response from GET /api/system/health. */
export interface HealthStatus {
  status: string;
}
