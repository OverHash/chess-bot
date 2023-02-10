use chrono::{TimeZone, Utc};
use error_stack::{IntoReport, Result, ResultExt};
use futures::stream::StreamExt;
use sqlx::{
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
    SqlitePool,
};
use std::{str::FromStr, sync::Arc};
use tokio::task::JoinHandle;
use twilight_cache_inmemory::{InMemoryCache, ResourceType};
use twilight_gateway::{Event, Intents, Shard};
use twilight_http::Client;
use twilight_model::{
    channel::message::{embed::EmbedAuthor, Embed},
    util::Timestamp,
};

mod config;
mod create_starboard_message;
mod error;
mod events;

use config::ApplicationConfig;
use error::{ApplicationError, ConfigError, DatabaseError, DiscordError, EventError, RssError};

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

        let _: JoinHandle<Result<(), RssError>> = tokio::spawn(async move {
            println!("Firing RSS requests");
            let web_client = reqwest::Client::new();

            loop {
                // check for new announcements
                for (url, channel) in announcement_urls.iter() {
                    let rss_feed = web_client
                        .get(url)
                        .send()
                        .await
                        .into_report()
                        .change_context(RssError::FetchError)?
                        .bytes()
                        .await
                        .into_report()
                        .change_context(RssError::FetchError)?;

                    let rss_feed = feed_rs::parser::parse_with_uri(&rss_feed[..], Some(url))
                        .into_report()
                        .change_context(RssError::ReadError)?;

                    // check updated time against database
                    let Some(updated_time) = rss_feed.updated else {
						// go to the next stream
						continue;
					};

                    let mut pool = pool
                        .acquire()
                        .await
                        .into_report()
                        .change_context(RssError::DatabaseError)?;

                    let database_updated_time = sqlx::query!(
                        r#"
						SELECT last_updated_time FROM announcement_feed WHERE id = ?
						"#,
                        rss_feed.id
                    )
                    .fetch_optional(&mut pool)
                    .await
                    .into_report()
                    .change_context(RssError::DatabaseError)?
                    .map(|timestamp| {
                        Utc.timestamp_millis_opt(timestamp.last_updated_time)
                            .single()
                            .ok_or(RssError::DatabaseError)
                    })
                    .transpose()?;

                    let current_time = Utc::now().timestamp_millis();
                    let Some(database_updated_time) = database_updated_time else {
						// this is our first time running this announcement stream
						// mark the current time and go to the next announcement stream
						// otherwise we will flood the output with announcements

						sqlx::query!(r#"
						INSERT INTO announcement_feed (id, last_updated_time)
						VALUES (?, ?)
						"#, rss_feed.id, current_time).execute(&mut pool).await.into_report().change_context(RssError::DatabaseError)?;

						continue;
					};

                    // update last update time in database
                    sqlx::query!(
                        r#"
					UPDATE announcement_feed
					SET last_updated_time = ?
					WHERE id = ?
					"#,
                        current_time,
                        rss_feed.id
                    )
                    .execute(&mut pool)
                    .await
                    .into_report()
                    .change_context(RssError::DatabaseError)?;

                    // if we have already processed the last event
                    if database_updated_time == updated_time {
                        continue;
                    }

                    // there are new events, get them all!
                    let new_entries = rss_feed.entries.into_iter().filter_map(|entry| {
                        entry
                            .updated
                            .map(|date| {
                                if date > database_updated_time {
                                    Some((entry, date))
                                } else {
                                    None
                                }
                            })
                            .unwrap_or_default()
                    });

                    for (entry, post_date) in new_entries {
                        println!("A new post was made!");
                        client
                            .create_message(channel.to_owned())
                            .embeds(&[Embed {
                                author: Some(EmbedAuthor {
                                    name: entry
                                        .authors
                                        .into_iter()
                                        .map(|author| author.name)
                                        .collect::<Vec<String>>()
                                        .join(", "),
                                    icon_url: None,
                                    proxy_icon_url: None,
                                    url: None,
                                }),
                                color: Some(15844367),
                                description: entry
                                    .content
                                    .map(|content| {
                                        content.body.map(|body| {
                                            body.replace("&nbsp;", "")
                                                .replace("<p>", "")
                                                .replace("</p>", "\n")
                                        })
                                    })
                                    .flatten(),
                                title: entry.title.map(|title| title.content),
                                // use this instead of first() so we can take ownership of the link
                                url: entry.links.into_iter().nth(0).map(|link| link.href),
                                fields: vec![],
                                footer: None,
                                timestamp: Some(
                                    Timestamp::from_micros(post_date.timestamp_micros())
                                        .into_report()
                                        .change_context(RssError::PostError)?,
                                ),
                                image: None,
                                kind: "rich".to_string(),
                                provider: None,
                                thumbnail: None,
                                video: None,
                            }])
                            .into_report()
                            .change_context(RssError::PostError)?
                            .await
                            .into_report()
                            .change_context(RssError::PostError)?;
                    }
                }

                tokio::time::sleep(check_interval).await;
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
