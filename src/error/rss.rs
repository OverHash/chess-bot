use std::fmt::Display;

use error_stack::Context;

#[derive(Debug)]
pub enum RssError {
    // An error occurred when performing the fetch request to a page
    Fetch,
    // An error occurred when reading the rss response from a page
    Read,
    // Failed to handle connection to the database
    Database,
    // Failed to post an announcement to Discord.
    Post,
}

impl Display for RssError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Fetch => write!(f, "Failed to fetch RSS data from Canvas server"),
            Self::Read => write!(f, "Failed to decode RSS response from Canvas server"),
            Self::Database => write!(f, "Failed to process database event"),
            Self::Post => write!(f, "Failed to post an RSS event to the Discord channel"),
        }
    }
}

impl Context for RssError {}
