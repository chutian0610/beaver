use std::{
    collections::HashSet,
    env,
    fmt::{self},
    path::{Path, PathBuf},
    str::FromStr,
    sync::LazyLock,
};

use serde::{Deserialize, Deserializer, Serialize};
use tracing_appender::non_blocking::WorkerGuard;

use crate::{
    config::{Config, ConfigPrefix},
    error::BootstrapError,
    serde::non_empty,
};

static DEFAULT_LOG_FOLDER: LazyLock<PathBuf> = LazyLock::new(|| {
    let dir = match env::var("CARGO_MANIFEST_DIR") {
        Ok(dir) => PathBuf::from(dir).join("logs"),
        Err(_) => {
            // get config path from current executable file path
            if let Ok(mut current_exe) = env::current_exe() {
                current_exe.pop();
                current_exe.push("logs");
                current_exe
            } else {
                // fallback to current directory
                PathBuf::from("./logs")
            }
        }
    };
    dir
});

#[derive(Debug)]
pub struct AppenderGuard {
    _guards: Vec<WorkerGuard>,
}
impl AppenderGuard {
    pub fn new(guards: Vec<WorkerGuard>) -> Self {
        let mut _guards = Vec::new();
        _guards.extend(guards);
        Self { _guards }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, Default, Eq, Hash, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Logger {
    #[serde(deserialize_with = "non_empty")]
    target: String,
    level: Level,
    #[serde(deserialize_with = "non_empty")]
    name: String,
}

impl Logger {
    pub fn new(name: &str, level: &Level, target: &str) -> Self {
        Self {
            target: target.to_owned(),
            level: level.to_owned(),
            name: name.to_owned(),
        }
    }

    pub fn name(&self) -> &str {
        &self.name.as_str()
    }
    pub fn target(&self) -> &str {
        &self.target.as_str()
    }

    pub fn level(&self) -> &Level {
        &self.level
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
pub struct AllLoggerSerde {
    loggers: Vec<Logger>,
    default_level: Level,
    #[serde(deserialize_with = "non_empty")]
    default_name: String,
}
impl From<AllLoggerSerde> for AllLogger {
    fn from(value: AllLoggerSerde) -> AllLogger {
        let mut all_logger: Vec<Logger> = Vec::new();
        value
            .loggers
            .iter()
            .for_each(|x| all_logger.push(x.to_owned()));
        all_logger.push(Logger {
            target: "".to_string(),
            level: value.default_level,
            name: value.default_name,
        });
        AllLogger {
            loggers: all_logger,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default, deny_unknown_fields, from = "AllLoggerSerde")]
pub struct AllLogger {
    loggers: Vec<Logger>,
}

impl AllLogger {
    pub fn loggers(&self) -> Vec<&Logger> {
        self.loggers.iter().collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FileAppenderConfigSerde {
    enable: bool,
    write_level: Option<Level>,
    file_dir: Option<String>,
    file_max_size: u64,
    file_max_count: usize,
    file_name: String,
    logger_names: Vec<String>,
}
impl From<FileAppenderConfigSerde> for FileAppenderConfig {
    fn from(value: FileAppenderConfigSerde) -> FileAppenderConfig {
        // get log file directory, if not set, use default log folder
        let log_file_dir = match value.file_dir {
            Some(path) => path,
            None => DEFAULT_LOG_FOLDER
                .as_path()
                .to_str()
                .map(String::from)
                .unwrap_or_else(|| "./logs".to_string()),
        };
        let log_level = match value.write_level {
            Some(level) => level,
            None => Level::Info,
        };
        // get full log file path
        let full_file_path: PathBuf = PathBuf::from(&log_file_dir).join(&value.file_name);
        FileAppenderConfig {
            enable: value.enable,
            write_level: log_level,
            file_dir: log_file_dir,
            file_max_size: value.file_max_size,
            file_max_count: value.file_max_count,
            file_name: value.file_name,
            file_path: full_file_path,
            logger_names: value.logger_names,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default, deny_unknown_fields, from = "FileAppenderConfigSerde")]
pub struct FileAppenderConfig {
    enable: bool,
    write_level: Level,
    file_dir: String,
    file_path: PathBuf,
    file_max_size: u64,
    file_max_count: usize,
    file_name: String,
    logger_names: Vec<String>,
}

impl FileAppenderConfig {
    pub fn write_level(&self) -> Level {
        self.write_level
    }

    pub fn enable(&self) -> bool {
        self.enable
    }

    pub fn file_dir(&self) -> &str {
        self.file_dir.as_str()
    }

    pub fn file_path(&self) -> &Path {
        self.file_path.as_path()
    }

    pub fn file_max_size(&self) -> u64 {
        self.file_max_size
    }

    pub fn file_max_count(&self) -> usize {
        self.file_max_count
    }
    pub fn file_name(&self) -> &str {
        &self.file_name.as_str()
    }

    pub fn logger_names(&self) -> Vec<&str> {
        self.logger_names.iter().map(|x| x.as_str()).collect()
    }

    /// make sure log directory exists, if not, create it
    pub fn ensure_log_directory(&self) -> std::io::Result<()> {
        let log_path = self.file_dir();
        let log_dir = PathBuf::from(log_path);

        if !log_dir.exists() {
            std::fs::create_dir_all(&log_dir)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
pub struct ConsoleAppenderConfig {
    enable: bool,
    write_level: Level,
    logger_names: Vec<String>,
}

impl ConsoleAppenderConfig {
    pub fn write_level(&self) -> Level {
        self.write_level
    }

    pub fn enable(&self) -> bool {
        self.enable
    }

    pub fn logger_names(&self) -> Vec<&str> {
        self.logger_names.iter().map(|x| x.as_str()).collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct LoggingConfig {
    all_logger: AllLogger,
    file_appenders: Vec<FileAppenderConfig>,
    console_appender: Option<ConsoleAppenderConfig>,
}

impl LoggingConfig {
    pub fn new(config: &Config) -> Result<Self, BootstrapError> {
        let logging_config = config
            .get::<LoggingConfig>()
            .map_err(|e| BootstrapError::LoggingConfigLoadError(e))?;
        // validate logging config
        logging_config.validate()?;
        Ok(logging_config)
    }

    pub fn logger_config(&self) -> &AllLogger {
        &self.all_logger
    }

    pub fn file_appender_config(&self) -> Vec<&FileAppenderConfig> {
        self.file_appenders
            .iter()
            .collect::<Vec<&FileAppenderConfig>>()
    }

    pub fn console_appender_config(&self) -> Option<&ConsoleAppenderConfig> {
        self.console_appender.as_ref()
    }

    fn all_logger_name(&self) -> Vec<&str> {
        self.logger_config()
            .loggers
            .iter()
            .map(|x| x.name.as_str())
            .collect()
    }

    fn validate_loggers(&self) -> Result<(), BootstrapError> {
        let logger_config = self.logger_config();
        let all_loggers = &logger_config.loggers;

        let mut set: HashSet<&Logger> = HashSet::new();

        for logger in all_loggers {
            if !set.insert(logger) {
                return Err(BootstrapError::DuplicateLoggerError(format!(
                    "{:?}",
                    logger
                )));
            }
        }
        Ok(())
    }

    fn validate_file_appender(&self) -> Result<(), BootstrapError> {
        let all_logger_name = self.all_logger_name();
        let all_logger_name_set: HashSet<&str> = all_logger_name.iter().cloned().collect();
        let file_appender_config = self.file_appender_config();
        let mut path_set: HashSet<&Path> = HashSet::new();
        for config in file_appender_config {
            config
                .ensure_log_directory()
                .map_err(|e| BootstrapError::LogDirectoryCreationError(Box::new(e)))?;
            // check log file path duplication
            let log_file_path: &Path = config.file_path();
            if !path_set.insert(log_file_path) {
                return Err(BootstrapError::DuplicateLogFilePathError(
                    log_file_path.to_str().unwrap_or("").to_string(),
                ));
            }
            let loggers = config.logger_names();
            for logger in loggers {
                if !all_logger_name_set.contains(logger) {
                    return Err(BootstrapError::InvalidConfigValueError(format!(
                        "wrong logger name {} in appender{:#?}",
                        logger, config
                    )));
                }
            }
        }
        Ok(())
    }
    fn validate_console_appender(&self) -> Result<(), BootstrapError> {
        let all_logger_name = self.all_logger_name();
        let all_logger_name_set: HashSet<&str> = all_logger_name.iter().cloned().collect();
        let Some(config) = &self.console_appender else {
            return Ok(());
        };
        let loggers = config.logger_names();
        for logger in loggers {
            if !all_logger_name_set.contains(logger) {
                return Err(BootstrapError::InvalidConfigValueError(format!(
                    "wrong logger name {} in console appender",
                    logger
                )));
            }
        }
        Ok(())
    }

    pub fn validate(&self) -> Result<(), BootstrapError> {
        self.validate_loggers()?;
        self.validate_file_appender()?;
        self.validate_console_appender()?;
        Ok(())
    }
}
impl ConfigPrefix for LoggingConfig {
    const PREFIX: &'static str = "logging";
}

#[derive(Debug, Default, Copy, Clone, Serialize, PartialEq, Eq, Hash)]
pub enum Level {
    /// The "trace" level.
    Trace,
    /// The "debug" level.
    Debug,
    /// The "info" level.
    #[default]
    Info,
    /// The "warn" level.
    Warn,
    /// The "error" level.
    Error,
    /// Off level.
    Off,
}

impl<'de> Deserialize<'de> for Level {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        const VARIANTS: [&str; 6] = ["trace", "debug", "info", "warn", "error", "off"];

        let s = String::deserialize(deserializer)?;
        s.parse()
            .map_err(|_| <D::Error as serde::de::Error>::unknown_variant(&s, &VARIANTS))
    }
}

#[non_exhaustive]
#[derive(Debug)]
pub struct ParseLevelError;

impl FromStr for Level {
    type Err = ParseLevelError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            s if s.eq_ignore_ascii_case("trace") => Ok(Level::Trace),
            s if s.eq_ignore_ascii_case("debug") => Ok(Level::Debug),
            s if s.eq_ignore_ascii_case("info") => Ok(Level::Info),
            s if s.eq_ignore_ascii_case("warn") => Ok(Level::Warn),
            s if s.eq_ignore_ascii_case("error") => Ok(Level::Error),
            s if s.eq_ignore_ascii_case("off") => Ok(Level::Off),
            _ => Err(ParseLevelError),
        }
    }
}

impl Level {
    pub fn as_str(&self) -> &'static str {
        match self {
            Level::Trace => "Trace",
            Level::Debug => "Debug",
            Level::Info => "Info",
            Level::Warn => "Warn",
            Level::Error => "Error",
            Level::Off => "Off",
        }
    }

    pub fn as_tracing_level(&self) -> Option<tracing::Level> {
        match self {
            Level::Trace => Some(tracing::Level::TRACE),
            Level::Debug => Some(tracing::Level::DEBUG),
            Level::Info => Some(tracing::Level::INFO),
            Level::Warn => Some(tracing::Level::WARN),
            Level::Error => Some(tracing::Level::ERROR),
            Level::Off => None,
        }
    }

    pub fn as_tracing_level_filter(&self) -> tracing::level_filters::LevelFilter {
        match self {
            Level::Trace => tracing::level_filters::LevelFilter::TRACE,
            Level::Debug => tracing::level_filters::LevelFilter::DEBUG,
            Level::Info => tracing::level_filters::LevelFilter::INFO,
            Level::Warn => tracing::level_filters::LevelFilter::WARN,
            Level::Error => tracing::level_filters::LevelFilter::ERROR,
            Level::Off => tracing::level_filters::LevelFilter::OFF,
        }
    }
}

impl fmt::Display for Level {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad(self.as_str())
    }
}
