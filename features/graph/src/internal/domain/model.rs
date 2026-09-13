use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// What a node is. Two kinds, and the vocabulary grows only when
/// something is actually emitted — an enum full of aspirational
/// variants is a schema nobody can trust.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    /// A whole file.
    File,
    /// A markdown heading and the prose under it.
    Section,
}

impl NodeKind {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Section => "section",
        }
    }

    #[must_use]
    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "file" => Some(Self::File),
            "section" => Some(Self::Section),
            _ => None,
        }
    }
}

/// A typed, directed relation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgePredicate {
    /// Structural: a file contains a section, a section contains a
    /// deeper one.
    Contains,
    /// An explicit markdown link, `[text](path)`.
    Links,
    /// A heuristic: one file names another, by basename or by path.
    /// The only predicate here that can be wrong — treated as a hint
    /// everywhere it surfaces, never as a fact.
    Mentions,
    /// An import statement, matched by a language-agnostic pattern.
    Imports,
}

impl EdgePredicate {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Contains => "contains",
            Self::Links => "links",
            Self::Mentions => "mentions",
            Self::Imports => "imports",
        }
    }

    #[must_use]
    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "contains" => Some(Self::Contains),
            "links" => Some(Self::Links),
            "mentions" => Some(Self::Mentions),
            "imports" => Some(Self::Imports),
            _ => None,
        }
    }
}

/// A stored node.
#[derive(Clone, Debug, Serialize)]
pub struct GraphNode {
    pub id: String,
    pub project: String,
    /// Canonical identity within the project — `src/auth.rs`, or
    /// `docs/api.md#authentication` for a section.
    pub fqn: String,
    pub name: String,
    pub kind: NodeKind,
    pub extension: String,
    pub file_path: String,
    pub start_line: i64,
    pub end_line: i64,
    pub excerpt: Option<String>,
    pub content_hash: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// A stored edge.
///
/// `dst_id` is `None` when the target did not resolve to a real node.
/// `dst_fqn` is kept regardless, so an unresolved edge still says what
/// it was reaching for.
#[derive(Clone, Debug, Serialize)]
pub struct GraphEdge {
    pub id: String,
    pub project: String,
    pub src_id: String,
    pub dst_id: Option<String>,
    pub dst_fqn: String,
    pub predicate: EdgePredicate,
    pub file_path: String,
    pub line: i64,
}

// ---- what an extractor produces -------------------------------------
//
// Extractors emit *raw* targets and never resolve them. Resolution is
// the indexer's job, because only it has seen every file and can tell
// a real destination from a string that looks like one. Keeping that
// out of extractors is what makes a new extractor a small, local thing.

/// A node an extractor found, before it has an id.
#[derive(Clone, Debug)]
pub struct ExtractedNode {
    pub fqn: String,
    pub name: String,
    pub kind: NodeKind,
    pub extension: String,
    pub file_path: String,
    pub start_line: i64,
    pub end_line: i64,
    pub excerpt: Option<String>,
    pub content_hash: String,
    /// Indexed for search, not stored on the node.
    pub search_text: String,
}

/// An edge an extractor found, before its endpoints are resolved.
#[derive(Clone, Debug)]
pub struct ExtractedEdge {
    /// The `fqn` of the source node, which the same extractor emitted.
    pub src_fqn: String,
    /// What the source pointed at, verbatim. The indexer decides
    /// whether this names a real node.
    pub dst_fqn: String,
    pub predicate: EdgePredicate,
    pub file_path: String,
    pub line: i64,
}

/// Everything one extractor found in one file.
#[derive(Clone, Debug, Default)]
pub struct FileExtraction {
    pub nodes: Vec<ExtractedNode>,
    pub edges: Vec<ExtractedEdge>,
}

impl FileExtraction {
    pub fn merge(&mut self, other: Self) {
        self.nodes.extend(other.nodes);
        self.edges.extend(other.edges);
    }
}

/// What one indexing run did. Returned to the caller and logged —
/// `edges_unresolved` in particular is the number worth watching: it
/// is the cost of refusing to guess, and a sudden jump means an
/// extractor started emitting targets nothing can resolve.
#[derive(Clone, Copy, Debug, Default, Serialize)]
pub struct IndexStats {
    pub files_discovered: usize,
    pub files_indexed: usize,
    pub files_skipped: usize,
    pub nodes: usize,
    pub edges: usize,
    pub edges_resolved: usize,
    pub edges_unresolved: usize,
    pub elapsed_ms: u128,
}

/// One file node, reduced to what any finding needs.
///
/// A data type rather than a use case, so it lives in the domain: the
/// store builds these and the analyser reads them, and having it in
/// `application/` made the store import upward.
#[derive(Clone, Debug)]
pub struct FileFacts {
    pub id: String,
    pub fqn: String,
    pub lines: i64,
}
