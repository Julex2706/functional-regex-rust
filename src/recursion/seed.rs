//! Support types for evaluating (left-)recursive references.
//!
//! The engine keeps a [`RecursionMemo`] of the recursions currently being
//! evaluated. Re-entering one at the same position is a left recursion and is
//! answered with the entry's *seed*, which grows until it reaches a fixed
//! point (see the `Recursive` arm of `PatternNode::match_at`).

use std::collections::{HashMap, HashSet};

use crate::matching::MatchStates;

/// The recursions currently being evaluated, keyed by (cell address, input
/// position). An entry that is looked up again while it is still being
/// evaluated is a left recursion; it is answered with the entry's seed.
pub(crate) type RecursionMemo = HashMap<(usize, usize), SeedEntry>;

/// Evaluation state of one recursion at one input position.
pub(crate) struct SeedEntry {
    /// The matches found so far. Starts empty and grows until it reaches a
    /// fixed point.
    pub(crate) seed: MatchStates,

    /// Whether the seed was consulted during the current evaluation, i.e.
    /// whether the recursion is left-recursive here.
    pub(crate) used: bool,
}

/// Counts the distinct end positions among a set of match states.
pub(crate) fn distinct_ends(states: &MatchStates) -> usize {
    states
        .iter()
        .map(|(end, _)| *end)
        .collect::<HashSet<_>>()
        .len()
}
