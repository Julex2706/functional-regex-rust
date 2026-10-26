//! Crate-private helpers shared by every builder method.
//!
//! These functions implement the mechanics (state transitions, error
//! bookkeeping, nesting, and the closure-based sub-builders) so that the
//! public methods in `atoms`, `groups`, `quantifiers` and `shortcuts` stay
//! small. `finish` is the hand-off point to the built phase.

use std::marker::PhantomData;

use super::alternatives::AlternativesBuilder;
use super::char_set::CharSetBuilder;
use super::regex_builder::RegexBuilder;
use super::state::{AppendOnly, BuilderState, Quantifiable};
use crate::built::ScopeLayout;
use crate::error::BuildError;
use crate::pattern::{CaptureSlot, PatternNode};

impl<S: BuilderState> RegexBuilder<S> {
    /// Converts the current builder into a different state type while preserving
    /// all assembled nodes and validation state.
    pub(crate) fn transition<T: BuilderState>(self) -> RegexBuilder<T> {
        RegexBuilder {
            nodes: self.nodes,
            error: self.error,
            _state: PhantomData,
        }
    }

    /// Stores the first error encountered while building the regex.
    pub(crate) fn note_error(&mut self, error: Option<BuildError>) {
        self.error = self.error.take().or(error);
    }

    /// Adds a pattern node and advances the builder into the quantifiable state.
    pub(crate) fn append_node(mut self, node: PatternNode) -> RegexBuilder<Quantifiable> {
        self.nodes.push(node);
        self.transition()
    }

    /// Builds a nested pattern, preserving the outer builder state while wrapping
    /// the inner result in a given pattern structure.
    pub(crate) fn append_nested<T: BuilderState>(
        self,
        build_inner: impl FnOnce(RegexBuilder<AppendOnly>) -> RegexBuilder<T>,
        wrap_inner: impl FnOnce(PatternNode) -> PatternNode,
    ) -> RegexBuilder<Quantifiable> {
        let inner = build_inner(RegexBuilder::new());
        let mut next = self.append_node(wrap_inner(PatternNode::Sequence(inner.nodes)));
        next.note_error(inner.error);
        next
    }

    /// Builds a character-set based pattern and tracks any validation error.
    pub(crate) fn append_set(
        self,
        build_set: impl FnOnce(CharSetBuilder) -> CharSetBuilder,
        negated: bool,
    ) -> RegexBuilder<Quantifiable> {
        let set = build_set(CharSetBuilder {
            items: vec![],
            error: None,
        });

        let mut next = self.append_node(PatternNode::CharClass {
            items: set.items,
            negated,
        });

        next.note_error(set.error);
        next
    }

    /// Builds a border pattern and converts the builder back to append-only mode.
    pub(crate) fn append_border(
        self,
        build_set: impl FnOnce(CharSetBuilder) -> CharSetBuilder,
        negated: bool,
    ) -> RegexBuilder<AppendOnly> {
        let set = build_set(CharSetBuilder {
            items: vec![],
            error: None,
        });

        let mut next = self.append_node(PatternNode::Boundary {
            items: set.items,
            negated,
        });

        next.note_error(set.error);
        next.transition()
    }

    /// Builds an alternative pattern in the given slot and tracks any
    /// validation error.
    pub(crate) fn append_alternatives(
        self,
        slot: CaptureSlot,
        build_alternatives: impl FnOnce(AlternativesBuilder) -> AlternativesBuilder,
    ) -> RegexBuilder<Quantifiable> {
        let alternatives = build_alternatives(AlternativesBuilder {
            branches: vec![],
            error: None,
        });

        let has_no_branches = alternatives.branches.is_empty();

        let mut next = self.append_node(PatternNode::Alternation {
            slot,
            branches: alternatives.branches,
            branch_layouts: vec![], // placeholder, filled in `build`
        });

        next.note_error(alternatives.error);

        if has_no_branches {
            next.note_error(Some(BuildError::EmptyAlternative));
        }

        next
    }

    /// Reports any recorded error, then validates duplicate names and assigns
    /// indexes for the top-level scope. Shared by `build` and `define`.
    pub(crate) fn finish(self) -> Result<(PatternNode, ScopeLayout), BuildError> {
        if let Some(error) = self.error {
            return Err(error);
        }

        PatternNode::Sequence(self.nodes).compile_scope()
    }
}
