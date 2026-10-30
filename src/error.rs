//! Errors that can occur while constructing or validating a regular
//! expression.
//!
//! Builders record the *first* construction error they see and report it when
//! the regex is finished (`build` or
//! [`RecursiveRegex::define`](crate::RecursiveRegex::define)). A regex that
//! still depends on an undefined recursive reference can be built, but
//! matching it returns [`BuildError::UndefinedRecursion`].
//!
//! # Examples
//!
//! ```
//! use regex_builder::{BuildError, RegexBuilder};
//!
//! let invalid_range = RegexBuilder::new().range('z', 'a').build();
//! assert_eq!(
//!     invalid_range.unwrap_err(),
//!     BuildError::InvalidRange { from: 'z', to: 'a' }
//! );
//! assert_eq!(
//!     BuildError::InvalidRange { from: 'z', to: 'a' }.to_string(),
//!     "range start 'z' comes after its end 'a'"
//! );
//! assert_eq!(
//!     BuildError::InvalidQuantifier { min: 3, max: 2 }.to_string(),
//!     "quantifier minimum 3 is greater than its maximum 2"
//! );
//!
//! let empty_alternative = RegexBuilder::new().or(|branches| branches).build();
//! assert_eq!(empty_alternative.unwrap_err(), BuildError::EmptyAlternative);
//! ```

use std::fmt;

/// Errors produced while constructing or validating a regular expression.
#[derive(Debug, PartialEq, Clone)]
pub enum BuildError {
    /// Signals that the same capture, alternative or regex name was used more
    /// than once in the same regex scope.
    DuplicateName(String),

    /// Signals that a quantifier was built with a minimum greater than the
    /// maximum.
    InvalidQuantifier {
        /// The minimum that was used.
        min: usize,
        /// The maximum that was used.
        max: usize,
    },

    /// Signals that a character range had its start after its end.
    InvalidRange {
        /// The start that was used.
        from: char,
        /// The end that was used.
        to: char,
    },

    /// Signals that an alternative expression (`or` / `any_of` and their named
    /// variants) has no branches.
    EmptyAlternative,

    /// Signals that matching was attempted while this regex depended on a
    /// recursive reference that had not been defined.
    UndefinedRecursion,
}

impl fmt::Display for BuildError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            BuildError::DuplicateName(name) => write!(
                f,
                "capture, alternative or regex name '{name}' is used more than once"
            ),
            BuildError::InvalidQuantifier { min, max } => write!(
                f,
                "quantifier minimum {min} is greater than its maximum {max}"
            ),
            BuildError::InvalidRange { from, to } => {
                write!(f, "range start '{from}' comes after its end '{to}'")
            }
            BuildError::EmptyAlternative => write!(f, "alternative expression has no branches"),
            BuildError::UndefinedRecursion => {
                write!(f, "recursive regex reference is used before it is defined")
            }
        }
    }
}

impl std::error::Error for BuildError {}
