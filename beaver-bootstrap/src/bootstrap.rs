use crate::{config::Config, error::BootstrapError};
use di::{Ref, ServiceCollection, singleton_as_self};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use typed_builder::TypedBuilder;
#[derive(TypedBuilder)]
pub struct Bootstrap {
    /// Whether need to initialize logging.
    #[builder(default = true)]
    initialize_logging: bool,
    /// Whether need to print config.
    #[builder(default = false)]
    show_config: bool,

    /// Prefix of environment variables to override config values.
    #[builder(default = Some("BEAVER_".to_string()))]
    env_config_prefix: Option<String>,
    /// Separator of environment variables to override config values.
    #[builder(default = "_".to_string())]
    env_config_split: String,

    /// a collection of registered services.
    ///
    /// This field is initialized internally.
    #[builder(default = ServiceCollection::new(), setter(skip))]
    service_collection: ServiceCollection,

    /// a collection of modules
    #[builder(default = vec![])]
    modules: Vec<Box<dyn Module>>,

    /// a collection of modules
    #[builder(default = BaseModule::default())]
    base_modules: BaseModule,
}

impl Bootstrap {
    pub fn initialize(&mut self) -> Result<(), BootstrapError> {
        // first we try to initialize config
        self.initialize_config()?;
        // then we try to initialize logging by logger config
        self.initialize_logging()?;
        if self.show_config {
            // after logging initialized, we print config if needed
            self.print_config()?;
        }
        Ok(())
    }

    pub fn initialize_config(&mut self) -> Result<(), BootstrapError> {
        let env_config_prefix: Option<&str> = self.env_config_prefix.as_deref();
        let env_config_split: &str = self.env_config_split.as_str();
        let config = Config::load(env_config_prefix, env_config_split)
            .map_err(|e| BootstrapError::ConfigLoadError(e))?;
        let _ = self.base_modules.config.insert(Ref::new(config));
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

    pub fn print_config(&self) -> Result<(), BootstrapError> {
        if let Some(config) = &self.base_modules.config {
            let properties = config
                .to_properties()
                .map_err(|e| BootstrapError::ConfigPrintError(e))?;
            for (key, value) in properties.get_properties() {
                tracing::info!("load config {}={}", key, value);
            }
        }
        Ok(())
    }
}

/// a module used for di configuration.
///
/// # Example
/// ```
/// use di::ServiceCollection;
/// use beaver_bootstrap::module::Module;
/// use di::*;
///
/// #[injectable]
/// pub struct A;
/// pub struct MyModule;
///
/// impl Module for MyModule {
///     fn configure(&self, binder: &mut ServiceCollection) {
///         binder.add(A::singleton());
///     }
/// }
/// ```
pub trait Module {
    fn configure(&self, binder: &mut ServiceCollection);
}
pub struct BaseModule {
    config: Option<Ref<Config>>,
}

impl Default for BaseModule {
    fn default() -> Self {
        Self { config: None }
    }
}

impl Module for BaseModule {
    fn configure(&self, binder: &mut ServiceCollection) {
        let config = self.config.clone();
        if let Some(config) = config {
            binder.add(singleton_as_self::<Config>().from(move |_| config.clone()));
        }
    }
}
