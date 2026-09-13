//! The coordination-protocol vocabulary.

mod error;
mod message;

pub use error::MessagingError;
pub use message::{Message, MessageId, NewMessage};
