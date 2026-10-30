//! Everything specific to recursive regexes.
//!
//! - `cell`: the write-once storage shared by all references to one regex.
//! - `reference`: the public [`RecursiveRegex`] and `define`.
//! - `embed`: the builder methods `recursive`, `recurse` and `named_recurse`.
//! - `checks`: detection of references that are still undefined.
//! - `seed`: the memo types used to evaluate left recursion.
//! - `wrapping`: recording one recursive occurrence per body match.
//!
//! The `Recursive` arm of the matching engine itself is part of
//! `PatternNode::match_at` in `matching::engine`, because a `match` arm
//! cannot be moved to another module unchanged.

mod cell;
mod checks;
mod embed;
mod reference;
mod seed;
mod wrapping;

pub(crate) use cell::{RecursiveCell, RecursiveDefinition};
pub use reference::RecursiveRegex;
pub(crate) use seed::{distinct_ends, RecursionMemo, SeedEntry};
pub(crate) use wrapping::wrap_recursive;
