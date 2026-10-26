//! The shared, write-once storage behind recursive references.
//!
//! Every [`RecursiveRegex`](crate::RecursiveRegex) and every `Recursive`
//! pattern node holds the same `Arc<RecursiveCell>`. The body is stored once,
//! by `define`, and looked up at match time. That indirection is what allows
//! a reference to be embedded before it is defined.

use std::fmt;
use std::sync::{Arc, OnceLock};

use crate::built::ScopeLayout;
use crate::pattern::PatternNode;

/// Write-once storage for the body of a recursive regex.
pub(crate) struct RecursiveCell {
    /// The body, set exactly once by `RecursiveRegex::define`.
    pub(crate) definition: OnceLock<RecursiveDefinition>,
}

/// The compiled body of a recursive regex.
pub(crate) struct RecursiveDefinition {
    /// The compiled pattern tree of the body.
    pub(crate) root: PatternNode,
    /// The layout of the body's top-level scope.
    pub(crate) layout: Arc<ScopeLayout>,
}

// Manual impl: the body may contain this very cell, so deriving `Debug`
// would recurse forever.
impl fmt::Debug for RecursiveCell {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("RecursiveCell")
            .field("defined", &self.definition.get().is_some())
            .finish()
    }
}
