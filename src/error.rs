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
pub enum MessageCreateError {
    /// The response message was too long
    ContentResponseError,
    /// Failed to reply to the user
    ReplyMessageError,
}

/// Errors associated when handling a Discord event
#[derive(Debug)]
pub enum EventError {
    /// Failed to handle an event related to message creation.
    ///
    /// The inner element represents a debug message to display.
    MessageCreateError(MessageCreateError),
}

impl EventError {
    fn get_event_name(&self) -> &'static str {
        match self {
            EventError::MessageCreateError(_) => "MessageCreate",
        }
    }
}

impl fmt::Display for EventError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let event_error = match self {
            EventError::MessageCreateError(create_error) => match create_error {
                MessageCreateError::ContentResponseError => {
                    "Response message exceeded maximum length"
                }
                MessageCreateError::ReplyMessageError => "Failed to respond to channel",
            },
        };

        write!(
            f,
            "Failed to process event '{}': {event_error}",
            self.get_event_name()
        )
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
            ApplicationError::Thread => write!(f, "Failed to handle thread unwinding"),
        }
    }
}
