use thiserror::Error;

#[derive(Debug, Error)]
pub enum BootstrapError {
    #[error("unable to initialize tracing subscriber: {0}")]
    TracingSubscriberInitError(#[from] Box<dyn std::error::Error>),
}
