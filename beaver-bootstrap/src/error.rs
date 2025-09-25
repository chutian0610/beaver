use config::ConfigError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum BootstrapError {
    #[error("unable to initialize tracing subscriber: {0}")]
    TracingSubscriberInitError(#[from] Box<dyn std::error::Error>),
    #[error("unable to load config: {0}")]
    ConfigLoadError(ConfigError),
    #[error("unable to print config: {0}")]
    ConfigPrintError(ConfigError),
}
