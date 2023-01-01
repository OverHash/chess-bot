use std::fmt::{self, Display, Formatter};

use error_stack::Context;

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

impl Display for EventError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "Failed to process event '{}'", self.get_event_name())
    }
}

impl Context for EventError {}
