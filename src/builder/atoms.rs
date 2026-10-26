//! Builder methods that append single atoms: literal text, embedded regexes
//! and character sets (including borders).
//!
//! Atoms never open a capture scope of their own, except embedded regexes,
//! which occupy one capture location and keep their own scope isolated.

use super::char_set::CharSetBuilder;
use super::regex_builder::RegexBuilder;
use super::state::{AppendOnly, BuilderState, Quantifiable};
use crate::built::Regex;
use crate::pattern::{CaptureSlot, PatternNode};

impl<S: BuilderState> RegexBuilder<S> {
    /// Adds a literal text atom to the pattern.
    pub fn literal(self, text: impl Into<String>) -> RegexBuilder<Quantifiable> {
        self.append_node(PatternNode::Literal(text.into().chars().collect()))
    }

    /// Appends a reusable regex without consuming it.
    ///
    /// The nested regex occupies the next free index in the current scope,
    /// sharing the index space with unnamed groups and alternatives. Its own
    /// captures stay isolated inside its result and are reached through the
    /// occurrence.
    pub fn regex(self, regex: &Regex) -> RegexBuilder<Quantifiable> {
        self.append_node(PatternNode::Embedded {
            slot: CaptureSlot::Index(0), // placeholder, assigned in `build`
            inner: Box::new(regex.root.clone()),
            layout: regex.layout.clone(),
        })
    }

    /// Appends a reusable regex under a name.
    ///
    /// The name is registered in the current scope's name namespace together
    /// with named captures, named alternatives and other named nested regexes.
    pub fn named_regex(self, name: impl Into<String>, regex: &Regex) -> RegexBuilder<Quantifiable> {
        self.append_node(PatternNode::Embedded {
            slot: CaptureSlot::Name(name.into()),
            inner: Box::new(regex.root.clone()),
            layout: regex.layout.clone(),
        })
    }

    /// Matches any one character in the set produced by the closure.
    pub fn any_char_of(
        self,
        build_set: impl FnOnce(CharSetBuilder) -> CharSetBuilder,
    ) -> RegexBuilder<Quantifiable> {
        self.append_set(build_set, false)
    }

    /// Matches any one character not in the set produced by the closure.
    pub fn any_except(
        self,
        build_set: impl FnOnce(CharSetBuilder) -> CharSetBuilder,
    ) -> RegexBuilder<Quantifiable> {
        self.append_set(build_set, true)
    }

    /// Matches a border around characters in the set produced by the closure.
    pub fn border_of(
        self,
        build_set: impl FnOnce(CharSetBuilder) -> CharSetBuilder,
    ) -> RegexBuilder<AppendOnly> {
        self.append_border(build_set, false)
    }

    /// Matches a non-border position relative to the set produced by the closure.
    pub fn non_border_of(
        self,
        build_set: impl FnOnce(CharSetBuilder) -> CharSetBuilder,
    ) -> RegexBuilder<AppendOnly> {
        self.append_border(build_set, true)
    }
}
