//! `RegexBuilder::build`: the public hand-off from the builder phase to the
//! built phase.

use std::sync::Arc;

use super::Regex;
use crate::builder::{BuilderState, RegexBuilder};
use crate::error::BuildError;

impl<S: BuilderState> RegexBuilder<S> {
    /// Finalizes the builder and turns it into a compiled regex.
    ///
    /// Reports any error recorded while building, then validates duplicate
    /// names and assigns indexes scope by scope: the top level first, then each
    /// group's, capture's and alternative branch's inner scope independently.
    ///
    /// # Errors
    ///
    /// Returns a recorded builder error or a scope-validation error, such as a
    /// duplicate name. An undefined recursive reference does not prevent
    /// building; it is reported by [`Regex::full_match`](crate::Regex::full_match)
    /// if it remains undefined when matching is attempted.
    pub fn build(self) -> Result<Regex, BuildError> {
        let (root, layout) = self.finish()?;

        let regex = Regex {
            root,
            layout: Arc::new(layout),
        };

        Ok(regex)
    }
}
