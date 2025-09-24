use crate::{error::BootstrapError, module::Module};
use di::ServiceCollection;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use typed_builder::TypedBuilder;
#[derive(TypedBuilder)]
pub struct Bootstrap {
    /// Whether need to initialize logging.
    #[builder(default = true)]
    initialize_logging: bool,

    /// a collection of registered services.
    ///
    /// This field is initialized internally.
    #[builder(default = ServiceCollection::new(), setter(skip))]
    service_collection: ServiceCollection,

    /// a collection of modules
    #[builder(default = vec![])]
    modules: Vec<Box<dyn Module>>,
}

impl Bootstrap {
    pub fn initialize(&self) -> Result<(), BootstrapError> {
        // first we try to initialize config
        self.initialize_config()?;
        // then we try to initialize logging by logger config
        self.initialize_logging()?;
        Ok(())
    }

    pub fn initialize_config(&self) -> Result<(), BootstrapError> {
        Ok(())
    }

    pub fn initialize_logging(&self) -> Result<(), BootstrapError> {
        if self.initialize_logging {
            // init the default logging subscriber.
            let subscriber =
                tracing_subscriber::Registry::default().with(tracing_subscriber::fmt::layer());
            if let Err(e) = subscriber.try_init() {
                tracing::error!("unable to initialize tracing subscriber: {:?}", e);
                return Err(BootstrapError::TracingSubscriberInitError(Box::new(e)));
            } else {
                tracing::debug!("tracing subscriber initialized");
            }
        }
        Ok(())
    }
}
