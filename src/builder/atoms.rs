//! Builder methods that append single atoms: literal text, embedded regexes
//! and character sets (including borders).
//!
//! Atoms never open a capture scope of their own, except embedded regexes,
//! which occupy one capture location and keep their own scope isolated.
//!
//! # Examples
//!
//! Reusable regexes are embedded by reference; the original remains usable,
//! and the embedded regex's capture scope stays separate.
//!
//! ```
//! use regex_builder::RegexBuilder;
//!
//! let digits = RegexBuilder::new().any_digit().one_or_more().build().unwrap();
//! let pair = RegexBuilder::new()
//!     .regex(&digits)
//!     .literal("-")
//!     .named_regex("right", &digits)
//!     .build()
//!     .unwrap();
//!
//! let captures = pair.full_match("12-34").unwrap().unwrap();
//! assert_eq!(captures.at_index(0).unwrap()[0].matched_text(), "12");
//! assert_eq!(captures.with_name("right").unwrap()[0].matched_text(), "34");
//! assert!(digits.full_match("567").unwrap().is_some());
//! ```

use super::char_set::CharSetBuilder;
use super::regex_builder::RegexBuilder;
use super::state::{AppendOnly, BuilderState, Quantifiable};
use crate::built::Regex;
use crate::pattern::{CaptureSlot, PatternNode};

impl<S: BuilderState> RegexBuilder<S> {
    /// Adds a literal text atom to the pattern.
    ///
    /// # Examples
    ///
    /// ```
    /// use regex_builder::RegexBuilder;
    ///
    /// let greeting = RegexBuilder::new().literal("hello").build().unwrap();
    /// assert!(greeting.full_match("hello").unwrap().is_some());
    /// assert!(greeting.full_match("hello!").unwrap().is_none());
    /// ```
    pub fn literal(self, text: impl Into<String>) -> RegexBuilder<Quantifiable> {
        self.append_node(PatternNode::Literal(text.into().chars().collect()))
    }

    /// Appends a reusable regex without consuming it.
    ///
    /// The nested regex occupies the next free index in the current scope,
    /// sharing the index space with unnamed groups and alternatives. Its own
    /// captures stay isolated inside its result and are reached through the
    /// occurrence.
    ///
    /// # Examples
    ///
    /// ```
    /// use regex_builder::RegexBuilder;
    ///
    /// let digits = RegexBuilder::new().any_digit().one_or_more().build().unwrap();
    /// let parent = RegexBuilder::new().regex(&digits).build().unwrap();
    /// assert!(parent.full_match("123").unwrap().is_some());
    /// ```
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
    ///
    /// # Examples
    ///
    /// ```
    /// use regex_builder::RegexBuilder;
    ///
    /// let word = RegexBuilder::new()
    ///     .capture("text", |b| b.any_letter().one_or_more())
    ///     .build()
    ///     .unwrap();
    /// let parent = RegexBuilder::new().named_regex("word", &word).build().unwrap();
    /// let captures = parent.full_match("rust").unwrap().unwrap();
    /// assert_eq!(captures.with_name("word").unwrap()[0].matched_text(), "rust");
    /// ```
    pub fn named_regex(self, name: impl Into<String>, regex: &Regex) -> RegexBuilder<Quantifiable> {
        self.append_node(PatternNode::Embedded {
            slot: CaptureSlot::Name(name.into()),
            inner: Box::new(regex.root.clone()),
            layout: regex.layout.clone(),
        })
    }

    /// Matches any one character in the set produced by the closure.
    ///
    /// An empty set matches no character.
    ///
    /// # Examples
    ///
    /// ```
    /// use regex_builder::RegexBuilder;
    ///
    /// let vowel = RegexBuilder::new()
    ///     .any_char_of(|set| set.literal("aeiou"))
    ///     .build()
    ///     .unwrap();
    /// assert!(vowel.full_match("e").unwrap().is_some());
    /// assert!(vowel.full_match("x").unwrap().is_none());
    /// ```
    pub fn any_char_of(
        self,
        build_set: impl FnOnce(CharSetBuilder) -> CharSetBuilder,
    ) -> RegexBuilder<Quantifiable> {
        self.append_set(build_set, false)
    }

    /// Matches any one character not in the set produced by the closure.
    ///
    /// An empty set matches every character.
    ///
    /// # Examples
    ///
    /// ```
    /// use regex_builder::RegexBuilder;
    ///
    /// let not_a_vowel = RegexBuilder::new()
    ///     .any_except(|set| set.literal("aeiou"))
    ///     .build()
    ///     .unwrap();
    /// assert!(not_a_vowel.full_match("z").unwrap().is_some());
    /// assert!(not_a_vowel.full_match("a").unwrap().is_none());
    /// ```
    pub fn any_except(
        self,
        build_set: impl FnOnce(CharSetBuilder) -> CharSetBuilder,
    ) -> RegexBuilder<Quantifiable> {
        self.append_set(build_set, true)
    }

    /// Matches a border around characters in the set produced by the closure.
    ///
    /// # Examples
    ///
    /// ```
    /// use regex_builder::RegexBuilder;
    ///
    /// let consonant = RegexBuilder::new()
    ///     .literal("a")
    ///     .border_of(|set| set.literal("aeiou"))
    ///     .any_except(|set| set.literal("aeiou"))
    ///     .build()
    ///     .unwrap();
    /// assert!(consonant.full_match("ab").unwrap().is_some());
    /// assert!(consonant.full_match("aa").unwrap().is_none());
    /// ```
    pub fn border_of(
        self,
        build_set: impl FnOnce(CharSetBuilder) -> CharSetBuilder,
    ) -> RegexBuilder<AppendOnly> {
        self.append_border(build_set, false)
    }

    /// Matches a non-border position relative to the set produced by the closure.
    ///
    /// # Examples
    ///
    /// ```
    /// use regex_builder::RegexBuilder;
    ///
    /// let middle = RegexBuilder::new()
    ///     .any_word()
    ///     .non_border_of(|set| set.range('a', 'z').range('A', 'Z').literal("_"))
    ///     .literal("_")
    ///     .non_border_of(|set| set.range('a', 'z').range('A', 'Z').literal("_"))
    ///     .any_word()
    ///     .build()
    ///     .unwrap();
    /// assert!(middle.full_match("a_b").unwrap().is_some());
    /// assert!(middle.full_match("a_").unwrap().is_none());
    /// ```
    pub fn non_border_of(
        self,
        build_set: impl FnOnce(CharSetBuilder) -> CharSetBuilder,
    ) -> RegexBuilder<AppendOnly> {
        self.append_border(build_set, true)
    }
}
