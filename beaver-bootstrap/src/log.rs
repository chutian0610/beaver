use std::{env, fmt, path::PathBuf, str::FromStr, sync::LazyLock};

use di::inject;
use serde::{Deserialize, Deserializer, Serialize};
use tracing_appender::non_blocking::WorkerGuard;

use crate::config::{Config, ConfigPrefix};

static DEFAULT_LOG_FOLDER: LazyLock<PathBuf> = LazyLock::new(|| {
    let dir = match env::var("CARGO_MANIFEST_DIR") {
        Ok(dir) => PathBuf::from(dir).join("logs"),
        Err(_) => {
            // get config path from current executable file path
            let mut current_exe =
                env::current_exe().expect("failed to get current executable file path");
            current_exe.pop();
            current_exe.push("logs");
            current_exe
        }
    };
    dir
});

pub struct Logger {
    _file_guard: WorkerGuard,
    _stdout_guard: Option<WorkerGuard>,
}
impl Logger {
    pub fn new(file_guard: WorkerGuard, stdout_guard: Option<WorkerGuard>) -> Self {
        Self {
            _file_guard: file_guard,
            _stdout_guard: stdout_guard,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LoggingConfig {
    log_level: Level,
    enable_console: bool,
    log_file_path: Option<String>,
    log_file_max_size: u64,
    log_file_max_count: usize,
    log_file_name: Option<String>,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            log_level: Level::default(),
            enable_console: true,
            log_file_path: None,
            log_file_max_size: 0,
            log_file_max_count: 0,
            log_file_name: None,
        }
    }
}

impl LoggingConfig {
    pub fn new(config: &Config) -> Self {
        config.get().expect("failed to load LoggingConfig")
    }

    pub fn log_level(&self) -> Level {
        self.log_level
    }

    pub fn enable_console(&self) -> bool {
        self.enable_console
    }

    pub fn log_file_path(&self) -> &str {
        self.log_file_path
            .as_deref()
            .unwrap_or_else(|| DEFAULT_LOG_FOLDER.as_path().to_str().unwrap())
    }

    pub fn full_log_file_path(&self) -> String {
        let log_file_path = self.log_file_path();
        if log_file_path.ends_with('/') {
            format!("{}{}", log_file_path, self.log_file_name())
        } else {
            format!("{}/{}", log_file_path, self.log_file_name())
        }
    }

    pub fn log_file_max_size(&self) -> u64 {
        self.log_file_max_size
    }

    pub fn log_file_max_count(&self) -> usize {
        self.log_file_max_count
    }
    pub fn log_file_name(&self) -> &str {
        self.log_file_name.as_deref().unwrap_or("beaver.log")
    }
}
impl ConfigPrefix for LoggingConfig {
    const PREFIX: &'static str = "logging";
}

#[derive(Debug, Default, Copy, Clone, Serialize, PartialEq, Eq)]
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
