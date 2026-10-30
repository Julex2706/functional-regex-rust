//! Repetition: the methods that exist only in the [`Quantifiable`] and
//! [`Quantified`] states.
//!
//! Quantifying is only possible right after an atom was added, and `lazy` is
//! only possible right after quantifying. That is enforced by the types, so
//! neither can fail at run time apart from the recorded
//! [`BuildError::InvalidQuantifier`].
//!
//! # Examples
//!
//! ```
//! use regex_builder::RegexBuilder;
//!
//! let bounded = RegexBuilder::new()
//!     .any_digit()
//!     .between(2, 4)
//!     .build()
//!     .unwrap();
//! assert!(bounded.full_match("42").unwrap().is_some());
//! assert!(bounded.full_match("4").unwrap().is_none());
//!
//! let greedy = RegexBuilder::new()
//!     .capture("first", |b| b.literal("a").zero_or_more())
//!     .capture("second", |b| b.literal("a").zero_or_more())
//!     .build()
//!     .unwrap();
//! let lazy = RegexBuilder::new()
//!     .capture("first", |b| b.literal("a").zero_or_more().lazy())
//!     .capture("second", |b| b.literal("a").zero_or_more())
//!     .build()
//!     .unwrap();
//! let greedy = greedy.full_match("aa").unwrap().unwrap();
//! let lazy = lazy.full_match("aa").unwrap().unwrap();
//! assert_eq!(greedy.with_name("first").unwrap()[0].matched_text(), "aa");
//! assert_eq!(lazy.with_name("first").unwrap()[0].matched_text(), "");
//! ```

use super::regex_builder::RegexBuilder;
use super::state::{AppendOnly, Quantifiable, Quantified};
use crate::error::BuildError;
use crate::pattern::PatternNode;

impl RegexBuilder<Quantifiable> {
    /// Applies a repetition range to the most recently added expression.
    ///
    /// The `min` value is the minimum number of repetitions, and `max` is the
    /// optional maximum. If `min > max`, an `InvalidQuantifier` error is stored.
    ///
    /// # Examples
    ///
    /// ```
    /// use regex_builder::RegexBuilder;
    ///
    /// let regex = RegexBuilder::new().literal("ha").quantify(2, Some(3)).build().unwrap();
    /// assert!(regex.full_match("haha").unwrap().is_some());
    /// assert!(regex.full_match("hahaha").unwrap().is_some());
    /// assert!(regex.full_match("ha").unwrap().is_none());
    /// ```
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
    ///
    /// # Examples
    ///
    /// ```
    /// use regex_builder::RegexBuilder;
    ///
    /// let sign = RegexBuilder::new().literal("-").optional().any_digit().build().unwrap();
    /// assert!(sign.full_match("7").unwrap().is_some());
    /// assert!(sign.full_match("-7").unwrap().is_some());
    /// ```
    pub fn optional(self) -> RegexBuilder<Quantified> {
        self.quantify(0, Some(1))
    }

    /// Matches zero or more repetitions of the previous expression.
    ///
    /// # Examples
    ///
    /// ```
    /// use regex_builder::RegexBuilder;
    ///
    /// let regex = RegexBuilder::new().literal("ha").zero_or_more().build().unwrap();
    /// assert!(regex.full_match("").unwrap().is_some());
    /// assert!(regex.full_match("hahaha").unwrap().is_some());
    /// ```
    pub fn zero_or_more(self) -> RegexBuilder<Quantified> {
        self.quantify(0, None)
    }

    /// Matches one or more repetitions of the previous expression.
    ///
    /// # Examples
    ///
    /// ```
    /// use regex_builder::RegexBuilder;
    ///
    /// let regex = RegexBuilder::new().any_digit().one_or_more().build().unwrap();
    /// assert!(regex.full_match("42").unwrap().is_some());
    /// assert!(regex.full_match("").unwrap().is_none());
    /// ```
    pub fn one_or_more(self) -> RegexBuilder<Quantified> {
        self.quantify(1, None)
    }

    /// Matches exactly `count` repetitions of the previous expression.
    ///
    /// # Examples
    ///
    /// ```
    /// use regex_builder::RegexBuilder;
    ///
    /// let year = RegexBuilder::new().any_digit().exactly(4).build().unwrap();
    /// assert!(year.full_match("2026").unwrap().is_some());
    /// assert!(year.full_match("26").unwrap().is_none());
    /// ```
    pub fn exactly(self, count: usize) -> RegexBuilder<Quantified> {
        self.quantify(count, Some(count))
    }

    /// Matches at least `count` repetitions of the previous expression.
    ///
    /// # Examples
    ///
    /// ```
    /// use regex_builder::RegexBuilder;
    ///
    /// let regex = RegexBuilder::new().literal("a").at_least(2).build().unwrap();
    /// assert!(regex.full_match("aa").unwrap().is_some());
    /// assert!(regex.full_match("aaaa").unwrap().is_some());
    /// ```
    pub fn at_least(self, count: usize) -> RegexBuilder<Quantified> {
        self.quantify(count, None)
    }

    /// Matches at most `count` repetitions of the previous expression.
    ///
    /// # Examples
    ///
    /// ```
    /// use regex_builder::RegexBuilder;
    ///
    /// let regex = RegexBuilder::new().literal("x").at_most(2).build().unwrap();
    /// assert!(regex.full_match("").unwrap().is_some());
    /// assert!(regex.full_match("xx").unwrap().is_some());
    /// assert!(regex.full_match("xxx").unwrap().is_none());
    /// ```
    pub fn at_most(self, count: usize) -> RegexBuilder<Quantified> {
        self.quantify(0, Some(count))
    }

    /// Matches between `min` and `max` repetitions of the previous expression.
    ///
    /// # Examples
    ///
    /// ```
    /// use regex_builder::RegexBuilder;
    ///
    /// let regex = RegexBuilder::new().any_digit().between(2, 4).build().unwrap();
    /// assert!(regex.full_match("12").unwrap().is_some());
    /// assert!(regex.full_match("12345").unwrap().is_none());
    /// ```
    pub fn between(self, min: usize, max: usize) -> RegexBuilder<Quantified> {
        self.quantify(min, Some(max))
    }
}

impl RegexBuilder<Quantified> {
    /// Makes the last quantified expression lazy instead of greedy.
    ///
    /// # Examples
    ///
    /// ```
    /// use regex_builder::RegexBuilder;
    ///
    /// let regex = RegexBuilder::new()
    ///     .capture("first", |b| b.literal("a").zero_or_more().lazy())
    ///     .capture("second", |b| b.literal("a").zero_or_more())
    ///     .build()
    ///     .unwrap();
    /// let captures = regex.full_match("aa").unwrap().unwrap();
    /// assert_eq!(captures.with_name("first").unwrap()[0].matched_text(), "");
    /// assert_eq!(captures.with_name("second").unwrap()[0].matched_text(), "aa");
    /// ```
    pub fn lazy(mut self) -> RegexBuilder<AppendOnly> {
        let Some(PatternNode::Repeat { lazy, .. }) = self.nodes.last_mut() else {
            unreachable!("Quantified state guarantees the last node to be of type Repeat")
        };

        *lazy = true;

        self.transition()
    }
}
