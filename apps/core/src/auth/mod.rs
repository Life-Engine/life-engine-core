//! Authentication module for the Life Engine Core.
//!
//! Transport-agnostic providers (AuthProvider trait, LocalToken, OIDC,
//! WebAuthn) are now in `life_engine_auth::legacy`. This module
//! re-exports them and keeps HTTP-specific middleware and routes local.

pub mod middleware;
pub mod routes;

// Re-export transport-agnostic auth from the package.
pub use life_engine_auth::legacy::jwt;
pub use life_engine_auth::legacy::local_token;
pub use life_engine_auth::legacy::oidc;
pub use life_engine_auth::legacy::types;
pub use life_engine_auth::legacy::webauthn_provider;
pub use life_engine_auth::legacy::webauthn_store;
pub use life_engine_auth::legacy::{
    build_auth_provider, AuthProvider, MultiAuthProvider,
};
pub use types::{AuthError, AuthIdentity, TokenInfo, TokenRequest, TokenResponse};
