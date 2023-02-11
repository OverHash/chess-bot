use error_stack::{IntoReport, Result, ResultExt};
use futures::stream::StreamExt;
use sqlx::{
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
    SqlitePool,
};
use std::{str::FromStr, sync::Arc};
use twilight_cache_inmemory::{InMemoryCache, ResourceType};
use twilight_gateway::{Event, Intents, Shard};
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

    let config = Arc::new(ApplicationConfig::load().change_context(ApplicationError::LoadConfig)?);

    // connect to sqlite database
    let connection_options = SqliteConnectOptions::from_str(&config.database_url)
        .into_report()
        .change_context(ApplicationError::LoadConfig)
        .attach(ConfigError::ParseError {
            config_option: "DATABASE_URL".to_string(),
        })?
        .create_if_missing(true);
    let pool = SqlitePoolOptions::new()
        .connect_with(connection_options)
        .await
        .into_report()
        .change_context(ApplicationError::Database(DatabaseError::ConnectError))?;
    println!(
        "Connected to sqlite database with {} connections",
        pool.num_idle()
    );

    // let Discord know the intentions we need to run the bot with
    let intents = Intents::GUILDS
        | Intents::GUILD_MESSAGES
        | Intents::MESSAGE_CONTENT
        | Intents::GUILD_MESSAGE_REACTIONS;
    let (cluster, mut events) = Shard::new(config.discord_token.clone(), intents);
    cluster
        .start()
        .await
        .into_report()
        .change_context(ApplicationError::Discord(DiscordError::ConnectError))?;

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
                println!("RSS task failed: {report}");
            }
        });
    }

    // Startup an event loop to process each event in the event stream as they
    // come in.
    while let Some(event) = events.next().await {
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
        .into_report()
        .change_context(ApplicationError::Thread)?
        .change_context(ApplicationError::Event)?;
    }

    Ok(())
}

async fn handle_event(
    event: Event,
    http: Arc<Client>,
    pool: SqlitePool,
    config: Arc<ApplicationConfig>,
) -> Result<(), EventError> {
    match event {
        Event::ReactionAdd(added) => {
            events::reaction_add(added, http, pool, config)
                .await
                .change_context(EventError::ReactionError)?;
        }
        Event::ShardConnected(e) => {
            println!("Connected on shard {}", e.shard_id);
        }
        _ => {}
    }

    Ok(())
}
