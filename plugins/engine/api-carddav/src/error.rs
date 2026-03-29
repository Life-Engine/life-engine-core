use life_engine_plugin_sdk::prelude::*;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CardDavError {
    #[error("unknown action: {0}")]
    UnknownAction(String),
}

impl EngineError for CardDavError {
    fn code(&self) -> &str {
        match self {
            Self::UnknownAction(_) => "CARDDAV_001",
        }
    }

    fn severity(&self) -> Severity {
        match self {
            Self::UnknownAction(_) => Severity::Fatal,
        }
    }

    fn source_module(&self) -> &str {
        "api-carddav"
    }
}
