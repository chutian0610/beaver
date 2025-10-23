use config::ConfigError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum BootstrapError {
    #[error("unable to initialize tracing subscriber: {0}")]
    TracingSubscriberInitError(Box<dyn std::error::Error>),
    #[error("unable to load config: {0}")]
    ConfigLoadError(ConfigError),
    #[error("unable to show config: {0}")]
    ConfigShowError(ConfigError),
    #[error("invalid config value: {0}")]
    InvalidConfigValueError(String),
    #[error("missing config value: {0}")]
    MissingConfigValueError(String),
    #[error("unable to load logging config: {0}")]
    LoggingConfigLoadError(ConfigError),
    #[error("unable to create log directory: {0}")]
    LogDirectoryCreationError(Box<dyn std::error::Error>),
    #[error("unable to create log file: {0}")]
    LogFileCreationError(Box<&'static str>),
    #[error("duplicate logger: {0}")]
    DuplicateLoggerError(String),
    #[error("duplicate log file path: {0}")]
    DuplicateLogFilePathError(String),
}
