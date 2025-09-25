use std::{
    collections::HashMap,
    env,
    path::{Path, PathBuf},
    sync::LazyLock,
};

use config::{ConfigError, File, ValueKind};
use di::injectable;
use serde::Deserialize;

#[derive(Clone)]
#[injectable]
pub struct Config {
    inner: config::Config,
}
static DEFAULT_CONFIG_FOLDER: LazyLock<PathBuf> = LazyLock::new(|| {
    let mut root_path = match env::var("CARGO_MANIFEST_DIR") {
        Ok(dir) => PathBuf::from(dir),
        Err(_) => {
            let mut current_exe =
                env::current_exe().expect("failed to get current executable file path");
            current_exe.pop();
            current_exe
        }
    };
    root_path.push("etc");
    root_path
});

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

        if cfg.exists() {
            builder = builder.add_source(File::from(cfg))
        } else {
            tracing::warn!("not found config `{}`", cfg.display());
        }
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
    pub fn to_properties(&self) -> Result<Properties, ConfigError> {
        Properties::from_config(self)
    }
}

pub trait ConfigPrefix {
    const PREFIX: &'static str;
}

pub struct Properties {
    properties: HashMap<String, String>,
}

pub struct PropertiesConfig {
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

            match &value.kind {
                ValueKind::Boolean(b) => {
                    properties.insert(full_key, b.to_string());
                }
                ValueKind::I64(i_64) => {
                    properties.insert(full_key, i_64.to_string());
                }
                ValueKind::I128(i_128) => {
                    properties.insert(full_key, i_128.to_string());
                }
                ValueKind::U64(u_64) => {
                    properties.insert(full_key, u_64.to_string());
                }
                ValueKind::U128(u_128) => {
                    properties.insert(full_key, u_128.to_string());
                }
                ValueKind::Float(f) => {
                    properties.insert(full_key, format!("{:.2}", f));
                }
                ValueKind::String(s) => {
                    properties.insert(full_key, s.clone());
                }
                ValueKind::Array(arr) => {
                    if properties_config.array_split {
                        for (index, item) in arr.iter().enumerate() {
                            let array_key = format!("{}[{}]", full_key, index);
                            properties.insert(array_key, item.to_string());
                        }
                    } else {
                        let array_str = arr
                            .iter()
                            .map(|v| v.to_string())
                            .collect::<Vec<String>>()
                            .join(",");
                        properties.insert(full_key, array_str);
                    }
                }
                ValueKind::Table(nested_map) => {
                    Self::flatten(&full_key, nested_map, properties, properties_config);
                }
                ValueKind::Nil => {
                    properties.insert(full_key, "Null".to_string());
                }
            }
        }
    }
    pub fn to_properties(&self) -> String {
        let mut lines: Vec<String> = Vec::new();

        for (key, value) in &self.properties {
            lines.push(format!("{} = {}", key, value));
        }
        lines.sort();
        lines.join("\n")
    }
    pub fn get_properties(&self) -> &HashMap<String, String> {
        &self.properties
    }
}
