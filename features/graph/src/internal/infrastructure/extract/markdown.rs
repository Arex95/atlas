//! Markdown headings become nodes; links between documents become
//! edges.
//!
//! For a repository whose reasoning lives in prose — decisions,
//! guides, READMEs — this is most of the useful graph. A heading is
//! the unit a person actually refers to ("see the auth section"), so
//! it is the unit worth addressing.
//!
//! Two things the parser has to get right, and both are why this is
//! a small state machine rather than a set of regexes:
//!
//! * **Fenced code.** A `# comment` inside a fenced block is not a
//!   heading, and treating it as one produces sections that do not
//!   exist in a file full of shell examples.
//! * **Nesting by level.** `##` under `#` is contained by it; the
//!   next `#` closes everything below. Tracking that needs a stack.

use super::{Extractor, FileContext};
use crate::internal::domain::{
    EdgePredicate, ExtractedEdge, ExtractedNode, FileExtraction, NodeKind,
};

const SECTION_EXTENSION: &str = "md-section";

pub struct MarkdownExtractor;

struct OpenSection {
    level: usize,
    fqn: String,
    name: String,
    start_line: i64,
    body: String,
}

impl Extractor for MarkdownExtractor {
    fn name(&self) -> &'static str {
        "markdown"
    }

    fn handles(&self, file: &FileContext<'_>) -> bool {
        matches!(file.extension, "md" | "markdown" | "mdx")
    }

    fn extract(&self, file: &FileContext<'_>) -> FileExtraction {
        let mut out = FileExtraction::default();
        let mut open: Vec<OpenSection> = Vec::new();
        let mut in_fence = false;
        let mut used_slugs: Vec<String> = Vec::new();

        let lines: Vec<&str> = file.content.lines().collect();

        for (index, raw) in lines.iter().enumerate() {
            let line_no = i64::try_from(index + 1).unwrap_or(i64::MAX);
            let trimmed = raw.trim_start();

            if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
                in_fence = !in_fence;
            }
            if in_fence {
                append_body(&mut open, raw);
                continue;
            }

            for target in links_in(raw) {
                let source = open
                    .last()
                    .map_or_else(|| file.relative_path.to_owned(), |s| s.fqn.clone());
                if let Some(dst) = resolve_relative(file.relative_path, &target) {
                    out.edges.push(ExtractedEdge {
                        src_fqn: source,
                        dst_fqn: dst,
                        predicate: EdgePredicate::Links,
                        file_path: file.relative_path.to_owned(),
                        line: line_no,
                    });
                }
            }

            let Some((level, heading)) = heading_of(trimmed) else {
                append_body(&mut open, raw);
                continue;
            };

            // A heading at this level or above closes everything open
            // below it.
            while open.last().is_some_and(|s| s.level >= level) {
                close_section(&mut open, &mut out, file, line_no - 1);
            }

            let slug = unique_slug(&slugify(&heading), &mut used_slugs);
            let fqn = format!("{}#{slug}", file.relative_path);

            // The containing node is the section above, or the file
            // itself when this is a top-level heading.
            let parent = open
                .last()
                .map_or_else(|| file.relative_path.to_owned(), |s| s.fqn.clone());
            out.edges.push(ExtractedEdge {
                src_fqn: parent,
                dst_fqn: fqn.clone(),
                predicate: EdgePredicate::Contains,
                file_path: file.relative_path.to_owned(),
                line: line_no,
            });

            open.push(OpenSection {
                level,
                fqn,
                name: heading,
                start_line: line_no,
                body: String::new(),
            });
        }

        let last_line = i64::try_from(lines.len().max(1)).unwrap_or(i64::MAX);
        while !open.is_empty() {
            close_section(&mut open, &mut out, file, last_line);
        }

        out
    }
}

fn append_body(open: &mut [OpenSection], line: &str) {
    if let Some(section) = open.last_mut() {
        section.body.push_str(line);
        section.body.push('\n');
    }
}

