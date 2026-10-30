//! [`CharSetBuilder`]: the closure-based builder for character classes.
//!
//! It is used by `any_char_of`, `any_except`, `border_of` and
//! `non_border_of`. Invalid ranges are recorded as an error rather than
//! panicking, and surface when the regex is built. An empty set matches no
//! character in a positive matcher and every character in a negated matcher.

use crate::error::BuildError;
use crate::pattern::SetItem;

/// A builder for a character class.
///
/// # Examples
///
/// ```
/// use regex_builder::RegexBuilder;
///
/// let identifier = RegexBuilder::new()
///     .any_char_of(|set| set.range('a', 'z').range('A', 'Z').literal("_"))
///     .build()
///     .unwrap();
/// assert!(identifier.full_match("a").unwrap().is_some());
/// assert!(identifier.full_match("_").unwrap().is_some());
/// assert!(identifier.full_match("7").unwrap().is_none());
/// ```
pub struct CharSetBuilder {
    /// The raw character items in the set.
    pub(crate) items: Vec<SetItem>,

    /// The first error produced while building the character class.
    pub(crate) error: Option<BuildError>,
}

impl CharSetBuilder {
    /// Adds a literal string to the current set.
    ///
    /// # Examples
    ///
    /// ```
    /// use regex_builder::RegexBuilder;
    ///
    /// let vowels = RegexBuilder::new()
    ///     .any_char_of(|set| set.literal("aeiou"))
    ///     .build()
    ///     .unwrap();
    /// assert!(vowels.full_match("e").unwrap().is_some());
    /// assert!(vowels.full_match("b").unwrap().is_none());
    /// ```
    pub fn literal(mut self, chars: impl Into<String>) -> CharSetBuilder {
        self.items
            .push(SetItem::Chars(chars.into().chars().collect()));
        self
    }

    /// Adds an inclusive range of Unicode scalar values to the current set.
    ///
    /// If `from > to`, the builder records an invalid range error instead of
    /// silently producing nonsense.
    ///
    /// # Examples
    ///
    /// ```
    /// use regex_builder::RegexBuilder;
    ///
    /// let lowercase = RegexBuilder::new()
    ///     .any_char_of(|set| set.range('a', 'z'))
    ///     .build()
    ///     .unwrap();
    /// assert!(lowercase.full_match("m").unwrap().is_some());
    /// assert!(lowercase.full_match("M").unwrap().is_none());
    /// ```
    pub fn range(mut self, from: char, to: char) -> CharSetBuilder {
        if from > to {
            self.error = self.error.or(Some(BuildError::InvalidRange { from, to }));
        } else {
            self.items.push(SetItem::Range(from, to));
        }
        self
    }
}
