use futures::stream::StreamExt;
use std::{env, error::Error, future::IntoFuture, sync::Arc};
use twilight_cache_inmemory::{InMemoryCache, ResourceType};
use twilight_gateway::{Event, Intents, Shard};
use twilight_http::Client;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    dotenvy::dotenv().ok();

    let token = env::var("DISCORD_TOKEN")?;

    // Let Discord know the intentions we need to run the bot
    let intents = Intents::GUILDS | Intents::GUILD_MESSAGES | Intents::MESSAGE_CONTENT;
    let (cluster, mut events) = Shard::new(token.clone(), intents);
    cluster.start().await?;

    let client = Arc::new(Client::new(token));

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
        tokio::spawn(handle_event(event, client.clone()));
    }

    Ok(())
}

async fn handle_event(event: Event, http: Arc<Client>) -> Result<(), Box<dyn Error + Send + Sync>> {
    match event {
        Event::MessageCreate(msg) if msg.content == "!ping" => {
            http.create_message(msg.channel_id)
                .content("Pong!")?
                .into_future()
                .await?;
        }
        Event::ShardConnected(e) => {
            println!("Connected on shard {}", e.shard_id);
        }
        _ => {}
    }

    Ok(())
}
