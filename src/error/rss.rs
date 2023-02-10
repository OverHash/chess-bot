use std::fmt::Display;

use error_stack::Context;

#[derive(Debug)]
pub enum RssError {
    // An error occurred when performing the fetch request to a page
    FetchError,
    // An error occurred when reading the rss response from a page
    ReadError,
    // Failed to handle connection to the database
    DatabaseError,
    // Failed to post an announcement to Discord.
    PostError,
}

impl Display for RssError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FetchError => write!(f, "Failed to fetch RSS data from Canvas server"),
            Self::ReadError => write!(f, "Failed to decode RSS response from Canvas server"),
            Self::DatabaseError => write!(f, "Failed to process database event"),
            Self::PostError => write!(f, "Failed to post an RSS event to the Discord channel"),
        }
    }
}

impl Context for RssError {}
