use std::cell::RefCell;

use crate::{
    config::Config,
    error::BootstrapError,
    log::{Logger, LoggingConfig},
};
use di::{Ref, ServiceCollection, singleton_as_self};
use tracing_appender::non_blocking::WorkerGuard;
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
    #[builder(default = ServiceCollection::new(), setter(skip))]
    service_collection: ServiceCollection,

    /// a collection of modules
    #[builder(default = vec![])]
    modules: Vec<Box<dyn Module>>,

    /// a collection of modules
    #[builder(default = RefCell::new(BaseModule::default()))]
    base_modules: RefCell<BaseModule>,
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
        if self.initialize_logging && self.base_modules.borrow().config.is_some() {
            let logging_config =
                LoggingConfig::new(&self.base_modules.borrow().config.as_ref().unwrap());
            let Some(level) = logging_config.log_level().as_tracing_level() else {
                return Err(BootstrapError::InvalidConfigValueError(format!(
                    "logging.log_level={:?}",
                    logging_config.log_level()
                )));
            };
            // file layer
            let builder = RollingFileAppenderBase::builder();
            let file_appender = builder
                .filename(logging_config.full_log_file_path())
                .max_filecount(logging_config.log_file_max_count())
                .condition_max_file_size(logging_config.log_file_max_size())
                .condition_daily()
                .build()
                .unwrap();
            let (non_blocking_file_writer, file_writer_guard) =
                tracing_appender::non_blocking(file_appender);
            let file_guard: WorkerGuard = file_writer_guard;
            let file_layer = tracing_subscriber::fmt::layer()
                .with_ansi(false)
                .with_writer(non_blocking_file_writer.with_max_level(level));

            let mut stdout_guard: Option<WorkerGuard> = None;
            // optional stdout layer
            let stdout_layer = if logging_config.enable_console() {
                let (non_blocking_stdout_writer, stdout_writer_guard) =
                    tracing_appender::non_blocking(std::io::stdout());
                stdout_guard = Some(stdout_writer_guard);
                Some(
                    tracing_subscriber::fmt::layer()
                        .with_writer(non_blocking_stdout_writer.with_max_level(level)),
                )
            } else {
                stdout_guard = None;
                None
            };
            // hold log layer guard
            let logger = Logger::new(file_guard, stdout_guard);
            let _ = self
                .base_modules
                .borrow_mut()
                .logger
                .insert(Ref::new(logger));

            return tracing_subscriber::registry()
                .with(file_layer)
                .with(stdout_layer)
                .try_init()
                .map_err(|e| BootstrapError::TracingSubscriberInitError(Box::new(e)));
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
    logger: Option<Ref<Logger>>,
}

impl Default for BaseModule {
    fn default() -> Self {
        Self {
            config: None,
            logger: None,
        }
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
