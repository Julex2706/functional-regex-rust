//! Predefined character classes and borders, available in every builder
//! state.
//!
//! Each shortcut is a thin wrapper over `any_char_of`, `any_except`,
//! `border_of` or `non_border_of` with a fixed set. The shared "word
//! character" set is defined once on [`CharSetBuilder`].

use super::char_set::CharSetBuilder;
use super::regex_builder::RegexBuilder;
use super::state::{AppendOnly, BuilderState, Quantifiable};

impl CharSetBuilder {
    /// Builds a default word-character set using letters, digits, and underscore.
    fn word_chars(self) -> CharSetBuilder {
        self.range('a', 'z')
            .range('A', 'Z')
            .range('0', '9')
            .literal('_')
    }
}

impl<S: BuilderState> RegexBuilder<S> {
    /// Matches any character in the inclusive range from `from` to `to`.
    pub fn range(self, from: char, to: char) -> RegexBuilder<Quantifiable> {
        self.any_char_of(|set| set.range(from, to))
    }

    /// Matches any single character.
    pub fn any_char(self) -> RegexBuilder<Quantifiable> {
        self.any_except(|set| set)
    }

    /// Matches any decimal digit.
    pub fn any_digit(self) -> RegexBuilder<Quantifiable> {
        self.range('0', '9')
    }

    /// Matches any character that is not a decimal digit.
    pub fn any_non_digit(self) -> RegexBuilder<Quantifiable> {
        self.any_except(|set| set.range('0', '9'))
    }

    /// Matches any character considered part of a word.
    pub fn any_word(self) -> RegexBuilder<Quantifiable> {
        self.any_char_of(CharSetBuilder::word_chars)
    }

    /// Matches any character that is not part of a word.
    pub fn any_non_word(self) -> RegexBuilder<Quantifiable> {
        self.any_except(CharSetBuilder::word_chars)
    }

    /// Matches any whitespace character.
    pub fn any_whitespace(self) -> RegexBuilder<Quantifiable> {
        self.any_char_of(|set| set.literal(" \t\r\n\x0B\x0C"))
    }

    /// Matches any non-whitespace character.
    pub fn any_non_whitespace(self) -> RegexBuilder<Quantifiable> {
        self.any_except(|set| set.literal(" \t\r\n\x0B\x0C"))
    }

    /// Matches any hexadecimal digit.
    pub fn any_hex_digit(self) -> RegexBuilder<Quantifiable> {
        self.any_char_of(|set| set.range('0', '9').range('a', 'f').range('A', 'F'))
    }

    /// Matches any character that is not a hexadecimal digit.
    pub fn any_non_hex_digit(self) -> RegexBuilder<Quantifiable> {
        self.any_except(|set| set.range('0', '9').range('a', 'f').range('A', 'F'))
    }

    /// Matches any alphabetic character.
    pub fn any_letter(self) -> RegexBuilder<Quantifiable> {
        self.any_char_of(|set| set.range('a', 'z').range('A', 'Z'))
    }

    /// Matches any lowercase alphabetic character.
    pub fn any_lowercase(self) -> RegexBuilder<Quantifiable> {
        self.range('a', 'z')
    }

    /// Matches any uppercase alphabetic character.
    pub fn any_uppercase(self) -> RegexBuilder<Quantifiable> {
        self.range('A', 'Z')
    }

    /// Matches a word boundary.
    pub fn word_border(self) -> RegexBuilder<AppendOnly> {
        self.border_of(CharSetBuilder::word_chars)
    }

    /// Matches a non-word boundary.
    pub fn non_word_border(self) -> RegexBuilder<AppendOnly> {
        self.non_border_of(CharSetBuilder::word_chars)
    }
}
