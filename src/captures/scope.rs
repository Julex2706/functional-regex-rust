//! [`Captures`]: the capture results of a single regex scope.

use std::collections::HashMap;

use super::CaptureOccurrence;

/// The capture results of one regex scope.
///
/// Unnamed groups, unnamed alternatives and unnamed nested regexes share a
/// single zero-based index space. Named captures, named alternatives and named
/// nested regexes share a single name namespace. Both accessors return the
/// same kind of result: the ordered occurrences of that capture location.
///
/// Accessors distinguish three situations:
///
/// * `None`: the location does not exist in this scope (index out of range or
///   unknown name).
/// * `Some(&[])`: the location exists but its route was not taken.
/// * `Some(&[..])`: one entry per occurrence, in order of production.
#[derive(Debug, PartialEq)]
pub struct Captures {
    /// Occurrences of every unnamed location, indexed by position in the
    /// shared index space. Untaken locations hold an empty vector.
    pub(crate) indexed: Vec<Vec<CaptureOccurrence>>,

    /// Occurrences of every named location, keyed by name. Untaken locations
    /// hold an empty vector.
    pub(crate) by_name: HashMap<String, Vec<CaptureOccurrence>>,
}

impl Captures {
    /// Returns the occurrences of the nth unnamed group, unnamed alternative,
    /// or unnamed nested regex in this scope, counting from zero.
    pub fn at_index(&self, index: usize) -> Option<&[CaptureOccurrence]> {
        self.indexed.get(index).map(Vec::as_slice)
    }

    /// Returns the occurrences of a named capture, named alternative or named
    /// nested regex in this scope.
    pub fn with_name(&self, name: impl AsRef<str>) -> Option<&[CaptureOccurrence]> {
        self.by_name.get(name.as_ref()).map(Vec::as_slice)
    }
}
