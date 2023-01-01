use std::fmt::{self};

use error_stack::Context;

use super::{DatabaseError, DiscordError};

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
