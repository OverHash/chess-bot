use std::fmt::{self};

use error_stack::Context;

#[derive(Debug)]
pub enum ConfigError {
    EnvError { env_name: String },
    ParseError { config_option: String },
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::EnvError { env_name } => {
                write!(f, "Failed to load environment for variable '{env_name}'")
            }
            ConfigError::ParseError { config_option } => {
                write!(f, "Failed to parse configuration for '{config_option}'")
            }
        }
    }
}

impl Context for ConfigError {}

#[derive(Debug)]
pub enum DatabaseError {
    ConnectError,
}

#[derive(Debug)]
pub enum DiscordError {
    ConnectError,
}

#[derive(Debug)]
pub enum ReactionError {
    /// Failed to acquire a lock on a database pool.
    DatabaseConnect,
    /// Failed to get the previous reaction count.
    PreviousReactionCount,
    /// Failed to retrieve the message reacted to.
    RetrieveMessage,
    /// The response message was too long.
    ContentResponseTooLong,
    /// Failed to generate starboard message
    StarboardMessage,
}

impl fmt::Display for ReactionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let event_error = match self {
            ReactionError::DatabaseConnect => "Failed to acquire database pool connection",
            ReactionError::PreviousReactionCount => {
                "Failed to retrieve the previous reaction count"
            }
            ReactionError::RetrieveMessage => "Failed to retrieve the message reacted to",
            ReactionError::ContentResponseTooLong => "Response message exceeded maximum length",
            ReactionError::StarboardMessage => "Failed to create starboard message",
        };

        write!(f, "{event_error}")
    }
}

impl Context for ReactionError {}

/// Errors associated when handling a Discord event
#[derive(Debug)]
pub enum EventError {
    /// Failed to handle a message having a reaction event (added / removed).
    ReactionError,
}

impl EventError {
    fn get_event_name(&self) -> &'static str {
        match self {
            EventError::ReactionError => "Reaction",
        }
    }
}

impl fmt::Display for EventError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Failed to process event '{}'", self.get_event_name())
    }
}

impl Context for EventError {}

/// The main application error entry point.
#[derive(Debug)]
pub enum ApplicationError {
    LoadConfig,
    Database(DatabaseError),
    Discord(DiscordError),
    Event,
    Thread,
}

impl Context for ApplicationError {}

impl fmt::Display for ApplicationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ApplicationError::LoadConfig => {
                write!(f, "Failed to load configuration of application")
            }
            ApplicationError::Database(database_error) => match database_error {
                DatabaseError::ConnectError => write!(f, "Failed when connecting to database"),
            },
            ApplicationError::Discord(discord_error) => match discord_error {
                DiscordError::ConnectError => write!(f, "Failed to start Discord bot"),
            },
            ApplicationError::Event => write!(f, "Failed to process event"),
            ApplicationError::Thread => write!(f, "Failed to handle tokio thread unwinding"),
        }
    }
}
