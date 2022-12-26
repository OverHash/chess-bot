use std::env;

use error_stack::{IntoReport, Result, ResultExt};

use crate::error::ConfigError;

pub struct ApplicationConfig {
    /// The token to be used to login to the Discord bot.
    pub discord_token: String,
    /// The URL of the database server to connect to or create, if it does not exist.
    pub database_url: String,
    /// The amount of unique reactions (not including message author) to a message to make it starboard material.
    pub reaction_requirement: u32,
}

/// Loads the specified environment variable, returning `Ok` with the env variable if found, or `Err` if it was not found.
fn load_env<T>(env_var: T) -> Result<String, ConfigError>
where
    T: ToString,
{
    let variable =
        env::var(env_var.to_string())
            .into_report()
            .change_context(ConfigError::EnvError {
                env_name: env_var.to_string(),
            })?;

    Ok(variable)
}

impl ApplicationConfig {
    /// Loads all environment variables, returning `Err` if one was missing.
    pub fn load() -> Result<Self, ConfigError> {
        let discord_token = load_env("DISCORD_TOKEN")?;
        let database_url = load_env("DATABASE_URL")?;
        let reaction_requirement: u32 = load_env("REACTION_REQUIREMENT")?
            .parse()
            .into_report()
            .change_context(ConfigError::ParseError {
                config_option: "REACTION_REQUIREMENT".to_string(),
            })?;

        Ok(Self {
            database_url,
            discord_token,
            reaction_requirement,
        })
    }
}
