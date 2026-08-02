use std::{
    error::Error,
    fmt::{self, Display, Formatter},
};

#[derive(Debug)]
pub enum ConfigError {
    EnvError { env_name: String },
    ParseError { config_option: String },
}

impl Display for ConfigError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::EnvError { env_name } => {
                write!(f, "Failed to load environment for variable '{env_name}'")
            }
            ConfigError::ParseError { config_option } => {
                write!(f, "Failed to parse configuration for '{config_option}'")
            }
        }
    }
}

impl Error for ConfigError {}
