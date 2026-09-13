//! How a project declares its own subdivisions and its architecture.
//!
//! Both live in one file in the project being described —
//! `atlas.layers.toml` at its root — rather than in Atlas's database.
//! A layer rule is a statement about the code, so it belongs beside
//! the code: versioned with it, reviewed in the same merge request,
//! and still true after Atlas's database is thrown away. A rule kept
//! anywhere else drifts from what it describes, and the drift is
//! silent.
//!
//! The file is optional. A project without one is still indexed,
//! searched and analysed; it simply has no layer findings, because
//! nothing has said what its layers are. **Atlas never infers layers
//! from directory names.** A guessed architecture reported as a
//! violation is worse than no finding at all.
//!
//! ```toml
//! [[module]]
//! name = "auth"
//! path = "features/auth"
//!
//! [[layer]]
//! name = "domain"
//! paths = ["**/internal/domain/**"]
//!
//! [[layer]]
//! name = "application"
//! paths = ["**/internal/application/**"]
//! depends_on = ["domain"]
//! ```
//!
//! A layer with no `depends_on` may depend on nothing. That is the
//! useful default for a domain layer, which is the one most worth
//! protecting, and it means the strictest reading is what you get for
//! not writing anything.

use serde::{Deserialize, Serialize};

/// The name a project's declaration file must have, at its root.
pub const LAYERS_FILE: &str = "atlas.layers.toml";

/// A curated subdivision of a project — what the scope picker offers,
/// and what a finding can be filtered to.
///
/// Curated rather than derived: every directory is a candidate
/// subdivision, and a list of all of them is not a subdivision, it is
/// a directory listing.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ModuleDecl {
    pub name: String,
    /// Path relative to the project root, without a trailing slash.
    pub path: String,
    #[serde(default)]
    pub description: Option<String>,
}

/// One architectural layer, and what it is allowed to depend on.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct LayerDecl {
    pub name: String,
    /// Glob patterns, relative to the project root, matching the files
    /// in this layer.
    pub paths: Vec<String>,
    /// Names of the layers this one may import from. Omitted means it
    /// may import from none.
    #[serde(default)]
    pub depends_on: Vec<String>,
}

/// A parsed and validated `atlas.layers.toml`.
#[derive(Clone, Debug, Deserialize, Serialize, Default, PartialEq, Eq)]
pub struct LayersFile {
    #[serde(default, rename = "module")]
    pub modules: Vec<ModuleDecl>,
    #[serde(default, rename = "layer")]
    pub layers: Vec<LayerDecl>,
}

/// Why a declaration file was rejected.
///
/// Rejected loudly and in full rather than repaired quietly: this file
/// is the basis for findings reported at error severity, and a
/// declaration that does not say what its author thought it said
/// produces confident wrong answers. Every variant here names the
/// offending declaration, because "invalid layers file" sends someone
/// reading a file they have already read.
#[derive(Clone, Debug, thiserror::Error, PartialEq, Eq)]
pub enum LayersError {
    #[error("{LAYERS_FILE} is not valid TOML: {0}")]
    Malformed(String),

    #[error("layer {0:?} is declared more than once")]
    DuplicateLayer(String),

    #[error("module {0:?} is declared more than once")]
    DuplicateModule(String),

    #[error("layer {0:?} declares no paths, so nothing can ever be in it")]
    LayerWithoutPaths(String),

    #[error("layer {layer:?} may depend on {missing:?}, which is not a declared layer")]
    UnknownDependency { layer: String, missing: String },

    #[error("layer {0:?} declares itself as one of its own dependencies")]
    SelfDependency(String),

    #[error("a layer or module was declared with an empty name")]
    EmptyName,

    #[error("layer {layer:?} has the invalid path pattern {pattern:?}: {reason}")]
    InvalidPattern {
        layer: String,
        pattern: String,
        reason: String,
    },
}

impl LayersFile {
    /// Parses and validates a declaration file.
    ///
    /// # Errors
    /// [`LayersError`] naming the specific declaration at fault.
    pub fn parse(source: &str) -> Result<Self, LayersError> {
        let file: Self =
            toml::from_str(source).map_err(|e| LayersError::Malformed(e.to_string()))?;
        file.validate()?;
        Ok(file)
    }

