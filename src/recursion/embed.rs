//! Builder methods for recursive regexes: creating a reference and embedding
//! it into a pattern.

use std::sync::{Arc, OnceLock};

use super::cell::RecursiveCell;
use super::reference::RecursiveRegex;
use crate::builder::{AppendOnly, BuilderState, Quantifiable, RegexBuilder};
use crate::pattern::{CaptureSlot, PatternNode};

impl RegexBuilder<AppendOnly> {
    /// Creates an undefined recursive regex reference.
    ///
    /// The reference can be embedded with [`RegexBuilder::recurse`] before it
    /// is defined. Define it exactly once with [`RecursiveRegex::define`].
    /// Using a regex that depends on a still undefined reference to build or
    /// match is a [`BuildError::UndefinedRecursion`](crate::BuildError::UndefinedRecursion) error.
    pub fn recursive() -> RecursiveRegex {
        RecursiveRegex {
            cell: Arc::new(RecursiveCell {
                definition: OnceLock::new(),
            }),
        }
    }
}

impl<S: BuilderState> RegexBuilder<S> {
    /// Appends a reference to a recursive regex, which may still be undefined.
    ///
    /// The reference occupies the next free index in the current scope,
    /// sharing the index space with unnamed groups, alternatives and nested
    /// regexes. Its occurrences are always groups carrying the captures of the
    /// recursive regex's top-level scope.
    pub fn recurse(self, recursive: &RecursiveRegex) -> RegexBuilder<Quantifiable> {
        self.append_node(PatternNode::Recursive {
            slot: CaptureSlot::Index(0), // placeholder, assigned in `build`
            cell: recursive.cell.clone(),
        })
    }

    /// Appends a reference to a recursive regex under a name.
    ///
    /// The name is registered in the current scope's name namespace together
    /// with named captures, named alternatives and named nested regexes.
    pub fn named_recurse(
        self,
        name: impl Into<String>,
        recursive: &RecursiveRegex,
    ) -> RegexBuilder<Quantifiable> {
        self.append_node(PatternNode::Recursive {
            slot: CaptureSlot::Name(name.into()),
            cell: recursive.cell.clone(),
        })
    }
}
