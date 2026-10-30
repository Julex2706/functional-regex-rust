//! Builder methods that open capture scopes: groups, named captures and
//! alternatives.
//!
//! Groups and captures differ only in how they are addressed (index versus
//! name). Alternatives additionally record which branch was taken. Each of
//! them is a capture location, and everything declared inside is a nested
//! scope with its own indexes and names.

use std::sync::Arc;

use super::alternatives::{add_literal_branches, AlternativesBuilder};
use super::regex_builder::RegexBuilder;
use super::state::{AppendOnly, BuilderState, Quantifiable};
use crate::pattern::{CaptureSlot, PatternNode};

impl<S: BuilderState> RegexBuilder<S> {
    /// Creates an unnamed group.
    ///
    /// The group takes the next free index in the current scope, sharing the
    /// index space with unnamed alternatives and nested regexes. Everything
    /// declared inside the closure forms a nested scope with its own indexes
    /// and names. Repeating the group records one occurrence per iteration,
    /// each with its own nested captures.
    ///
    /// # Examples
    ///
    /// ```
    /// use regex_builder::RegexBuilder;
    ///
    /// let regex = RegexBuilder::new()
    ///     .group(|b| b.literal("(").literal("ok").literal(")"))
    ///     .build()
    ///     .unwrap();
    /// let captures = regex.full_match("(ok)").unwrap().unwrap();
    /// assert_eq!(captures.at_index(0).unwrap()[0].matched_text(), "(ok)");
    /// ```
    pub fn group<T: BuilderState>(
        self,
        build_inner: impl FnOnce(RegexBuilder<AppendOnly>) -> RegexBuilder<T>,
    ) -> RegexBuilder<Quantifiable> {
        self.append_nested(build_inner, |inner| PatternNode::Capturing {
            slot: CaptureSlot::Index(0), // placeholder, assigned in `build`
            inner: Box::new(inner),
            layout: Arc::default(), // placeholder, filled in `build`
        })
    }

    /// Creates a named capture.
    ///
    /// Behaves like [`RegexBuilder::group`], but the location is addressed by
    /// name in the shared name namespace of the current scope.
    ///
    /// # Examples
    ///
    /// ```
    /// use regex_builder::RegexBuilder;
    ///
    /// let regex = RegexBuilder::new()
    ///     .capture("year", |year| year.any_digit().exactly(4))
    ///     .build()
    ///     .unwrap();
    /// let captures = regex.full_match("2026").unwrap().unwrap();
    ///
    /// assert_eq!(
    ///     captures.with_name("year").unwrap()[0].matched_text(),
    ///     "2026"
    /// );
    /// ```
    pub fn capture<T: BuilderState>(
        self,
        name: impl Into<String>,
        build_inner: impl FnOnce(RegexBuilder<AppendOnly>) -> RegexBuilder<T>,
    ) -> RegexBuilder<Quantifiable> {
        self.append_nested(build_inner, |inner| PatternNode::Capturing {
            slot: CaptureSlot::Name(name.into()),
            inner: Box::new(inner),
            layout: Arc::default(), // placeholder, filled in `build`
        })
    }

    /// Creates an unnamed alternative with one or more branches.
    ///
    /// The alternative takes the next free index in the current scope, sharing
    /// the index space with unnamed groups and nested regexes. Each
    /// occurrence records which branch was taken (zero-based), the full text
    /// it consumed and the captures of that branch.
    ///
    /// # Examples
    ///
    /// ```
    /// use regex_builder::RegexBuilder;
    ///
    /// let regex = RegexBuilder::new()
    ///     .or(|branches| {
    ///         branches
    ///             .branch(|b| b.literal("yes"))
    ///             .branch(|b| b.literal("no"))
    ///     })
    ///     .build()
    ///     .unwrap();
    ///
    /// let captures = regex.full_match("no").unwrap().unwrap();
    /// assert_eq!(captures.at_index(0).unwrap()[0].taken_branch(), Some(1));
    /// ```
    pub fn or(
        self,
        build_alternatives: impl FnOnce(AlternativesBuilder) -> AlternativesBuilder,
    ) -> RegexBuilder<Quantifiable> {
        self.append_alternatives(CaptureSlot::Index(0), build_alternatives) // placeholder, assigned in `build`
    }

    /// Creates a named alternative with one or more branches.
    ///
    /// Behaves like [`RegexBuilder::or`], but the location is addressed by
    /// name in the shared name namespace of the current scope.
    ///
    /// # Examples
    ///
    /// ```
    /// use regex_builder::RegexBuilder;
    ///
    /// let regex = RegexBuilder::new()
    ///     .or_named("answer", |branches| {
    ///         branches.branch(|b| b.literal("yes")).branch(|b| b.literal("no"))
    ///     })
    ///     .build()
    ///     .unwrap();
    /// let captures = regex.full_match("no").unwrap().unwrap();
    /// assert_eq!(captures.with_name("answer").unwrap()[0].taken_branch(), Some(1));
    /// ```
    pub fn or_named(
        self,
        name: impl Into<String>,
        build_alternatives: impl FnOnce(AlternativesBuilder) -> AlternativesBuilder,
    ) -> RegexBuilder<Quantifiable> {
        self.append_alternatives(CaptureSlot::Name(name.into()), build_alternatives)
    }

    /// Matches any one of the provided string options.
    ///
    /// Each option is one branch, numbered in the order given, so the result
    /// reports which option matched.
    ///
    /// # Examples
    ///
    /// ```
    /// use regex_builder::RegexBuilder;
    ///
    /// let regex = RegexBuilder::new().any_of(vec!["red", "blue"]).build().unwrap();
    /// let captures = regex.full_match("blue").unwrap().unwrap();
    /// assert_eq!(captures.at_index(0).unwrap()[0].taken_branch(), Some(1));
    /// ```
    pub fn any_of<T: Into<String>>(self, options: Vec<T>) -> RegexBuilder<Quantifiable> {
        self.or(|builder| add_literal_branches(builder, options))
    }

    /// Matches any one of the provided string options, under a name.
    ///
    /// Behaves like [`RegexBuilder::any_of`], but the location is addressed by
    /// name in the shared name namespace of the current scope.
    ///
    /// # Examples
    ///
    /// ```
    /// use regex_builder::RegexBuilder;
    ///
    /// let regex = RegexBuilder::new()
    ///     .any_of_named("color", vec!["red", "blue"])
    ///     .build()
    ///     .unwrap();
    /// let captures = regex.full_match("red").unwrap().unwrap();
    /// assert_eq!(captures.with_name("color").unwrap()[0].taken_branch(), Some(0));
    /// ```
    pub fn any_of_named<T: Into<String>>(
        self,
        name: impl Into<String>,
        options: Vec<T>,
    ) -> RegexBuilder<Quantifiable> {
        self.or_named(name, |builder| add_literal_branches(builder, options))
    }
}
