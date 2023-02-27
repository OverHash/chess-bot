use std::{sync::Arc, time::Duration};

use chrono::{TimeZone, Utc};
use error_stack::{IntoReport, Report, ResultExt};
use feed_rs::model::Feed;
use sqlx::SqlitePool;
use twilight_http::Client;
use twilight_model::{
    channel::message::{embed::EmbedAuthor, Embed},
    id::{
        marker::{ChannelMarker, RoleMarker},
        Id,
    },
    util::Timestamp,
};

use crate::error::RssError;

/// Retrieves the announcements for a specific channel at a `url` specified.
pub async fn get_channel_announcements(
    web_client: &reqwest::Client,
    url: &String,
) -> Result<Feed, Report<RssError>> {
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

    Ok(rss_feed)
}

/// Handles the announcement feed given a list of announcement URLs.
///
/// Checks for new announcements every `check_interval` and posts them to the
/// specified channel ID.
pub async fn handle_announcements(
    announcement_urls: Vec<(String, Id<ChannelMarker>, Option<Id<RoleMarker>>)>,
    pool: SqlitePool,
    client: Arc<Client>,
    check_interval: Duration,
) -> Result<(), Report<RssError>> {
    let web_client = reqwest::Client::new();

    loop {
        log::debug!("Checking for new announcement");

        // check for new announcements
        for (url, channel, role_id) in announcement_urls.iter() {
            let feed = get_channel_announcements(&web_client, url).await;

            // if it was an fetch/read error, output error and move to the next feed
            if let Err(report) = &feed {
                if matches!(report.current_context(), RssError::FetchError)
                    || matches!(report.current_context(), RssError::ReadError)
                {
                    log::error!("Failed to fetch feed at {url}: {report:?}");
                    log::info!("Ignoring fetch error, and continuing to next announcement stream");
                    continue;
                }
            }

            // otherwise, try decode the value and handle logic
            let feed = feed?;

            // check updated time against database
            let Some(updated_time) = feed.updated else {
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
                feed.id
            )
            .fetch_optional(&mut *pool)
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
				"#, feed.id, current_time).execute(&mut *pool).await.into_report().change_context(RssError::DatabaseError)?;

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
                feed.id
            )
            .execute(&mut *pool)
            .await
            .into_report()
            .change_context(RssError::DatabaseError)?;

            // if we have already processed the last event
            if database_updated_time == updated_time {
                continue;
            }

            // there are new events, get them all!
            let new_entries = feed.entries.into_iter().filter_map(|entry| {
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
                log::info!(
                    "A new post in {} was made at {post_date}",
                    entry
                        .title
                        .clone()
                        .map(|title| title.content)
                        .unwrap_or(entry.id)
                );
                client
                    .create_message(channel.to_owned())
                    .content(&match role_id {
                        Some(id) => format!("<@&{id}>"),
                        None => String::new(),
                    })
                    .into_report()
                    .change_context(RssError::PostError)?
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
                                    let mut filtered_body = body
                                        .replace("&nbsp;", "")
                                        .replace("<p>", "")
                                        .replace("</p>", "\n");

                                    filtered_body.truncate(4096);

                                    filtered_body
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
}
