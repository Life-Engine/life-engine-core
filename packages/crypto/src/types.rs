//! Types for configurable cryptographic parameters.

use serde::{Deserialize, Serialize};

/// Configurable Argon2id key-derivation parameters.
///
/// Defaults: 64 MB memory, 3 iterations, 4 parallel lanes.
/// Lower values can be used on resource-constrained devices.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Argon2Params {
    /// Memory cost in kilobytes (default 65536 = 64 MB).
    #[serde(default = "default_memory_kib")]
    pub memory_kib: u32,
    /// Number of iterations (default 3).
    #[serde(default = "default_iterations")]
    pub iterations: u32,
    /// Degree of parallelism (default 4).
    #[serde(default = "default_parallelism")]
    pub parallelism: u32,
}

fn default_memory_kib() -> u32 {
    65536
}

fn default_iterations() -> u32 {
    3
}

fn default_parallelism() -> u32 {
    4
}

impl Default for Argon2Params {
    fn default() -> Self {
        Self {
            memory_kib: default_memory_kib(),
            iterations: default_iterations(),
            parallelism: default_parallelism(),
        }
    }
}
