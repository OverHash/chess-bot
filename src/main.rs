use error_stack::{Result, ResultExt};
use sqlx::{
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
    SqlitePool,
};
use std::{str::FromStr, sync::Arc};
use twilight_cache_inmemory::{InMemoryCache, ResourceType};
use twilight_gateway::{Event, Intents, Shard, ShardId};
use twilight_http::Client;

mod config;
mod create_starboard_message;
mod error;
mod events;
mod rss_announcements;

use config::ApplicationConfig;
use error::{ApplicationError, ConfigError, DatabaseError, DiscordError, EventError};

use crate::rss_announcements::handle_announcements;

#[tokio::main]
async fn main() -> Result<(), ApplicationError> {
    // load `.env` file (if it exists) and subsequent config file into memory
    dotenvy::dotenv().ok();

    env_logger::init();

    let config = Arc::new(ApplicationConfig::load().change_context(ApplicationError::LoadConfig)?);
    log::debug!("Loaded config: {config:?}");

    // connect to sqlite database
    let connection_options = SqliteConnectOptions::from_str(&config.database_url)
        .change_context(ApplicationError::LoadConfig)
        .attach(ConfigError::ParseError {
            config_option: "DATABASE_URL".to_string(),
        })?
        .create_if_missing(true);
    let pool = SqlitePoolOptions::new()
        .connect_with(connection_options)
        .await
        .change_context(ApplicationError::Database(DatabaseError::ConnectError))?;
    log::info!(
        "Connected to sqlite database with {} connections",
        pool.num_idle()
    );

    // let Discord know the intentions we need to run the bot with
    let intents = Intents::GUILDS
        | Intents::GUILD_MESSAGES
        | Intents::MESSAGE_CONTENT
        | Intents::GUILD_MESSAGE_REACTIONS;
    let mut cluster = Shard::new(ShardId::ONE, config.discord_token.clone(), intents);

    // Since we only care about message emojis, make the cache only process messages.
    let cache = Arc::new(
        InMemoryCache::builder()
            .resource_types(ResourceType::MESSAGE | ResourceType::REACTION)
            .build(),
    );

    let client = Arc::new(Client::new(config.discord_token.to_owned()));

    // if there was announcement urls, spawn up a thread to handle checking it
    if let Some(announcement_urls) = config.announcement_rss_urls.to_owned() {
        let check_interval = config.announcement_check_interval;
        let pool = pool.clone();
        let client = client.clone();

        tokio::spawn(async move {
            let result =
                handle_announcements(announcement_urls, pool, client, check_interval).await;
            if let Err(report) = result {
                log::error!("RSS task failed: {report:?}");
            } else {
                log::debug!("RSS announcement thread completed with Ok variant");
            }
        });
    }

    // Startup an event loop to process each event in the event stream as they
    // come in.
    loop {
        match cluster.next_event().await {
            Ok(event) => {
                let cache = cache.clone();
                // Update the cache.
                cache.update(&event);

                // Spawn a new task to handle the event
                tokio::spawn(handle_event(
                    event,
                    client.clone(),
                    pool.clone(),
                    config.clone(),
                ))
                .await
                .change_context(ApplicationError::Thread)?
                .change_context(ApplicationError::Event)?;
            }
            Err(source) => {
                if source.is_fatal() {
                    return Err(source)
                        .change_context(ApplicationError::Discord(DiscordError::ConnectError))?;
                }
            }
        };
    }
}

async fn handle_event(
    event: Event,
    http: Arc<Client>,
    pool: SqlitePool,
    config: Arc<ApplicationConfig>,
) -> Result<(), EventError> {
    match event {
        Event::ReactionAdd(added) => {
            log::debug!("Received ReactionAdd event to message {}", added.message_id);
            events::reaction_add(added, http, pool, config)
                .await
                .change_context(EventError::ReactionError)?;
        }
        Event::GatewayHello(_) => {
            log::debug!("Connected to Discord gateway");
        }
        _ => {}
    }

    Ok(())
}
