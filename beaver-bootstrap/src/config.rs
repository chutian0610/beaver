use std::{
    collections::HashMap,
    env,
    path::{Path, PathBuf},
    sync::LazyLock,
};

use config::{ConfigError, File, ValueKind};
use di::injectable;
use serde::Deserialize;

static DEFAULT_CONFIG_FOLDER: LazyLock<PathBuf> = LazyLock::new(|| {
    
        // get config path from current source code folder
        PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/etc"))

        match env::var("BEAVER_CONFIG") {
            Ok(dir) => PathBuf::from(dir), // get config path from env
            Err(_) => {
                // get config path from current executable file path
                let mut current_exe =
                    env::current_exe().expect("failed to get current executable file path");
                current_exe.pop();
                current_exe.push("etc");
                current_exe
            }
        }
    }
});

/// Config is the configuration of the application.
///
/// It is loaded from the `config.toml` file in the `etc` folder of the application.
///
/// # Example
/// ```
/// use beaver_bootstrap::config::Config;
/// let config = Config::load(None, "_").unwrap();
/// let port = config.get::<PortConfig>().unwrap().port;
/// ```
///
#[derive(Clone)]
#[injectable]
pub struct Config {
    inner: config::Config,
}

impl Config {
    pub fn new(inner: config::Config) -> Self {
        Self { inner }
    }

    pub fn load(
        env_config_prefix: Option<&str>,
        env_config_split: &str,
    ) -> Result<Self, ConfigError> {
        Self::from_folder(
            DEFAULT_CONFIG_FOLDER.as_path(),
            env_config_prefix,
            env_config_split,
        )
    }
    pub fn from_folder(
        path: &Path,
        env_config_prefix: Option<&str>,
        env_config_split: &str,
    ) -> Result<Self, ConfigError> {
        let cfg = path.join("config.toml");
        let mut builder = config::Config::builder();
        // add default config file
        builder = builder.add_source(File::from(cfg).required(true));

        // add environment variables to config
        if let Some(prefix) = env_config_prefix {
            builder = builder
                .add_source(config::Environment::with_prefix(prefix).separator(env_config_split));
        } else {
            builder =
                builder.add_source(config::Environment::default().separator(env_config_split));
        }
        let config = builder.build()?;

        Ok(Self { inner: config })
    }
    pub fn get<'de, T>(&self) -> Result<T, ConfigError>
    where
        T: ConfigPrefix + Deserialize<'de>,
    {
        match self.inner.get::<T>(T::PREFIX) {
            Ok(o) => Ok(o),
            Err(e) => {
                let ConfigError::NotFound(_) = &e else {
                    return Err(e);
                };
                // get a map
                let v = config::Value::new(None, ValueKind::Table(Default::default()));

                match T::deserialize(v) {
                    Ok(o) => Ok(o),
                    Err(_) => Err(e),
                }
            }
        }
    }
    pub(crate) fn to_properties(&self) -> Result<Properties, ConfigError> {
        Properties::from_config(self)
    }
}

/// ConfigPrefix is a trait that is used to identify the prefix of a configuration.
///
/// # Example
/// ```
/// use beaver_bootstrap::config::ConfigPrefix;
/// #[derive(Deserialize)]
/// struct PortConfig {
///     port: u16,
/// }
/// impl ConfigPrefix for PortConfig {
///     const PREFIX: &'static str = "port";
/// }
/// ```
pub trait ConfigPrefix {
    const PREFIX: &'static str;
}

pub(crate) struct Properties {
    properties: HashMap<String, String>,
}

pub(crate) struct PropertiesConfig {
    array_split: bool,
    separator: char,
}
impl Default for PropertiesConfig {
    fn default() -> Self {
        PropertiesConfig {
            array_split: true,
            separator: '.',
        }
    }
}

impl Properties {
    pub fn from_config(config: &Config) -> Result<Self, ConfigError> {
        Self::from_config_opt(config, &PropertiesConfig::default())
    }

    pub fn from_config_opt(
        config: &Config,
        properties_config: &PropertiesConfig,
    ) -> Result<Self, ConfigError> {
        let mut properties = HashMap::new();
        let config_map: HashMap<String, config::Value> = config.inner.clone().try_deserialize()?;
        Self::flatten("", &config_map, &mut properties, properties_config);
        Ok(Self { properties })
    }

    fn flatten(
        prefix: &str,
        map: &HashMap<String, config::Value>,
        properties: &mut HashMap<String, String>,
        properties_config: &PropertiesConfig,
    ) {
        for (key, value) in map {
            let full_key = if prefix.is_empty() {
                key.clone()
            } else {
                format!("{}{}{}", prefix, properties_config.separator, key)
            };
            Self::handle_value(&full_key, value, properties, properties_config);
        }
    }
    fn handle_value(
        prefix: &str,
        value: &config::Value,
        properties: &mut HashMap<String, String>,
        properties_config: &PropertiesConfig,
    ) {
        match &value.kind {
            ValueKind::Boolean(b) => {
                properties.insert(prefix.to_string(), b.to_string());
            }
            ValueKind::I64(i_64) => {
                properties.insert(prefix.to_string(), i_64.to_string());
            }
            ValueKind::I128(i_128) => {
                properties.insert(prefix.to_string(), i_128.to_string());
            }
            ValueKind::U64(u_64) => {
                properties.insert(prefix.to_string(), u_64.to_string());
            }
            ValueKind::U128(u_128) => {
                properties.insert(prefix.to_string(), u_128.to_string());
            }
            ValueKind::Float(f) => {
                properties.insert(prefix.to_string(), format!("{:.2}", f));
            }
            ValueKind::String(s) => {
                properties.insert(prefix.to_string(), s.clone());
            }
            ValueKind::Array(arr) => {
                if properties_config.array_split {
                    for (index, item) in arr.iter().enumerate() {
                        let array_key = format!("{}[{}]", prefix, index);
                        Self::handle_value(&array_key, item, properties, properties_config);
                    }
                } else {
                    let array_str = arr
                        .iter()
                        .map(|v| v.to_string())
                        .collect::<Vec<String>>()
                        .join(",");
                    properties.insert(prefix.to_string(), array_str);
                }
            }
            ValueKind::Table(nested_map) => {
                Self::flatten(prefix, nested_map, properties, properties_config);
            }
            ValueKind::Nil => {
                properties.insert(prefix.to_string(), "Null".to_string());
            }
        }
    }
    pub fn get_properties(&self) -> &HashMap<String, String> {
        &self.properties
    }
}
