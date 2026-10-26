//! Repetition: the methods that exist only in the [`Quantifiable`] and
//! [`Quantified`] states.
//!
//! Quantifying is only possible right after an atom was added, and `lazy` is
//! only possible right after quantifying. That is enforced by the types, so
//! neither can fail at run time apart from the recorded
//! [`BuildError::InvalidQuantifier`].

use super::regex_builder::RegexBuilder;
use super::state::{AppendOnly, Quantifiable, Quantified};
use crate::error::BuildError;
use crate::pattern::PatternNode;

impl RegexBuilder<Quantifiable> {
    /// Applies a repetition range to the most recently added expression.
    ///
    /// The `min` value is the minimum number of repetitions, and `max` is the
    /// optional maximum. If `min > max`, an `InvalidQuantifier` error is stored.
    pub fn quantify(mut self, min: usize, max: Option<usize>) -> RegexBuilder<Quantified> {
        self.note_error(
            max.filter(|max| min > *max)
                .map(|max| BuildError::InvalidQuantifier { min, max }),
        );

        let target = self
            .nodes
            .pop()
            .expect("Quantifiable state guarantees an expression to repeat");

        self.nodes.push(PatternNode::Repeat {
            inner: Box::new(target),
            min,
            max,
            lazy: false,
        });

        self.transition()
    }

    /// Matches zero or one repetition of the previous expression.
    pub fn optional(self) -> RegexBuilder<Quantified> {
        self.quantify(0, Some(1))
    }

    /// Matches zero or more repetitions of the previous expression.
    pub fn zero_or_more(self) -> RegexBuilder<Quantified> {
        self.quantify(0, None)
    }

    /// Matches one or more repetitions of the previous expression.
    pub fn one_or_more(self) -> RegexBuilder<Quantified> {
        self.quantify(1, None)
    }

    /// Matches exactly `count` repetitions of the previous expression.
    pub fn exactly(self, count: usize) -> RegexBuilder<Quantified> {
        self.quantify(count, Some(count))
    }

    /// Matches at least `count` repetitions of the previous expression.
    pub fn at_least(self, count: usize) -> RegexBuilder<Quantified> {
        self.quantify(count, None)
    }

    /// Matches at most `count` repetitions of the previous expression.
    pub fn at_most(self, count: usize) -> RegexBuilder<Quantified> {
        self.quantify(0, Some(count))
    }

    /// Matches between `min` and `max` repetitions of the previous expression.
    pub fn between(self, min: usize, max: usize) -> RegexBuilder<Quantified> {
        self.quantify(min, Some(max))
    }
}

impl RegexBuilder<Quantified> {
    /// Makes the last quantified expression lazy instead of greedy.
    pub fn lazy(mut self) -> RegexBuilder<AppendOnly> {
        let Some(PatternNode::Repeat { lazy, .. }) = self.nodes.last_mut() else {
            unreachable!("Quantified state guarantees the last node to be of type Repeat")
        };

        *lazy = true;

        self.transition()
    }
}
