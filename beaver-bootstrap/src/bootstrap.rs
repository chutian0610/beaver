use std::{cell::RefCell, sync::RwLock};

use crate::{
    config::Config,
    error::BootstrapError,
    log::{Logger, LoggingConfig},
};
use di::{Ref, ServiceCollection, singleton_as_self};
use tracing_rolling_file::RollingFileAppenderBase;
use tracing_subscriber::{
    fmt::writer::MakeWriterExt, layer::SubscriberExt, util::SubscriberInitExt,
};
use typed_builder::TypedBuilder;

/// Bootstrap is the entry point of the application.
///
/// It is responsible for initializing the application, including loading the configuration,
/// initializing the logging, and initializing the service collection.
///
/// # Example
/// ```
/// use beaver_bootstrap::Bootstrap;
/// let bootstrap = Bootstrap::builder().build();
/// bootstrap.initialize().unwrap();
/// ```
///
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
    #[builder(default = RwLock::new(ServiceCollection::new()), setter(skip))]
    service_collection: RwLock<ServiceCollection>,

    /// a collection of modules
    #[builder(default = vec![])]
    modules: Vec<Box<dyn Module>>,

    /// a collection of modules
    #[builder(default = RefCell::new(BootstrapBaseModule::default()))]
    base_modules: RefCell<BootstrapBaseModule>,
}

impl Bootstrap {
    pub fn initialize(&self) -> Result<(), BootstrapError> {
        // first we try to initialize config
        self.initialize_config()?;
        // then we try to initialize logging by logger config
        self.initialize_logging()?;
        if self.show_config {
            // after logging initialized, we show config if needed
            self.show_config()?;
        }
        Ok(())
    }

    pub fn initialize_config(&self) -> Result<(), BootstrapError> {
        let env_config_prefix: Option<&str> = self.env_config_prefix.as_deref();
        let env_config_split: &str = self.env_config_split.as_str();
        let config = Config::load(env_config_prefix, env_config_split)
            .map_err(|e| BootstrapError::ConfigLoadError(e))?;
        let _ = self
            .base_modules
            .borrow_mut()
            .config
            .insert(Ref::new(config));
        Ok(())
    }

    pub fn initialize_logging(&self) -> Result<(), BootstrapError> {
        if self.initialize_logging {
            let config = match self.base_modules.borrow().config.clone() {
                Some(config) => config,
                None => return Ok(()), // if no config, just return
            };

            let logging_config = LoggingConfig::new(&config);

            let Some(level) = logging_config.log_level().as_tracing_level() else {
                return Err(BootstrapError::InvalidConfigValueError(format!(
                    "logging.log_level={:?}",
                    logging_config.log_level()
                )));
            };

            {
                // limit the scope of borrow_mut
                let mut base_modules = self.base_modules.borrow_mut();
                let _ = base_modules
                    .logging_config
                    .insert(Ref::new(logging_config.clone()));
            }

            // ensure log directory exists
            logging_config
                .ensure_log_directory()
                .map_err(|e| BootstrapError::LogDirectoryCreationError(Box::new(e)))?;

            // file layer
            let builder = RollingFileAppenderBase::builder();
            let file_appender = builder
                .filename(logging_config.full_log_file_path())
                .max_filecount(logging_config.log_file_max_count())
                .condition_max_file_size(logging_config.log_file_max_size())
                .condition_daily()
                .build()
                .map_err(|e| BootstrapError::LogFileCreationError(Box::new(e)))?;

            let (non_blocking_file_writer, file_writer_guard) =
                tracing_appender::non_blocking(file_appender);
            let file_layer = tracing_subscriber::fmt::layer()
                .with_ansi(false)
                .with_writer(non_blocking_file_writer.with_max_level(level));

            // optional stdout layer
            let (stdout_layer, stdout_guard) = if logging_config.enable_console() {
                let (non_blocking_stdout_writer, stdout_writer_guard) =
                    tracing_appender::non_blocking(std::io::stdout());
                let layer = tracing_subscriber::fmt::layer()
                    .with_writer(non_blocking_stdout_writer.with_max_level(level));
                (Some(layer), Some(stdout_writer_guard))
            } else {
                (None, None)
            };

            // save logger to keep guards active
            {
                // limit the scope of borrow_mut
                let mut base_modules = self.base_modules.borrow_mut();
                let logger = Logger::new(file_writer_guard, stdout_guard);
                let _ = base_modules.logger.insert(Ref::new(logger));
            }

            // initialize subscriber
            tracing_subscriber::registry()
                .with(file_layer)
                .with(stdout_layer)
                .try_init()
                .map_err(|e| BootstrapError::TracingSubscriberInitError(Box::new(e)))?;
        }
        Ok(())
    }
    pub fn show_config(&self) -> Result<(), BootstrapError> {
        if let Some(config) = &self.base_modules.borrow().config {
            let properties = config
                .to_properties()
                .map_err(|e| BootstrapError::ConfigShowError(e))?;
            for (key, value) in properties.get_properties() {
                tracing::info!("load config {}={}", key, value);
            }
        }
        Ok(())
    }
}

/// a module used for di configuration.
///
/// # Description
///
/// A module is a collection of services that can be registered with the service collection.
///
/// # Example
/// ```
/// use di::ServiceCollection;
/// use beaver_bootstrap::bootstrap::Module;
/// use std::sync::RwLock;
/// use di::*;
///
/// #[injectable]
/// pub struct A;
/// pub struct MyModule;
///
/// impl Module for MyModule {
///     fn configure(&self, binder: &RwLock<ServiceCollection>) {
///         let mut service_collection = binder.write().unwrap();
///         service_collection.add(A::singleton());
///     }
/// }
/// ```
pub trait Module {
    /// Configures the module by adding services to the service collection.
    ///
    /// # Arguments
    ///
    /// * `binder` - The service collection to configure.
    ///
    /// # Note
    /// binder is RwLock<ServiceCollection>, so it is thread safe.
    fn configure(&self, binder: &RwLock<ServiceCollection>);
}
struct BootstrapBaseModule {
    config: Option<Ref<Config>>,
    logger: Option<Ref<Logger>>,
    logging_config: Option<Ref<LoggingConfig>>,
}

impl Default for BootstrapBaseModule {
    fn default() -> Self {
        Self {
            config: None,
            logger: None,
            logging_config: None,
        }
    }
}

impl Module for BootstrapBaseModule {
    fn configure(&self, binder: &RwLock<ServiceCollection>) {
        // register base services
        self.register_service::<Config>(&self.config, binder);
        self.register_service::<LoggingConfig>(&self.logging_config, binder);
        self.register_service::<Logger>(&self.logger, binder);
    }
}

impl BootstrapBaseModule {
    /// register a service to the service collection.
    ///
    /// # Arguments
    ///
    /// * `service` - The service to register.
    /// * `binder` - The service collection to configure.
    fn register_service<T: Send + Sync + 'static>(
        &self,
        service: &Option<Ref<T>>,
        binder: &RwLock<ServiceCollection>,
    ) {
        if let Some(svc) = service.clone() {
            if let Ok(mut service_collection) = binder.write() {
                service_collection.add(singleton_as_self::<T>().from(move |_| svc.clone()));
            }
        }
    }
}
