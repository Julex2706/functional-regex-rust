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
    /// Also returns [`BuildError::UndefinedRecursion`] if the pattern depends
    /// on a recursive reference that has not been defined yet.
    pub fn build(self) -> Result<Regex, BuildError> {
        let (root, layout) = self.finish()?;

        let regex = Regex {
            root,
            layout: Arc::new(layout),
        };

        regex.ensure_defined()?;

        Ok(regex)
    }
}
