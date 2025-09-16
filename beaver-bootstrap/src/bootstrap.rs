use crate::error::BootstrapError;
use tracing::{debug, info};
use typed_builder::TypedBuilder;
#[derive(TypedBuilder)]
pub struct Bootstrap {
    /// Whether need to initialize logging.
    #[builder(default = true)]
    initialize_logging: bool,
}

impl Bootstrap {
    pub fn initialize(&self) -> Result<(), BootstrapError> {
        self.initialize_config()?;
        self.initialize_logging()?;
        Ok(())
    }

    pub fn initialize_config(&self) -> Result<(), BootstrapError> {
        Ok(())
    }

    pub fn initialize_logging(&self) -> Result<(), BootstrapError> {
        if self.initialize_logging {
            tracing_subscriber::fmt().init();
        }
        Ok(())
    }
}
