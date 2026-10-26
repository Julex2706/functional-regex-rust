//! Recording recursive occurrences.
//!
//! Every match of a recursive body becomes one
//! [`Group`](crate::CaptureOccurrence::Group) occurrence in the scope that
//! contains the reference.

use std::sync::Arc;

use crate::built::ScopeLayout;
use crate::matching::{MatchStates, NestedScope, RecordedKind, RecordedOccurrence, ScopeRecord};
use crate::pattern::CaptureSlot;

/// Records one occurrence of a recursive reference for every state of the
/// recursion's body, on top of the capture data of the current scope.
pub(crate) fn wrap_recursive(
    slot: &CaptureSlot,
    position: usize,
    states: MatchStates,
    layout: &Arc<ScopeLayout>,
    scope_data: &ScopeRecord,
) -> MatchStates {
    states
        .into_iter()
        .map(|(end, inner_data)| {
            let mut updated = scope_data.clone();

            updated
                .occurrences
                .entry(slot.clone())
                .or_default()
                .push(RecordedOccurrence {
                    char_span: (position, end),
                    kind: RecordedKind::Group(NestedScope {
                        record: Box::new(inner_data),
                        layout: layout.clone(),
                    }),
                });

            (end, updated)
        })
        .collect()
}
