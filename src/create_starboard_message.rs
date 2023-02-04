use twilight_model::channel::{
    message::{
        embed::{EmbedAuthor, EmbedField, EmbedImage},
        Embed, ReactionType,
    },
    Message,
};

/// A struct that contains the relevant information to pass to an [`twilight_http::request::channel::message::UpdateMessage`]
/// or [`twilight_http::request::channel::message::CreateMessage`] call to create the appropriate starboard message.
pub struct StarboardMessage {
    /// The content of the starboard message.
    pub content: String,
    /// The embeds to attach for the starboard message.
    pub embeds: Vec<Embed>,
}

/// Generates the relevant fields to set in a [`twilight_http::request::channel::message::UpdateMessage`]
/// or [`twilight_http::request::channel::message::CreateMessage`] struct to represent a starboard message.
pub fn create_starboard_message(message: Message) -> StarboardMessage {
    let max_reactions = message
        .reactions
        .iter()
        .reduce(|current_max_reaction, reaction| {
            if reaction.count > current_max_reaction.count {
                reaction
            } else {
                current_max_reaction
            }
        })
        .expect("Call to create_starboard_message with a message that has no reactions");

    let content = format!(
        "{} {} in <#{}>",
        max_reactions.count,
        match &max_reactions.emoji {
            ReactionType::Unicode { name } => name.to_owned(),
            ReactionType::Custom { id, name, .. } =>
                format!("<:{}:{id}>", name.as_deref().unwrap_or_default()),
        },
        message.channel_id
    );

    let embeds = vec![Embed {
        author: Some(EmbedAuthor {
            icon_url: Some(match message.author.avatar {
                Some(hash) => format!(
                    "https://cdn.discordapp.com/avatars/{}/{}.{}",
                    message.author.id,
                    hash,
                    if hash.is_animated() { "gif" } else { "webp" }
                ),
                None => format!(
                    "https://cdn.discordapp.com/embed/avatars/{}.png",
                    message.author.discriminator % 5
                ),
            }),
            name: message.author.name,
            proxy_icon_url: None,
            url: None,
        }),
        color: Some(15844367),
        description: Some(message.content),
        fields: vec![EmbedField {
            inline: false,
            name: "Message Link".to_string(),
            value: format!(
                "[Click to jump to message](https://discord.com/channels/{}/{}/{})",
                match message.guild_id {
                    Some(guild_id) => guild_id.to_string(),
                    None => "@me".to_string(),
                },
                message.channel_id,
                message.id
            ),
        }],
        footer: None,
        timestamp: Some(message.timestamp),
        kind: "rich".to_string(),
        image: message.attachments.into_iter().next().map(|i| EmbedImage {
            url: i.url,
            proxy_url: Some(i.proxy_url),
            height: None,
            width: None,
        }),
        provider: None,
        thumbnail: None,
        title: None,
        url: None,
        video: None,
    }];

    StarboardMessage { content, embeds }
}
