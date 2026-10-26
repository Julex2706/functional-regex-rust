//! [`CaptureOccurrence`]: one occurrence of one capture location.
//!
//! The enum distinguishes the three kinds of occurrence (group, literal,
//! alternative) and offers three families of accessors for them:
//! `try_*` (returns `Option`), `expect_*` (panics on the wrong kind) and the
//! kind-independent [`CaptureOccurrence::matched_text`].

use super::Captures;

/// A single occurrence of a capture location
///
/// The variant describes the *kind* of capture, not whether it matched: all
/// variants always represent a successful match of that occurrence.
///
/// * [`CaptureOccurrence::Group`] is a group, capture or nested regex that
///   declares its own capture locations. It carries the nested [`Captures`] of
///   this occurrence.
/// * [`CaptureOccurrence::Literal`] is a group or capture that contains no
///   capture locations of its own, so only its matched text is retained.
/// * [`CaptureOccurrence::Alternative`] is an alternative expression (`or`,
///   `any_of`  and their named variants). It records which branch was taken
///   (zero-based, in the order the branches were added) and carries the
///   captures of that branch only.
///
/// A group or capture is a `Group` if and only if its inner scope declares at
/// least one capture location (group, capture, alternative or nested regex).
/// Nested regexes (including recursive references) are always groups, and
/// alternatives are always `Alternative`, even if they declare no captures
/// themselves.
#[derive(Debug, PartialEq)]
pub enum CaptureOccurrence {
    /// An occurrence with a nested capture scope.
    Group {
        /// The complete text consumed by this occurrence, including text
        /// matched by expressions that are not individually addressable.
        matched: String,
        /// The captures declared inside this occurrence.
        nested: Captures,
    },

    /// An occurrence without nested capture locations.
    Literal(String),

    /// An occurrence of an alternative expression.
    Alternative {
        /// The complete text consumed by the taken branch.
        matched: String,
        /// The zero-based index of the branch that was taken.
        branch_index: usize,
        /// The captures declared inside the taken branch. Every branch is a
        /// scope of its own, so captures of other branches never appear here.
        nested: Captures,
    },
}

impl CaptureOccurrence {
    /// Returns the complete text matched by this occurrence.
    ///
    /// Works for every variant. For repeated captures, each occurrence returns
    /// only its own text, never a concatenation of all occurrences.
    pub fn matched_text(&self) -> &str {
        match self {
            CaptureOccurrence::Group { matched, .. }
            | CaptureOccurrence::Literal(matched)
            | CaptureOccurrence::Alternative { matched, .. } => matched,
        }
    }

    /// Returns the nested captures if this is a `Group`, otherwise `None`.
    pub fn try_group(&self) -> Option<&Captures> {
        match self {
            CaptureOccurrence::Group { nested, .. } => Some(nested),
            _ => None,
        }
    }

    /// Returns the matched text if this is a `Literal`, otherwise `None`.
    pub fn try_literal(&self) -> Option<&str> {
        match self {
            CaptureOccurrence::Literal(text) => Some(text),
            _ => None,
        }
    }

    /// Returns the taken branch index and the captures of that branch if this
    /// is an `Alternative`, otherwise `None`.
    pub fn try_alternative(&self) -> Option<(usize, &Captures)> {
        match self {
            CaptureOccurrence::Alternative {
                branch_index,
                nested,
                ..
            } => Some((*branch_index, nested)),
            _ => None,
        }
    }

    /// Returns the zero-based index of the taken branch if this is an
    /// `Alternative`, otherwise `None`.
    pub fn taken_branch(&self) -> Option<usize> {
        self.try_alternative().map(|(branch_index, _)| branch_index)
    }

    /// Returns the nested captures of a `Group`.
    ///
    /// # Panics
    ///
    /// Panics if this is not a `Group`. Use [`CaptureOccurrence::try_group`]
    /// for a non-panicking alternative.
    pub fn expect_group(&self) -> &Captures {
        self.try_group()
            .expect("expect_group() called on an occurrence that is not a Group")
    }

    /// Returns the matched text of a `Literal`.
    ///
    /// # Panics
    ///
    /// Panics if this is not a `Literal`. Use [`CaptureOccurrence::try_literal`]
    /// for a non-panicking alternative, or [`CaptureOccurrence::matched_text`]
    /// which works for every variant.
    pub fn expect_literal(&self) -> &str {
        self.try_literal()
            .expect("expect_literal() called on an occurrence that is not a Literal")
    }

    /// Returns the taken branch index and the captures of that branch of an
    /// `Alternative`.
    ///
    /// # Panics
    ///
    /// Panics if this is not an `Alternative`. Use
    /// [`CaptureOccurrence::try_alternative`] for a non-panicking alternative.
    pub fn expect_alternative(&self) -> (usize, &Captures) {
        self.try_alternative()
            .expect("expect_alternative() called on an occurrence that is not an Alternative")
    }
}