    fn validate(&self) -> Result<(), LayersError> {
        let mut seen_modules = std::collections::HashSet::new();
        for module in &self.modules {
            if module.name.trim().is_empty() {
                return Err(LayersError::EmptyName);
            }
            if !seen_modules.insert(module.name.as_str()) {
                return Err(LayersError::DuplicateModule(module.name.clone()));
            }
        }

        let mut seen_layers = std::collections::HashSet::new();
        for layer in &self.layers {
            if layer.name.trim().is_empty() {
                return Err(LayersError::EmptyName);
            }
            if !seen_layers.insert(layer.name.as_str()) {
                return Err(LayersError::DuplicateLayer(layer.name.clone()));
            }
            if layer.paths.is_empty() {
                return Err(LayersError::LayerWithoutPaths(layer.name.clone()));
            }
        }

        // Checked after every name is known, so a dependency on a layer
        // declared further down the file is not reported as missing.
        for layer in &self.layers {
            for dependency in &layer.depends_on {
                if dependency == &layer.name {
                    return Err(LayersError::SelfDependency(layer.name.clone()));
                }
                if !seen_layers.contains(dependency.as_str()) {
                    return Err(LayersError::UnknownDependency {
                        layer: layer.name.clone(),
                        missing: dependency.clone(),
                    });
                }
            }
        }

        Ok(())
    }

    /// Whether this declares anything worth analysing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.modules.is_empty() && self.layers.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_full_declaration_parses() {
        let file = LayersFile::parse(
            r#"
            [[module]]
            name = "auth"
            path = "features/auth"

            [[layer]]
            name = "domain"
            paths = ["**/internal/domain/**"]

            [[layer]]
            name = "application"
            paths = ["**/internal/application/**"]
            depends_on = ["domain"]
            "#,
        )
        .unwrap();

        assert_eq!(file.modules.len(), 1);
        assert_eq!(file.layers.len(), 2);
        assert_eq!(file.layers[1].depends_on, vec!["domain"]);
        // Omitting depends_on means depending on nothing, which is the
        // strictest reading and the right default for a domain layer.
        assert!(file.layers[0].depends_on.is_empty());
    }

    #[test]
    fn an_empty_file_is_valid_and_declares_nothing() {
        assert!(LayersFile::parse("").unwrap().is_empty());
    }

    #[test]
    fn a_dependency_declared_later_in_the_file_still_resolves() {
        // Validation that checked dependencies while walking would
        // reject this, and the ordering is not meaningful.
        let file = LayersFile::parse(
            r#"
            [[layer]]
            name = "application"
            paths = ["a/**"]
            depends_on = ["domain"]

            [[layer]]
            name = "domain"
            paths = ["d/**"]
            "#,
        );
        assert!(file.is_ok(), "{file:?}");
    }

    #[test]
    fn a_dependency_on_an_undeclared_layer_is_rejected_by_name() {
        let err = LayersFile::parse(
            r#"
            [[layer]]
            name = "application"
            paths = ["a/**"]
            depends_on = ["persistence"]
            "#,
        )
        .unwrap_err();

        assert_eq!(
            err,
            LayersError::UnknownDependency {
                layer: "application".to_owned(),
                missing: "persistence".to_owned(),
            }
        );
    }

    #[test]
    fn a_layer_depending_on_itself_is_rejected() {
        let err = LayersFile::parse(
            r#"
            [[layer]]
            name = "domain"
            paths = ["d/**"]
            depends_on = ["domain"]
            "#,
        )
        .unwrap_err();
        assert_eq!(err, LayersError::SelfDependency("domain".to_owned()));
    }

    #[test]
    fn a_layer_with_no_paths_is_rejected() {
        // Nothing can ever be in it, so every finding about it would be
        // silence — which reads as compliance.
        let err = LayersFile::parse(
            r#"
            [[layer]]
            name = "domain"
            paths = []
            "#,
        )
        .unwrap_err();
        assert_eq!(err, LayersError::LayerWithoutPaths("domain".to_owned()));
    }

    #[test]
    fn a_duplicate_layer_is_rejected() {
        let err = LayersFile::parse(
            r#"
            [[layer]]
            name = "domain"
            paths = ["a/**"]

            [[layer]]
            name = "domain"
            paths = ["b/**"]
            "#,
        )
        .unwrap_err();
        assert_eq!(err, LayersError::DuplicateLayer("domain".to_owned()));
    }

    #[test]
    fn a_duplicate_module_is_rejected() {
        let err = LayersFile::parse(
            r#"
            [[module]]
            name = "auth"
            path = "features/auth"

            [[module]]
            name = "auth"
            path = "features/authz"
            "#,
        )
        .unwrap_err();
        assert_eq!(err, LayersError::DuplicateModule("auth".to_owned()));
    }

    #[test]
    fn malformed_toml_names_the_problem_rather_than_the_file() {
        let err = LayersFile::parse("[[layer]\nname = ").unwrap_err();
        assert!(
            matches!(err, LayersError::Malformed(ref m) if !m.is_empty()),
            "{err:?}"
        );
    }
}
