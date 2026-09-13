//! Import statements, matched by patterns that hold across languages.
//!
//! Six forms cover most of what people write: `import`, `from … import`,
//! `require(…)`, `use …`, `#include …`, `using …`. That is deliberately
//! shallow — this is not a parser, and it does not know a language from
//! a shopping list.
//!
//! What it buys is that a repository is indexed the day it appears, in
//! whatever it is written in, including languages nobody has thought
//! about. What it costs is precision, which is why an import that does
//! not resolve to a real file in the project is dropped rather than
//! stored pointing at a guess: a wrong edge sends an agent somewhere
//! confidently, and that is worse than no edge.

use regex::Regex;

use super::{Extractor, FileContext};
use crate::internal::domain::{EdgePredicate, ExtractedEdge, FileExtraction};

/// Extensions to try when an import omits one — `./auth` meaning
/// `./auth.ts`. Ordered by how likely the guess is to be right.
const CANDIDATE_EXTENSIONS: &[&str] = &[
    "rs", "ts", "tsx", "js", "jsx", "mjs", "py", "go", "rb", "java", "kt", "c", "h", "cpp", "hpp",
    "php", "vue", "svelte",
];

/// Tried when an import names a directory — `./auth` meaning
/// `./auth/index.ts` or `./auth/mod.rs`.
const INDEX_FILES: &[&str] = &["index.ts", "index.tsx", "index.js", "mod.rs", "__init__.py"];

/// Whether this is a file the import extractor can read.
///
/// Used to decide what a finding may call a source file. It is
/// deliberately the extractor's own list rather than a separate one:
/// a file whose imports were never looked for cannot meaningfully be
/// reported as importing nothing, and a denylist of config formats
/// would silently grow stale — `.gitignore` and `.editorconfig` were
/// both reported as unreferenced source files before this existed.
///
/// Conservative on purpose. A language missing from the list is
/// omitted from those findings rather than described wrongly.
#[must_use]
pub fn is_code_path(fqn: &str) -> bool {
    fqn.rsplit_once('.')
        .is_some_and(|(_, ext)| CANDIDATE_EXTENSIONS.contains(&ext))
}

pub struct ImportsExtractor {
    patterns: Vec<Regex>,
}

impl Default for ImportsExtractor {
    fn default() -> Self {
        Self::new()
    }
}

impl ImportsExtractor {
    #[must_use]
    pub fn new() -> Self {
        // Compiled once and reused: these run over every line of every
        // file, and rebuilding them per file dominated the index.
        let patterns = [
            // import x from "path" / import "path"
            r#"^\s*import\s+(?:.*?\s+from\s+)?['"]([^'"]+)['"]"#,
            // from path import x  (Python)
            r"^\s*from\s+([\w./]+)\s+import\s",
            // require("path")
            r#"require\(\s*['"]([^'"]+)['"]\s*\)"#,
            // use crate::path  (Rust)
            r"^\s*(?:pub\s+)?use\s+([\w:]+)",
            // #include "path"  (C/C++)
            r#"^\s*#\s*include\s+["<]([^">]+)[">]"#,
            // using Namespace.Path  (C#)
            r"^\s*using\s+([\w.]+)\s*;",
        ];

        Self {
            patterns: patterns.iter().filter_map(|p| Regex::new(p).ok()).collect(),
        }
    }
}

impl Extractor for ImportsExtractor {
    fn name(&self) -> &'static str {
        "imports"
    }

    fn handles(&self, file: &FileContext<'_>) -> bool {
        // Prose has no imports, and running six regexes over every
        // line of every markdown file to learn that is waste.
        !matches!(file.extension, "md" | "markdown" | "mdx" | "txt" | "")
    }

    fn extract(&self, file: &FileContext<'_>) -> FileExtraction {
        let mut out = FileExtraction::default();

        for (index, line) in file.content.lines().enumerate() {
            // Cheap reject before six regexes: an import line always
            // contains one of these words.
            if !line.contains("import")
                && !line.contains("require")
                && !line.contains("use ")
                && !line.contains("include")
                && !line.contains("using")
            {
                continue;
            }

            for pattern in &self.patterns {
                let Some(captures) = pattern.captures(line) else {
                    continue;
                };
                let Some(raw) = captures.get(1).map(|m| m.as_str()) else {
                    continue;
                };
                let Some(target) = resolve(file.relative_path, raw) else {
                    continue;
                };
                out.edges.push(ExtractedEdge {
                    src_fqn: file.relative_path.to_owned(),
                    dst_fqn: target,
                    predicate: EdgePredicate::Imports,
                    file_path: file.relative_path.to_owned(),
                    line: i64::try_from(index + 1).unwrap_or(i64::MAX),
                });
                break;
            }
        }

        out
    }
}

/// Normalises an import target into a project-relative path shape.
///
/// Two forms, because languages split roughly in half:
///
/// * **Relative** — `./auth`, `../db/pool`. Resolved against the
///   importing file, which pins it to one place.
/// * **Module path** — `crate::internal::domain`, `a.b.C`,
///   `System.Text`. Separators become `/` and the result is matched
///   by *suffix* against real files, later, by the indexer. That is
///   still generic: Rust, Python, Java and C# all write a path with
///   a different separator, and none of them needs a parser here.
///
/// A single segment with no separator — `serde`, `react`, `os` — is
/// dropped. It names a dependency or a standard library, not a file
/// in this project, and an edge for it could never resolve.
fn resolve(from_file: &str, raw: &str) -> Option<String> {
    if !raw.starts_with('.') {
        let normalised = raw.replace("::", "/").replace('.', "/");
        let segments = normalised.split('/').filter(|s| !s.is_empty()).count();
        return (segments >= 2).then_some(normalised);
    }

    let base: Vec<&str> = from_file.split('/').collect();
    let mut stack: Vec<String> = base[..base.len().saturating_sub(1)]
        .iter()
        .map(|s| (*s).to_owned())
        .collect();

    for part in raw.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                stack.pop();
            }
            other => stack.push(other.to_owned()),
        }
    }

    let joined = stack.join("/");
    (!joined.is_empty()).then_some(joined)
}

/// Every path an import might have meant, in preference order. The
/// indexer keeps the first that names a real node and drops the edge
/// when none do.
#[must_use]
pub fn candidates(resolved: &str) -> Vec<String> {
    let mut out = vec![resolved.to_owned()];
    for ext in CANDIDATE_EXTENSIONS {
        out.push(format!("{resolved}.{ext}"));
    }
    for index in INDEX_FILES {
        out.push(format!("{resolved}/{index}"));
    }
    out
}
