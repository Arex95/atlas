//! Every text file becomes exactly one node.
//!
//! The simplest extractor, and the one the others depend on: it emits
//! the file node that markdown sections hang from and that import
//! edges start at.

use super::{Extractor, FileContext};
use crate::internal::domain::{ExtractedNode, FileExtraction, NodeKind};

/// How much of a file to keep as a preview. Enough for an agent to
/// recognise what it is looking at without pulling the file into
/// context.
const EXCERPT_BYTES: usize = 400;

pub struct FilesystemExtractor;

impl Extractor for FilesystemExtractor {
    fn name(&self) -> &'static str {
        "filesystem"
    }

    fn handles(&self, _file: &FileContext<'_>) -> bool {
        true
    }

    fn extract(&self, file: &FileContext<'_>) -> FileExtraction {
        let name = file
            .relative_path
            .rsplit('/')
            .next()
            .unwrap_or(file.relative_path)
            .to_owned();

        let line_count = file.content.lines().count().max(1);

        FileExtraction {
            nodes: vec![ExtractedNode {
                fqn: file.relative_path.to_owned(),
                name,
                kind: NodeKind::File,
                extension: file.extension.to_owned(),
                file_path: file.relative_path.to_owned(),
                start_line: 1,
                end_line: i64::try_from(line_count).unwrap_or(i64::MAX),
                excerpt: Some(excerpt(file.content)),
                content_hash: file.content_hash.to_owned(),
                search_text: file.content.to_owned(),
            }],
            edges: Vec::new(),
        }
    }
}

/// Truncates on a character boundary — slicing bytes would panic on
/// any file whose first 400 bytes end mid-codepoint, which is most
/// files with an accent in them.
fn excerpt(content: &str) -> String {
    let trimmed = content.trim_start();
    match trimmed.char_indices().nth(EXCERPT_BYTES) {
        Some((idx, _)) => format!("{}…", &trimmed[..idx]),
        None => trimmed.to_owned(),
    }
}
