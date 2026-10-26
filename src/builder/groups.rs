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
    pub fn any_of<T: Into<String>>(self, options: Vec<T>) -> RegexBuilder<Quantifiable> {
        self.or(|builder| add_literal_branches(builder, options))
    }

    /// Matches any one of the provided string options, under a name.
    ///
    /// Behaves like [`RegexBuilder::any_of`], but the location is addressed by
    /// name in the shared name namespace of the current scope.
    pub fn any_of_named<T: Into<String>>(
        self,
        name: impl Into<String>,
        options: Vec<T>,
    ) -> RegexBuilder<Quantifiable> {
        self.or_named(name, |builder| add_literal_branches(builder, options))
    }
}