fn close_section(
    open: &mut Vec<OpenSection>,
    out: &mut FileExtraction,
    file: &FileContext<'_>,
    end_line: i64,
) {
    let Some(section) = open.pop() else {
        return;
    };
    let excerpt = section.body.trim().chars().take(300).collect::<String>();

    out.nodes.push(ExtractedNode {
        fqn: section.fqn,
        name: section.name.clone(),
        kind: NodeKind::Section,
        extension: SECTION_EXTENSION.to_owned(),
        file_path: file.relative_path.to_owned(),
        start_line: section.start_line,
        end_line: end_line.max(section.start_line),
        excerpt: (!excerpt.is_empty()).then_some(excerpt),
        content_hash: file.content_hash.to_owned(),
        search_text: format!("{}\n{}", section.name, section.body),
    });
}

/// `## Title` → `(2, "Title")`. Requires the space: `#hashtag` is not
/// a heading.
fn heading_of(trimmed: &str) -> Option<(usize, String)> {
    if !trimmed.starts_with('#') {
        return None;
    }
    let level = trimmed.chars().take_while(|c| *c == '#').count();
    if !(1..=6).contains(&level) {
        return None;
    }
    let rest = &trimmed[level..];
    if !rest.starts_with(' ') {
        return None;
    }
    let text = rest.trim().trim_end_matches('#').trim();
    (!text.is_empty()).then(|| (level, text.to_owned()))
}

/// The target of every markdown inline link on a line.
/// Deliberately not a regex: nested brackets in link text are common
/// enough in documentation that a naive pattern gets them wrong.
fn links_in(line: &str) -> Vec<String> {
    let bytes: Vec<char> = line.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] != '[' {
            i += 1;
            continue;
        }
        let mut depth = 0;
        let mut close = None;
        for (j, ch) in bytes.iter().enumerate().skip(i) {
            match ch {
                '[' => depth += 1,
                ']' => {
                    depth -= 1;
                    if depth == 0 {
                        close = Some(j);
                        break;
                    }
                }
                _ => {}
            }
        }
        let Some(close) = close else { break };
        if bytes.get(close + 1) != Some(&'(') {
            i = close + 1;
            continue;
        }
        let Some(end) = bytes.iter().skip(close + 2).position(|c| *c == ')') else {
            break;
        };
        let target: String = bytes[close + 2..close + 2 + end].iter().collect();
        let target = target.split_whitespace().next().unwrap_or("").to_owned();
        if !target.is_empty() {
            out.push(target);
        }
        i = close + 2 + end;
    }

    out
}

/// Resolves a link target against the linking file, returning `None`
/// for anything that does not name a path inside the project.
///
/// External URLs and bare anchors are dropped rather than stored
/// unresolved: they are not edges to a node that might appear later,
/// they are edges to something the graph will never contain.
fn resolve_relative(from_file: &str, target: &str) -> Option<String> {
    if target.starts_with("http://")
        || target.starts_with("https://")
        || target.starts_with("mailto:")
        || target.starts_with('#')
    {
        return None;
    }

    let (path_part, anchor) = match target.split_once('#') {
        Some((p, a)) => (p, Some(a)),
        None => (target, None),
    };
    if path_part.is_empty() {
        return None;
    }

    let base: Vec<&str> = from_file.split('/').collect();
    let mut stack: Vec<String> = base[..base.len().saturating_sub(1)]
        .iter()
        .map(|s| (*s).to_owned())
        .collect();

    for part in path_part.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                stack.pop();
            }
            other => stack.push(other.to_owned()),
        }
    }

    let resolved = stack.join("/");
    if resolved.is_empty() {
        return None;
    }
    Some(match anchor {
        Some(a) => format!("{resolved}#{a}"),
        None => resolved,
    })
}

fn slugify(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut last_dash = true;
    for ch in text.chars() {
        if ch.is_alphanumeric() {
            out.extend(ch.to_lowercase());
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    out.trim_matches('-').to_owned()
}

/// Two headings with the same text in one file would otherwise share
/// an identity, and the second would silently overwrite the first.
fn unique_slug(base: &str, used: &mut Vec<String>) -> String {
    let base = if base.is_empty() { "section" } else { base };
    if !used.contains(&base.to_owned()) {
        used.push(base.to_owned());
        return base.to_owned();
    }
    let mut n = 2;
    loop {
        let candidate = format!("{base}-{n}");
        if !used.contains(&candidate) {
            used.push(candidate.clone());
            return candidate;
        }
        n += 1;
    }
}
