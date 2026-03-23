//! Capability types for plugin access control.
//!
//! Defines the `Capability` enum representing host function access grants,
//! and `CapabilityViolation` for when a plugin exceeds its permissions.

use std::fmt;
use std::str::FromStr;

use crate::error::{EngineError, Severity};

/// A host function access grant that plugins must declare and have approved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Capability {
    /// Read from the storage layer.
    StorageRead,
    /// Write to the storage layer.
    StorageWrite,
    /// Make outbound HTTP requests.
    HttpOutbound,
    /// Emit events to the event bus.
    EventsEmit,
    /// Subscribe to events from the event bus.
    EventsSubscribe,
    /// Read configuration values.
    ConfigRead,
}

impl fmt::Display for Capability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Capability::StorageRead => write!(f, "storage:read"),
            Capability::StorageWrite => write!(f, "storage:write"),
            Capability::HttpOutbound => write!(f, "http:outbound"),
            Capability::EventsEmit => write!(f, "events:emit"),
            Capability::EventsSubscribe => write!(f, "events:subscribe"),
            Capability::ConfigRead => write!(f, "config:read"),
        }
    }
}

/// Error returned when parsing a capability string fails.
#[derive(Debug, Clone)]
pub struct ParseCapabilityError {
    value: String,
}

impl fmt::Display for ParseCapabilityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unknown capability: {}", self.value)
    }
}

impl std::error::Error for ParseCapabilityError {}

impl FromStr for Capability {
    type Err = ParseCapabilityError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "storage:read" => Ok(Capability::StorageRead),
            "storage:write" => Ok(Capability::StorageWrite),
            "http:outbound" => Ok(Capability::HttpOutbound),
            "events:emit" => Ok(Capability::EventsEmit),
            "events:subscribe" => Ok(Capability::EventsSubscribe),
            "config:read" => Ok(Capability::ConfigRead),
            _ => Err(ParseCapabilityError {
                value: s.to_string(),
            }),
        }
    }
}

/// Error produced when a plugin attempts to use a capability it was not granted.
#[derive(Debug)]
pub struct CapabilityViolation {
    /// The capability the plugin attempted to use.
    pub capability: Capability,
    /// The plugin that attempted the violation.
    pub plugin_id: String,
    /// Description of what the plugin was trying to do.
    pub context: String,
    /// Whether this is a load-time or runtime violation.
    pub at_load_time: bool,
}

impl fmt::Display for CapabilityViolation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "plugin '{}' lacks capability '{}': {}",
            self.plugin_id, self.capability, self.context
        )
    }
}

impl std::error::Error for CapabilityViolation {}

impl EngineError for CapabilityViolation {
    fn code(&self) -> &str {
        if self.at_load_time {
            "CAP_001"
        } else {
            "CAP_002"
        }
    }

    fn severity(&self) -> Severity {
        Severity::Fatal
    }

    fn source_module(&self) -> &str {
        "capability-enforcement"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_fromstr_round_trip() {
        let capabilities = [
            Capability::StorageRead,
            Capability::StorageWrite,
            Capability::HttpOutbound,
            Capability::EventsEmit,
            Capability::EventsSubscribe,
            Capability::ConfigRead,
        ];

        for cap in &capabilities {
            let s = cap.to_string();
            let parsed: Capability = s.parse().unwrap();
            assert_eq!(*cap, parsed, "round-trip failed for {s}");
        }
    }

    #[test]
    fn display_formats() {
        assert_eq!(Capability::StorageRead.to_string(), "storage:read");
        assert_eq!(Capability::StorageWrite.to_string(), "storage:write");
        assert_eq!(Capability::HttpOutbound.to_string(), "http:outbound");
        assert_eq!(Capability::EventsEmit.to_string(), "events:emit");
        assert_eq!(Capability::EventsSubscribe.to_string(), "events:subscribe");
        assert_eq!(Capability::ConfigRead.to_string(), "config:read");
    }

    #[test]
    fn fromstr_rejects_unknown() {
        assert!("storage:delete".parse::<Capability>().is_err());
        assert!("unknown".parse::<Capability>().is_err());
        assert!("".parse::<Capability>().is_err());
    }

    #[test]
    fn violation_load_time_code() {
        let v = CapabilityViolation {
            capability: Capability::StorageRead,
            plugin_id: "test-plugin".to_string(),
            context: "declared but not approved".to_string(),
            at_load_time: true,
        };
        assert_eq!(v.code(), "CAP_001");
        assert_eq!(v.severity(), Severity::Fatal);
        assert_eq!(v.source_module(), "capability-enforcement");
    }

    #[test]
    fn violation_runtime_code() {
        let v = CapabilityViolation {
            capability: Capability::HttpOutbound,
            plugin_id: "test-plugin".to_string(),
            context: "attempted outbound HTTP call".to_string(),
            at_load_time: false,
        };
        assert_eq!(v.code(), "CAP_002");
        assert_eq!(v.severity(), Severity::Fatal);
        assert_eq!(v.source_module(), "capability-enforcement");
    }

    #[test]
    fn violation_display() {
        let v = CapabilityViolation {
            capability: Capability::StorageWrite,
            plugin_id: "my-plugin".to_string(),
            context: "tried to write data".to_string(),
            at_load_time: false,
        };
        let msg = v.to_string();
        assert!(msg.contains("my-plugin"));
        assert!(msg.contains("storage:write"));
        assert!(msg.contains("tried to write data"));
    }
}
