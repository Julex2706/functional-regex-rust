//! [`CaptureSlot`]: the identity of a capture location within one scope.

/// A capture location within a single scope.
///
/// Groups, captures, alternatives and nested regexes all use this type, which
/// is what gives them a shared index space and a shared name namespace.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum CaptureSlot {
    /// A location identified by its zero-based index.
    Index(usize),

    /// A location identified by a user-defined name.
    Name(String),
}
