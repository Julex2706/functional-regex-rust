//! The [`RegexBuilder`] type and its constructor.
//!
//! This module defines only the data. Behaviour is spread over sibling
//! modules by topic: `internals` (private helpers), `atoms`, `groups`,
//! `quantifiers` and `shortcuts`.

use std::marker::PhantomData;

use super::state::{AppendOnly, BuilderState};
use crate::error::BuildError;
use crate::pattern::PatternNode;

/// A stateful builder for regex patterns.
#[derive(Debug)]
pub struct RegexBuilder<S: BuilderState> {
    /// The sequence of compiled pattern nodes currently assembled.
    pub(crate) nodes: Vec<PatternNode>,

    /// The first validation error encountered while building.
    pub(crate) error: Option<BuildError>,

    /// Phantom state marker used to enforce the builder state machine.
    pub(crate) _state: PhantomData<fn() -> S>,
}

impl RegexBuilder<AppendOnly> {
    /// Creates a new regex builder in the append-only state.
    pub fn new() -> RegexBuilder<AppendOnly> {
        RegexBuilder {
            nodes: vec![],
            error: None,
            _state: PhantomData,
        }
    }
}
