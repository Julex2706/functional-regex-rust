//! [`CharSetBuilder`]: the closure-based builder for character classes.
//!
//! It is used by `any_char_of`, `any_except`, `border_of` and
//! `non_border_of`. Invalid ranges are recorded as an error rather than
//! panicking, and surface when the regex is built.

use crate::error::BuildError;
use crate::pattern::SetItem;

/// A builder for a character class.
pub struct CharSetBuilder {
    /// The raw character items in the set.
    pub(crate) items: Vec<SetItem>,

    /// The first error produced while building the character class.
    pub(crate) error: Option<BuildError>,
}

impl CharSetBuilder {
    /// Adds a literal string to the current set.
    pub fn literal(mut self, chars: impl Into<String>) -> CharSetBuilder {
        self.items
            .push(SetItem::Chars(chars.into().chars().collect()));
        self
    }

    /// Adds a range to the current set.
    ///
    /// If `from > to`, the builder records an invalid range error instead of
    /// silently producing nonsense.
    pub fn range(mut self, from: char, to: char) -> CharSetBuilder {
        if from > to {
            self.error = self.error.or(Some(BuildError::InvalidRange { from, to }));
        } else {
            self.items.push(SetItem::Range(from, to));
        }
        self
    }
}
