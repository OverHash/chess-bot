use error_stack::{IntoReport, Result, ResultExt};
use futures::stream::StreamExt;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::{str::FromStr, sync::Arc};
use twilight_cache_inmemory::{InMemoryCache, ResourceType};
use twilight_gateway::{Event, Intents, Shard};
use twilight_http::Client;

mod config;
mod error;

use config::ApplicationConfig;
use error::{
    ApplicationError, ConfigError, DatabaseError, DiscordError, EventError, MessageCreateError,
};

#[tokio::main]
async fn main() -> Result<(), ApplicationError> {
    dotenvy::dotenv().ok();

    let config = ApplicationConfig::load().change_context(ApplicationError::LoadConfig)?;

    let connection_options = SqliteConnectOptions::from_str(&config.database_url)
        .into_report()
        .change_context(ApplicationError::LoadConfig)
        .attach(ConfigError::ParseError {
            config_option: "DATABASE_URI".to_string(),
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

    // Let Discord know the intentions we need to run the bot
    let intents = Intents::GUILDS | Intents::GUILD_MESSAGES | Intents::MESSAGE_CONTENT;
    let (cluster, mut events) = Shard::new(config.discord_token.clone(), intents);
    cluster
        .start()
        .await
        .into_report()
        .change_context(ApplicationError::Discord(DiscordError::ConnectError))?;

    let client = Arc::new(Client::new(config.discord_token));

    // Since we only care about messages, make the cache only process messages.
    let cache = InMemoryCache::builder()
        .resource_types(ResourceType::MESSAGE)
        .build();

    // Startup an event loop to process each event in the event stream as they
    // come in.
    while let Some(event) = events.next().await {
        // Update the cache.
        cache.update(&event);

        // Spawn a new task to handle the event
        tokio::spawn(handle_event(event, client.clone()))
            .await
            .into_report()
            .change_context(ApplicationError::Thread)?
            .change_context(ApplicationError::Event)?;
    }

    Ok(())
}

async fn handle_event(event: Event, http: Arc<Client>) -> Result<(), EventError> {
    match event {
        Event::MessageCreate(msg) if msg.content == "!ping" => {
            http.create_message(msg.channel_id)
                .content("Pong!")
                .into_report()
                .change_context(EventError::MessageCreateError(
                    MessageCreateError::ContentResponseError,
                ))?
                .await
                .into_report()
                .change_context(EventError::MessageCreateError(
                    MessageCreateError::ReplyMessageError,
                ))?;
        }
        Event::ShardConnected(e) => {
            println!("Connected on shard {}", e.shard_id);
        }
        _ => {}
    }

    Ok(())
}
