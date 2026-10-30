//! The compiled [`Regex`] type.
//!
//! This module defines only the data. Construction is in `build` (and
//! `RecursiveRegex::define`), matching is in `matching::full_match`, and the
//! recursion check is in `recursion::checks`.

use std::sync::Arc;

use super::layout::ScopeLayout;
use crate::pattern::PatternNode;

/// A fully compiled regex pattern with its scope layout.
///
/// Cloning a regex clones its pattern and layout. Recursive references inside
/// the pattern continue to share their write-once definitions.
#[derive(Debug, Clone)]
pub struct Regex {
    /// The compiled pattern tree, with all indexes assigned.
    pub(crate) root: PatternNode,

    /// Describes the capture locations of the top-level scope.
    pub(crate) layout: Arc<ScopeLayout>,
}
