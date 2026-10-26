//! [`RecursiveRegex`]: the public, possibly still undefined, reference to a
//! regex that may refer to itself.

use std::sync::Arc;

use super::cell::{RecursiveCell, RecursiveDefinition};
use crate::builder::{BuilderState, RegexBuilder};
use crate::built::Regex;
use crate::error::BuildError;

/// A reference to a regex that may refer to itself.
///
/// Created with [`RegexBuilder::recursive`], embedded with
/// [`RegexBuilder::recurse`] or [`RegexBuilder::named_recurse`] (even before
/// it is defined) and finished exactly once with [`RecursiveRegex::define`].
///
/// An occurrence of a recursive reference is always a
/// [`CaptureOccurrence::Group`](crate::CaptureOccurrence::Group) whose nested
/// captures are those of the recursive regex's own top-level scope.
///
/// Left recursion (for example `expr = expr "+" digit | digit`) is supported:
/// the recursion is re-evaluated until it stops reaching new end positions.
#[derive(Debug)]
pub struct RecursiveRegex {
    /// The cell shared with every embedded reference to this regex.
    pub(crate) cell: Arc<RecursiveCell>,
}

impl RecursiveRegex {
    /// Defines the recursive regex and returns it as a [`Regex`].
    ///
    /// Consumes the reference, so it can only be defined once. The builder
    /// may embed this reference (or other recursive references) directly or
    /// indirectly. Other references may still be undefined here; using the
    /// result to build or match is an error until they are defined.
    ///
    /// Because this consumes the reference, build the `builder` first and
    /// then call `define`; the builder only borrows the reference while it
    /// is being constructed.
    ///
    /// # Errors
    ///
    /// Returns any error recorded by the builder, or a scope validation error
    /// such as [`BuildError::DuplicateName`].
    pub fn define<T: BuilderState>(
        self,
        builder: RegexBuilder<T>,
    ) -> Result<Regex, BuildError> {
        let (root, layout) = builder.finish()?;
        let layout = Arc::new(layout);

        // `self` is consumed, so the cell cannot have been defined before.
        let _ = self.cell.definition.set(RecursiveDefinition {
            root: root.clone(),
            layout: layout.clone(),
        });

        Ok(Regex { root, layout })
    }
}
