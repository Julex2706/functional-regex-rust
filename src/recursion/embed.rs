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
    /// A regex that depends on a still undefined reference can be built, but
    /// attempting to match it returns
    /// [`BuildError::UndefinedRecursion`](crate::BuildError::UndefinedRecursion).
    ///
    /// # Examples
    ///
    /// ```
    /// use regex_builder::RegexBuilder;
    ///
    /// let parens = RegexBuilder::recursive();
    /// let body = RegexBuilder::new().or(|branches| {
    ///     branches
    ///         .branch(|b| b.literal("(").recurse(&parens).literal(")"))
    ///         .branch(|b| b.literal("x"))
    /// });
    /// let parens = parens.define(body).unwrap();
    /// assert!(parens.full_match("(x)").unwrap().is_some());
    /// ```
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
    ///
    /// # Examples
    ///
    /// ```
    /// use regex_builder::RegexBuilder;
    ///
    /// let item = RegexBuilder::recursive();
    /// let body = RegexBuilder::new().or(|branches| {
    ///     branches
    ///         .branch(|b| b.literal("(").recurse(&item).literal(")"))
    ///         .branch(|b| b.literal("x"))
    /// });
    /// let item = item.define(body).unwrap();
    /// assert!(item.full_match("((x))").unwrap().is_some());
    /// ```
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
    ///
    /// # Examples
    ///
    /// ```
    /// use regex_builder::RegexBuilder;
    ///
    /// let item = RegexBuilder::recursive();
    /// let body = RegexBuilder::new().or(|branches| {
    ///     branches
    ///         .branch(|b| b.literal("x"))
    ///         .branch(|b| b.literal("[").named_recurse("inner", &item).literal("]"))
    /// });
    /// let item = item.define(body).unwrap();
    /// let captures = item.full_match("[x]").unwrap().unwrap();
    /// let branch = captures.at_index(0).unwrap()[0].expect_alternative().1;
    /// assert_eq!(branch.with_name("inner").unwrap()[0].matched_text(), "x");
    /// ```
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
