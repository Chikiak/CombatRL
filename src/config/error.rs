use thiserror::Error;

#[derive(Debug, PartialEq, Error)]
pub enum ConfigError {
    #[error("Configuration file not found")]
    FileNotFound,
    #[error("Invalid configuration schema or ID exceeds 32 bytes")]
    InvalidSchema,
    #[error("Invalid bounds for parameter '{parameter}': value {value}")]
    InvalidBounds { parameter: &'static str, value: f32 },
    #[error("Subnormal, NaN or infinite float detected in parameter '{parameter}'")]
    SubnormalFloatDetected { parameter: &'static str },
    #[error("IO error while reading configuration: {0:?}")]
    IoError(std::io::ErrorKind),
}

impl From<std::io::Error> for ConfigError {
    fn from(err: std::io::Error) -> Self {
        match err.kind() {
            std::io::ErrorKind::NotFound => ConfigError::FileNotFound,
            other => ConfigError::IoError(other),
        }
    }
}

impl From<serde_json::Error> for ConfigError {
    fn from(_: serde_json::Error) -> Self {
        ConfigError::InvalidSchema
    }
}
