use std::fmt::{self, Display, Formatter};

use error_stack::Context;

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

impl Display for ReactionError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
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
