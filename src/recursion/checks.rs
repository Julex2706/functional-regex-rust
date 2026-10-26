//! Detection of recursive references that are still undefined.
//!
//! A regex that (transitively) depends on an undefined reference can neither
//! be built nor matched. `build` and `full_match` call
//! `Regex::ensure_defined`, which walks the pattern tree with
//! `PatternNode::has_undefined`.

use std::collections::HashSet;
use std::sync::Arc;

use super::cell::RecursiveCell;
use crate::built::Regex;
use crate::error::BuildError;
use crate::pattern::PatternNode;

impl Regex {
    /// Fails if this regex depends on a recursive reference that has not
    /// been defined yet.
    pub(crate) fn ensure_defined(&self) -> Result<(), BuildError> {
        if self.root.has_undefined(&mut HashSet::new()) {
            Err(BuildError::UndefinedRecursion)
        } else {
            Ok(())
        }
    }
}

impl PatternNode {
    /// Returns true if this pattern depends on a recursive reference that is
    /// not defined yet.
    ///
    /// Definitions are followed transitively; `seen` holds the cells already
    /// visited, which also breaks cycles.
    fn has_undefined(&self, seen: &mut HashSet<*const RecursiveCell>) -> bool {
        match self {
            PatternNode::Literal(_)
            | PatternNode::CharClass { .. }
            | PatternNode::Boundary { .. } => false,

            PatternNode::Sequence(nodes) => nodes.iter().any(|node| node.has_undefined(seen)),

            PatternNode::Alternation { branches, .. } => {
                branches.iter().any(|branch| branch.has_undefined(seen))
            }

            PatternNode::Capturing { inner, .. }
            | PatternNode::Embedded { inner, .. }
            | PatternNode::Repeat { inner, .. } => inner.has_undefined(seen),

            PatternNode::Recursive { cell, .. } => match cell.definition.get() {
                None => true,
                Some(definition) => {
                    seen.insert(Arc::as_ptr(cell)) && definition.root.has_undefined(seen)
                }
            },
        }
    }
}
