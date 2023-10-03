use std::sync::Arc;

use error_stack::{Report, Result, ResultExt};
use sqlx::SqlitePool;
use twilight_http::Client;
use twilight_model::{
    gateway::payload::incoming::ReactionAdd,
    id::{marker::MessageMarker, Id},
};

use crate::{
    config::ApplicationConfig, create_starboard_message::create_starboard_message,
    error::ReactionError,
};

/// Fired when a reaction is added to a message.
///
/// Handles updating the starboard channel.
pub async fn reaction_add(
    added: Box<ReactionAdd>,
    http: Arc<Client>,
    pool: SqlitePool,
    config: Arc<ApplicationConfig>,
) -> Result<(), ReactionError> {
    // ensure that message was in a server we are tracking
    // if we are not tracking a server id, then we default to
    // accepting this incoming event
    if !config
        .server_id
        .map(|id| Some(id) == added.guild_id)
        .unwrap_or(true)
    {
        return Ok(());
    }

    // first check if message has already been starboard'd
    let mut pool = pool
        .acquire()
        .await
        .change_context(ReactionError::DatabaseConnect)?;

    let message_id = added.message_id.to_string();

    let starboard_id: Option<Id<MessageMarker>> = sqlx::query!(
        r#"
SELECT starboard_id
FROM starboard
WHERE message_id = ?
	"#,
        message_id
    )
    .fetch_optional(&mut *pool)
    .await
    .map_err(Report::new)
    .change_context(ReactionError::PreviousReactionCount)?
    .map(|id| -> std::result::Result<u64, _> { id.starboard_id.try_into() })
    .transpose()
    .change_context(ReactionError::PreviousReactionCount)?
    .map(Id::new);

    // retrieve the amount of reactions the message has now
    let message = http
        .message(added.channel_id, added.message_id)
        .await
        .change_context(ReactionError::RetrieveMessage)?
        .model()
        .await
        .change_context(ReactionError::RetrieveMessage)?;

    // check if we are above the config `reaction_requirement` threshold
    // if not, early exit
    let max_reactions = message
        .reactions
        .iter()
        .map(|r| r.count)
        .max()
        .unwrap_or_default();
    log::info!("message {message_id} has {max_reactions} max reactions for a single emoji now");

    // update the starboard message if we already made one
    // to display the new amount of reactions
    if let Some(starboard_message_id) = starboard_id {
        let new_message = create_starboard_message(message);

        http.update_message(config.starboard_channel_id, starboard_message_id)
            .content(Some(&new_message.content))
            .change_context(ReactionError::ContentResponseTooLong)?
            .embeds(Some(&new_message.embeds))
            .change_context(ReactionError::StarboardMessage)?
            .await
            .change_context(ReactionError::StarboardMessage)?;

        return Ok(());
    }

    // check if not enough reactions were done to make a starboard post
    if max_reactions < config.reaction_requirement.into() {
        return Ok(());
    }

    // add to starboard!
    let starboard_message = create_starboard_message(message);
    let starboard_message = http
        .create_message(config.starboard_channel_id)
        .content(&starboard_message.content)
        .change_context(ReactionError::ContentResponseTooLong)?
        .embeds(&starboard_message.embeds)
        .change_context(ReactionError::StarboardMessage)?
        .await
        .change_context(ReactionError::StarboardMessage)?
        .model()
        .await
        .change_context(ReactionError::StarboardMessage)?;

    let starboard_message_id = starboard_message.id.to_string();

    sqlx::query!(
        r#"
INSERT INTO starboard (starboard_id, message_id)
VALUES (?, ?)
		"#,
        starboard_message_id,
        message_id
    )
    .execute(&mut *pool)
    .await
    .change_context(ReactionError::PreviousReactionCount)?;

    Ok(())
}
