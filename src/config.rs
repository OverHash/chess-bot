use std::env;

use error_stack::{IntoReport, Result, ResultExt};

use crate::error::ConfigError;

pub struct ApplicationConfig {
    /// The token to be used to login to the Discord bot.
    pub discord_token: String,
    /// The URL of the database server to connect to or create, if it does not exist.
    pub database_url: String,
}

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
    pub fn load() -> Result<Self, ConfigError> {
        let discord_token = load_env("DISCORD_TOKEN")?;
        let database_uri = load_env("DATABASE_URI")?;

        Ok(Self {
            database_url: database_uri,
            discord_token,
        })
    }
}
