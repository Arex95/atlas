//! The extractor port, and the generic extractors behind it.
//!
//! This boundary is the one piece worth keeping from the symbol-graph
//! design that preceded this one: schema, storage and resolution are
//! extractor-agnostic, so adding a precise per-language extractor —
//! the day one language earns the cost of a grammar — is an adapter
//! rather than a rewrite.
//!
//! Everything behind it today is generic: it reads text and matches
//! patterns, and knows nothing about any language. That is why a
//! repository is fully indexed the day it is written, in whatever it
//! is written in.

mod filesystem;
pub mod imports;
mod markdown;

pub use filesystem::FilesystemExtractor;
pub use imports::{ImportsExtractor, is_code_path};
pub use markdown::MarkdownExtractor;

use crate::internal::domain::FileExtraction;

/// One file, read once and handed to every extractor.
///
/// Reading is the indexer's job rather than each extractor's: a file
/// is read once no matter how many extractors look at it, and an
/// extractor cannot accidentally read something the walker excluded.
pub struct FileContext<'a> {
    /// Path relative to the project root, always with `/` separators.
    pub relative_path: &'a str,
    pub extension: &'a str,
    pub content: &'a str,
    pub content_hash: &'a str,
}

/// Turns one file into nodes and unresolved edges.
///
/// An extractor never resolves a target and never looks at another
/// file. Both are deliberate: resolution needs the whole project,
/// which only the indexer has, and an extractor that reaches across
/// files cannot be reasoned about or tested on its own.
pub trait Extractor: Send + Sync {
    /// Short name, for logs and for saying which extractor produced
    /// what when one of them misbehaves.
    ///
    /// Nothing calls this yet. It stays because an extractor that
    /// cannot identify itself is undiagnosable the first time two of
    /// them disagree about a file, and adding it later means touching
    /// every implementation.
    #[allow(dead_code)]
    fn name(&self) -> &'static str;

    /// Whether this extractor has anything to say about the file.
    /// Checked before `extract`, so an extractor that does not apply
    /// costs a comparison rather than a pass over the content.
    fn handles(&self, file: &FileContext<'_>) -> bool;

    fn extract(&self, file: &FileContext<'_>) -> FileExtraction;
}

/// The extractors every index runs, in order.
///
/// Filesystem comes first because it emits the file node the others
/// attach their edges to.
#[must_use]
pub fn default_extractors() -> Vec<Box<dyn Extractor>> {
    vec![
        Box::new(FilesystemExtractor),
        Box::new(MarkdownExtractor),
        Box::new(ImportsExtractor::new()),
    ]
}
