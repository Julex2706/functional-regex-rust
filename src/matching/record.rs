//! Internal match bookkeeping.
//!
//! While matching, every way a pattern can match is a *match state*: an end
//! position plus the [`ScopeRecord`] collected on the way there. Records are
//! cloned freely so that alternative ways of matching never interfere. They
//! are converted into public captures only for the winning state (see
//! `results`).

use std::collections::HashMap;
use std::sync::Arc;

use crate::built::ScopeLayout;
use crate::pattern::CaptureSlot;

/// A sequence of possible match states for a pattern: end position plus the
/// captures collected on the way there.
pub(crate) type MatchStates = Vec<(usize, ScopeRecord)>;

/// Captures collected for one scope while evaluating a pattern.
///
/// Each slot maps to its occurrences in production order. Slots that were
/// never reached simply have no entry; they are materialized as empty vectors
/// when the final [`Captures`](crate::Captures) is built.
#[derive(Debug, Clone, Default)]
pub(crate) struct ScopeRecord {
    /// The recorded occurrences of each slot reached so far.
    pub(crate) occurrences: HashMap<CaptureSlot, Vec<RecordedOccurrence>>,
}

/// One recorded occurrence of a slot.
#[derive(Debug, Clone)]
pub(crate) struct RecordedOccurrence {
    /// Half-open character span `[start, end)` consumed by this occurrence.
    /// Retaining it lets `matched_text()` return the exact matched text.
    pub(crate) char_span: (usize, usize),

    /// What kind of occurrence this is, together with its nested scope if it
    /// has one.
    pub(crate) kind: RecordedKind,
}

/// The kind of recorded occurrence.
#[derive(Debug, Clone)]
pub(crate) enum RecordedKind {
    /// A capture without nested capture locations: only the text is needed.
    Literal,

    /// A group-like occurrence with a nested scope.
    Group(NestedScope),

    /// An alternative occurrence: the taken branch and that branch's scope.
    Alternative {
        /// The zero-based index of the branch that was taken.
        branch_index: usize,
        /// The scope of the taken branch.
        scope: NestedScope,
    },
}

/// The nested scope belonging to one group or alternative occurrence.
#[derive(Debug, Clone)]
pub(crate) struct NestedScope {
    /// Captures recorded inside this occurrence only.
    pub(crate) record: Box<ScopeRecord>,

    /// Describes which locations exist in this scope.
    pub(crate) layout: Arc<ScopeLayout>,
}
