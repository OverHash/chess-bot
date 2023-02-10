use std::{env, time::Duration};

use error_stack::{IntoReport, Result, ResultExt};
use twilight_model::id::{marker::ChannelMarker, Id};

use crate::error::ConfigError;

pub struct ApplicationConfig {
    /// The token to be used to login to the Discord bot.
    pub discord_token: String,
    /// The URL of the database server to connect to or create, if it does not exist.
    pub database_url: String,
    /// The amount of unique reactions (not including message author) to a message to make it starboard material.
    pub reaction_requirement: u32,
    /// The channel to post starboard messages into
    pub starboard_channel_id: Id<ChannelMarker>,
    /// The announcement RSS URLs to read from, paired with the channel ID to post to.
    ///
    /// This is an optional feature, and the user may not specify it.
    pub announcement_rss_urls: Option<Vec<(String, Id<ChannelMarker>)>>,
    /// The amount of time (in seconds) to wait before performing checking operations for new announcements.
    pub announcement_check_interval: Duration,
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
        let starboard_channel_id = Id::new(
            load_env("STARBOARD_CHANNEL_ID")?
                .parse()
                .into_report()
                .change_context(ConfigError::ParseError {
                    config_option: "STARBOARD_CHANNEL_ID".to_string(),
                })?,
        );
        let announcement_check_interval = load_env("ANNOUNCEMENT_CHECK_INTERVAL")?
            .parse::<u64>()
            .into_report()
            .change_context(ConfigError::ParseError {
                config_option: "ANNOUNCEMENT_CHECK_INTERVAL".to_string(),
            })?;
        let announcement_check_interval = Duration::from_secs(announcement_check_interval);

        // since this is an optional feature, if it didn't exist, then no problem
        let announcement_rss_urls = load_env("CANVAS_ANNOUNCEMENT_URLS").ok();
        let announcement_rss_urls = announcement_rss_urls
            .map(|val| {
                // each new line denotes a new URL and channel pair
                val.split('\n')
                    .map(|line| {
                        let Some((rss, channel_id)) = line.split_once(',') else {
					return Ok(None);
				};
                        // attempt to parse the channel id and create channel marker
                        let channel_id = channel_id.parse::<u64>().into_report().change_context(
                            ConfigError::ParseError {
                                config_option: "STARBOARD_CHANNEL_ID".to_string(),
                            },
                        )?;

                        let channel_marker = Id::new(channel_id);

                        Ok(Some((rss.to_string(), channel_marker)))
                    })
                    .collect::<Result<Vec<_>, _>>()
            })
            // turn our Option<Result<...>> into a Result<Option<...>>
            .transpose()?
            // turn our Option<Vec<Option<...>>> into a Option<Vec<...>>
            .map(|urls| urls.into_iter().flatten().collect());

        Ok(Self {
            database_url,
            discord_token,
            reaction_requirement,
            starboard_channel_id,
            announcement_rss_urls,
            announcement_check_interval,
        })
    }
}
