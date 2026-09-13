//! atlas-graph — Project Map: a content-centric index of a project.
//!
//! Nodes are files and markdown sections; edges are typed relations
//! between them. Nothing here is language-aware, which is what lets
//! it index a repository in any language the day it appears.
//!
//! The public surface is re-exported from [`api`]; consumers depend
//! only on that module.

pub mod api;

mod internal;
