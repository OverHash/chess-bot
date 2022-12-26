use error_stack::{IntoReport, Result, ResultExt};
use futures::stream::StreamExt;
use sqlx::{
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
    SqlitePool,
};
use std::{str::FromStr, sync::Arc};
use twilight_cache_inmemory::{InMemoryCache, ResourceType};
use twilight_gateway::{Event, Intents, Shard};
use twilight_http::{request::channel::reaction::RequestReactionType, Client};
use twilight_model::id::{marker::MessageMarker, Id};

mod config;
mod error;

use config::ApplicationConfig;
use error::{
    ApplicationError, ConfigError, DatabaseError, DiscordError, EventError, ReactionError,
};

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

    // let Discord know the intentions we need to run the bot with the correct intentions
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

    // Startup an event loop to process each event in the event stream as they
    // come in.
    let client = Arc::new(Client::new(config.discord_token.to_owned()));
    while let Some(event) = events.next().await {
        let cache = cache.clone();
        // Update the cache.
        cache.update(&event);

        // Spawn a new task to handle the event
        tokio::spawn(handle_event(
            event,
            client.clone(),
            cache,
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
    cache: Arc<InMemoryCache>,
    pool: SqlitePool,
    config: Arc<ApplicationConfig>,
) -> Result<(), EventError> {
    match event {
        Event::ReactionAdd(added) => {
            // first check if message has already been starboard'd
            let mut pool = pool
                .acquire()
                .await
                .into_report()
                .change_context(EventError::ReactionError(ReactionError::DatabaseConnect))?;

            let message_id = added.message_id.to_string();

            let starboard_id: Option<Id<MessageMarker>> = sqlx::query!(
                r#"
SELECT starboard_id
FROM starboard
WHERE message_id = ?
				"#,
                message_id
            )
            .fetch_optional(&mut pool)
            .await
            .into_report()
            .change_context(EventError::ReactionError(
                ReactionError::PreviousReactionCount,
            ))?
            .map(|id| -> std::result::Result<u64, _> { id.starboard_id.try_into() })
            .transpose()
            .into_report()
            .change_context(EventError::ReactionError(
                ReactionError::PreviousReactionCount,
            ))?
            .map(Id::new);

            // if the starboard message was already created
            // we do not need to do any more work
            if starboard_id.is_some() {
                return Ok(());
            }

            // retrieve the amount of reactions the message has now
            // if the message was not in cache, we need to perform an http request to get it
            let reactions = match cache.message(added.message_id) {
                Some(m) => m.reactions().to_owned(),
                None => {
                    let message = http
                        .message(added.channel_id, added.message_id)
                        .await
                        .into_report()
                        .change_context(EventError::ReactionError(ReactionError::RetrieveMessage))?
                        .model()
                        .await
                        .into_report()
                        .change_context(EventError::ReactionError(
                            ReactionError::RetrieveMessage,
                        ))?;

                    message.reactions
                }
            };

            // check if we are above the config `reaction_requirement` threshold
            // if not, early exit
            let max_reactions = reactions.iter().map(|r| r.count).max().unwrap_or_default();
            if max_reactions < config.reaction_requirement.into() {
                println!(
                    "message {message_id} has {max_reactions} max reactions for a single emoji now"
                );

                http.create_reaction(
                    added.channel_id,
                    added.message_id,
                    &RequestReactionType::Unicode { name: "💀" },
                )
                .await
                .expect("Failed to react");

                return Ok(());
            }

            // add to starboard!
            // todo
            http.create_message(added.channel_id)
                .content(&format!(
                    "added reaction {:?}, which now has {reactions:?} unique emoji reactions",
                    added.emoji
                ))
                .into_report()
                .change_context(EventError::ReactionError(
                    ReactionError::ContentResponseTooLong,
                ))?
                .reply(added.message_id)
                .await
                .into_report()
                .change_context(EventError::ReactionError(ReactionError::ReplyMessage))?;
        }
        Event::ShardConnected(e) => {
            println!("Connected on shard {}", e.shard_id);
        }
        _ => {}
    }

    Ok(())
}
