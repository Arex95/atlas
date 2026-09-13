//! Agent memory vocabulary (Type 1 / Type 2 split).

mod error;
mod model;
mod validation;

pub use error::MemoryError;
pub use model::{MemoryEntry, MemoryScope};
pub use validation::validate_key;
