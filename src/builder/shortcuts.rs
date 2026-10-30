//! Predefined character classes and borders, available in every builder
//! state.
//!
//! Each shortcut is a thin wrapper over `any_char_of`, `any_except`,
//! `border_of` or `non_border_of` with a fixed set. The shared "word
//! character" set is defined once on [`CharSetBuilder`]. These shortcuts are
//! ASCII-based: letters are `A-Z` and `a-z`, word characters additionally
//! include digits and `_`, and whitespace is space, tab, carriage return,
//! line feed, vertical tab, or form feed. Use `range` or a custom character
//! set when you need different character semantics.
//!
//! # Examples
//!
//! ```
//! use regex_builder::RegexBuilder;
//!
//! let code = RegexBuilder::new()
//!     .any_uppercase()
//!     .any_hex_digit()
//!     .exactly(2)
//!     .build()
//!     .unwrap();
//! assert!(code.full_match("A7f").unwrap().is_some());
//! assert!(code.full_match("a7f").unwrap().is_none());
//!
//! let boundary = RegexBuilder::new()
//!     .word_border()
//!     .literal("rust")
//!     .word_border()
//!     .build()
//!     .unwrap();
//! assert!(boundary.full_match("rust").unwrap().is_some());
//! assert!(boundary.full_match("rusty").unwrap().is_none());
//! ```

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
    ///
    /// # Examples
    ///
    /// ```
    /// use regex_builder::RegexBuilder;
    ///
    /// let lowercase = RegexBuilder::new().range('a', 'z').build().unwrap();
    /// assert!(lowercase.full_match("m").unwrap().is_some());
    /// assert!(lowercase.full_match("M").unwrap().is_none());
    /// ```
    pub fn range(self, from: char, to: char) -> RegexBuilder<Quantifiable> {
        self.any_char_of(|set| set.range(from, to))
    }

    /// Matches any single character.
    ///
    /// # Examples
    ///
    /// ```
    /// use regex_builder::RegexBuilder;
    ///
    /// let character = RegexBuilder::new().any_char().build().unwrap();
    /// assert!(character.full_match("🦀").unwrap().is_some());
    /// assert!(character.full_match("ab").unwrap().is_none());
    /// ```
    pub fn any_char(self) -> RegexBuilder<Quantifiable> {
        self.any_except(|set| set)
    }

    /// Matches any decimal digit.
    ///
    /// # Examples
    ///
    /// ```
    /// use regex_builder::RegexBuilder;
    ///
    /// let digit = RegexBuilder::new().any_digit().build().unwrap();
    /// assert!(digit.full_match("5").unwrap().is_some());
    /// assert!(digit.full_match("x").unwrap().is_none());
    /// ```
    pub fn any_digit(self) -> RegexBuilder<Quantifiable> {
        self.range('0', '9')
    }

    /// Matches any character that is not a decimal digit.
    ///
    /// # Examples
    ///
    /// ```
    /// use regex_builder::RegexBuilder;
    ///
    /// let not_digit = RegexBuilder::new().any_non_digit().build().unwrap();
    /// assert!(not_digit.full_match("x").unwrap().is_some());
    /// assert!(not_digit.full_match("5").unwrap().is_none());
    /// ```
    pub fn any_non_digit(self) -> RegexBuilder<Quantifiable> {
        self.any_except(|set| set.range('0', '9'))
    }

    /// Matches an ASCII letter, digit, or underscore.
    ///
    /// # Examples
    ///
    /// ```
    /// use regex_builder::RegexBuilder;
    ///
    /// let word = RegexBuilder::new().any_word().build().unwrap();
    /// assert!(word.full_match("a").unwrap().is_some());
    /// assert!(word.full_match("_").unwrap().is_some());
    /// assert!(word.full_match("é").unwrap().is_none());
    /// ```
    pub fn any_word(self) -> RegexBuilder<Quantifiable> {
        self.any_char_of(CharSetBuilder::word_chars)
    }

    /// Matches any character other than an ASCII letter, digit, or underscore.
    ///
    /// # Examples
    ///
    /// ```
    /// use regex_builder::RegexBuilder;
    ///
    /// let punctuation = RegexBuilder::new().any_non_word().build().unwrap();
    /// assert!(punctuation.full_match("!").unwrap().is_some());
    /// assert!(punctuation.full_match("a").unwrap().is_none());
    /// ```
    pub fn any_non_word(self) -> RegexBuilder<Quantifiable> {
        self.any_except(CharSetBuilder::word_chars)
    }

    /// Matches ASCII space, tab, carriage return, line feed, vertical tab, or
    /// form feed.
    ///
    /// # Examples
    ///
    /// ```
    /// use regex_builder::RegexBuilder;
    ///
    /// let whitespace = RegexBuilder::new().any_whitespace().build().unwrap();
    /// assert!(whitespace.full_match("\t").unwrap().is_some());
    /// assert!(whitespace.full_match("x").unwrap().is_none());
    /// ```
    pub fn any_whitespace(self) -> RegexBuilder<Quantifiable> {
        self.any_char_of(|set| set.literal(" \t\r\n\x0B\x0C"))
    }

    /// Matches any character other than ASCII space, tab, carriage return,
    /// line feed, vertical tab, or form feed.
    ///
    /// # Examples
    ///
    /// ```
    /// use regex_builder::RegexBuilder;
    ///
    /// let non_whitespace = RegexBuilder::new().any_non_whitespace().build().unwrap();
    /// assert!(non_whitespace.full_match("x").unwrap().is_some());
    /// assert!(non_whitespace.full_match(" ").unwrap().is_none());
    /// ```
    pub fn any_non_whitespace(self) -> RegexBuilder<Quantifiable> {
        self.any_except(|set| set.literal(" \t\r\n\x0B\x0C"))
    }

    /// Matches any hexadecimal digit.
    ///
    /// # Examples
    ///
    /// ```
    /// use regex_builder::RegexBuilder;
    ///
    /// let hex = RegexBuilder::new().any_hex_digit().build().unwrap();
    /// assert!(hex.full_match("F").unwrap().is_some());
    /// assert!(hex.full_match("g").unwrap().is_none());
    /// ```
    pub fn any_hex_digit(self) -> RegexBuilder<Quantifiable> {
        self.any_char_of(|set| set.range('0', '9').range('a', 'f').range('A', 'F'))
    }

    /// Matches any character that is not a hexadecimal digit.
    ///
    /// # Examples
    ///
    /// ```
    /// use regex_builder::RegexBuilder;
    ///
    /// let non_hex = RegexBuilder::new().any_non_hex_digit().build().unwrap();
    /// assert!(non_hex.full_match("g").unwrap().is_some());
    /// assert!(non_hex.full_match("F").unwrap().is_none());
    /// ```
    pub fn any_non_hex_digit(self) -> RegexBuilder<Quantifiable> {
        self.any_except(|set| set.range('0', '9').range('a', 'f').range('A', 'F'))
    }

    /// Matches an ASCII letter (`A-Z` or `a-z`).
    ///
    /// # Examples
    ///
    /// ```
    /// use regex_builder::RegexBuilder;
    ///
    /// let letter = RegexBuilder::new().any_letter().build().unwrap();
    /// assert!(letter.full_match("Q").unwrap().is_some());
    /// assert!(letter.full_match("7").unwrap().is_none());
    /// ```
    pub fn any_letter(self) -> RegexBuilder<Quantifiable> {
        self.any_char_of(|set| set.range('a', 'z').range('A', 'Z'))
    }

    /// Matches an ASCII lowercase letter (`a-z`).
    ///
    /// # Examples
    ///
    /// ```
    /// use regex_builder::RegexBuilder;
    ///
    /// let lowercase = RegexBuilder::new().any_lowercase().build().unwrap();
    /// assert!(lowercase.full_match("q").unwrap().is_some());
    /// assert!(lowercase.full_match("Q").unwrap().is_none());
    /// ```
    pub fn any_lowercase(self) -> RegexBuilder<Quantifiable> {
        self.range('a', 'z')
    }

    /// Matches an ASCII uppercase letter (`A-Z`).
    ///
    /// # Examples
    ///
    /// ```
    /// use regex_builder::RegexBuilder;
    ///
    /// let uppercase = RegexBuilder::new().any_uppercase().build().unwrap();
    /// assert!(uppercase.full_match("Q").unwrap().is_some());
    /// assert!(uppercase.full_match("q").unwrap().is_none());
    /// ```
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
