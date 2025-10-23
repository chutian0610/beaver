use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    sync::RwLock,
};

use crate::{
    config::Config,
    error::BootstrapError,
    log::{
        AllLogger, AppenderGuard, ConsoleAppenderConfig, FileAppenderConfig, Logger, LoggingConfig,
    },
};
use di::{Ref, ServiceCollection, singleton_as_self};
use tracing::{Level, level_filters::LevelFilter};
use tracing_appender::non_blocking::{NonBlocking, WorkerGuard};
use tracing_rolling_file::RollingFileAppenderBase;
use tracing_subscriber::{
    EnvFilter, Layer,
    filter::{Targets, targets},
    fmt::writer::MakeWriterExt,
    layer::SubscriberExt,
    registry,
    util::SubscriberInitExt,
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

    fn initialize_logging_config(&self) -> Result<(), BootstrapError> {
        let config: Option<std::sync::Arc<Config>> = self.base_modules.borrow().config.clone();

        let logging_config_result = match config {
            Some(config) => LoggingConfig::new(&config),
            None => Err(BootstrapError::MissingConfigValueError(
                "logging.logger_config is empty".to_string(),
            )),
        };
        let logging_config = Ref::new(logging_config_result?);
        {
            // limit the scope of borrow_mut
            let mut base_modules = self.base_modules.borrow_mut();
            let _ = base_modules.logging_config.insert(logging_config);
        }
        return Ok(());
    }
    fn initialize_logging_loggers(&self) -> Result<(), BootstrapError> {
        let logging_config: Option<std::sync::Arc<LoggingConfig>> =
            self.base_modules.borrow().logging_config.clone();
        if logging_config.is_none() {
            return Err(BootstrapError::MissingConfigValueError(
                "logging.logger_config is empty".to_string(),
            ));
        }
        let binding: std::sync::Arc<LoggingConfig> = logging_config.unwrap();
        let mut non_blocking_writers = Vec::new();
        let mut writer_guards = Vec::new();

        let all_logger = binding.logger_config().loggers();
        let mut logger_map: HashMap<&str, &Logger> = HashMap::new();
        all_logger.iter().cloned().for_each(|x| {
            logger_map.insert(x.name(), x);
        });
        for file_config in binding.file_appender_config() {
            if file_config.enable() {
                let (non_blocking_file_writer, targets, level, file_writer_guard) =
                    self.initialize_logging_file_tracing(file_config, &logger_map)?;
                non_blocking_writers.push((non_blocking_file_writer, targets, level));
                writer_guards.push(file_writer_guard);
            }
        }
        let mut console_writer = None;
        let console_opt = binding.console_appender_config();
        if console_opt.is_some() && console_opt.unwrap().enable() {
            let (non_blocking_console_writer, targets, level, console_writer_guard) =
                self.initialize_logging_console_tracing(console_opt.unwrap(), &logger_map)?;
            let _ = console_writer.insert((non_blocking_console_writer, targets, level));
            writer_guards.push(console_writer_guard);
        }
        let mut layers = Vec::new();
        for (non_blocking_file_writer, target, level) in non_blocking_writers {
            let file_layer = tracing_subscriber::fmt::layer()
                .with_ansi(false)
                .with_writer(non_blocking_file_writer.with_max_level(level))
                .with_filter(target);
            layers.push(file_layer);
        }
        let _console_layer = console_writer.is_some_and(|(x, y, z)| {
            let layer = tracing_subscriber::fmt::layer()
                .with_writer(x.with_max_level(z))
                .with_filter(y);
            layers.push(layer);
            return true;
        });
        // save logger to keep guards active
        {
            // limit the scope of borrow_mut
            let mut base_modules = self.base_modules.borrow_mut();
            let logger = AppenderGuard::new(writer_guards);
            let _ = base_modules.logger.insert(Ref::new(logger));
        }
        let subscriber = tracing_subscriber::registry().with(layers);
        subscriber
            .try_init()
            .map_err(|e| BootstrapError::TracingSubscriberInitError(Box::new(e)))?;

        Ok(())
    }
    fn initialize_logging_console_tracing(
        &self,
        appender_config: &ConsoleAppenderConfig,
        logger_map: &HashMap<&str, &Logger>,
    ) -> Result<(NonBlocking, Targets, Level, WorkerGuard), BootstrapError> {
        // get write level from appender config
        let Some(level) = appender_config.write_level().as_tracing_level() else {
            return Err(BootstrapError::InvalidConfigValueError(format!(
                "logging.console_appender[?].write_level={:?}",
                appender_config.write_level()
            )));
        };
        let targets: Vec<String> = appender_config
            .logger_names()
            .iter()
            .cloned()
            .collect::<HashSet<&str>>()
            .into_iter()
            .map(String::from)
            .collect::<Vec<String>>();
        let mut logger_target: Vec<&Logger> = Vec::new();
        for target in targets {
            // unwrap is safe, validate during logging config init
            let value = logger_map.get(target.as_str()).unwrap();
            logger_target.push(value);
        }
        let (non_blocking_file_writer, console_writer_guard) =
            tracing_appender::non_blocking(std::io::stdout());
        let target_builder: Targets = Targets::new();
        let targets = logger_target.into_iter().fold(target_builder, |acc, item| {
            if item.target().is_empty() {
                acc.with_default(item.level().as_tracing_level_filter())
            } else {
                acc.with_target(item.target(), item.level().as_tracing_level_filter())
            }
        });
        Ok((
            non_blocking_file_writer,
            targets,
            level,
            console_writer_guard,
        ))
    }
    fn initialize_logging_file_tracing(
        &self,
        appender_config: &FileAppenderConfig,
        logger_map: &HashMap<&str, &Logger>,
    ) -> Result<(NonBlocking, Targets, Level, WorkerGuard), BootstrapError> {
        // get write level from appender config
        let Some(level) = appender_config.write_level().as_tracing_level() else {
            return Err(BootstrapError::InvalidConfigValueError(format!(
                "logging.file_appenders[?].write_level={:?}",
                appender_config.write_level()
            )));
        };
        // build file layer
        let builder = RollingFileAppenderBase::builder();
        let file_appender = builder
            .filename(appender_config.file_path().to_str().unwrap().to_string())
            .max_filecount(appender_config.file_max_count())
            .condition_max_file_size(appender_config.file_max_size())
            .condition_daily()
            .build()
            .map_err(|e| BootstrapError::LogFileCreationError(Box::new(e)))?;
        let targets: Vec<String> = appender_config
            .logger_names()
            .iter()
            .cloned()
            .collect::<HashSet<&str>>()
            .into_iter()
            .map(String::from)
            .collect::<Vec<String>>();
        let mut logger_target: Vec<&Logger> = Vec::new();
        for target in targets {
            // unwrap is safe, validate during logging config init
            let value = logger_map.get(target.as_str()).unwrap();
            logger_target.push(value);
        }
        let (non_blocking_file_writer, file_writer_guard) =
            tracing_appender::non_blocking(file_appender);
        let target_builder: Targets = Targets::new();
        let targets = logger_target.into_iter().fold(target_builder, |acc, item| {
            if item.target().is_empty() {
                acc.with_default(item.level().as_tracing_level_filter())
            } else {
                acc.with_target(item.target(), item.level().as_tracing_level_filter())
            }
        });
        Ok((non_blocking_file_writer, targets, level, file_writer_guard))
    }
    pub fn initialize_logging(&self) -> Result<(), BootstrapError> {
        if self.initialize_logging {
            self.initialize_logging_config()?;
            self.initialize_logging_loggers()?;
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
    logger: Option<Ref<AppenderGuard>>,
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
        self.register_service::<AppenderGuard>(&self.logger, binder);
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
