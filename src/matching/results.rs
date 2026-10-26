//! Conversion of internal match records into the public capture results.
//!
//! Only the winning match state is converted. Matched text is sliced from the
//! recorded character spans, and every location declared in the scope layout
//! is materialized, which is what makes untaken routes observable.

use super::record::{RecordedKind, RecordedOccurrence, ScopeRecord};
use crate::built::ScopeLayout;
use crate::captures::{CaptureOccurrence, Captures};
use crate::pattern::CaptureSlot;

/// Builds the public [`Captures`] for one scope.
///
/// Every location declared in `layout` is materialized: locations that never
/// matched become empty vectors, which is what distinguishes an untaken route
/// from an occurrence that matched the empty string.
pub(crate) fn build_captures(record: &ScopeRecord, chars: &[char], layout: &ScopeLayout) -> Captures {
    let occurrences_of = |slot: CaptureSlot| {
        record
            .occurrences
            .get(&slot)
            .map(|recorded| {
                recorded
                    .iter()
                    .map(|occurrence| build_occurrence(occurrence, chars))
                    .collect()
            })
            .unwrap_or_default()
    };

    Captures {
        indexed: (0..layout.index_count)
            .map(|index| occurrences_of(CaptureSlot::Index(index)))
            .collect(),
        by_name: layout
            .declared_names
            .iter()
            .map(|name| {
                (
                    name.clone(),
                    occurrences_of(CaptureSlot::Name(name.clone())),
                )
            })
            .collect(),
    }
}

/// Converts one recorded occurrence into its public form.
///
/// The text is sliced from the exact span the occurrence consumed, so
/// `matched_text()` is correct per occurrence and includes everything matched
/// inside a group.
fn build_occurrence(occurrence: &RecordedOccurrence, chars: &[char]) -> CaptureOccurrence {
    let (start, end) = occurrence.char_span;
    let matched = chars[start..end].iter().collect();

    match &occurrence.kind {
        RecordedKind::Literal => CaptureOccurrence::Literal(matched),
        RecordedKind::Group(scope) => CaptureOccurrence::Group {
            matched,
            nested: build_captures(&scope.record, chars, &scope.layout),
        },
        RecordedKind::Alternative {
            branch_index,
            scope,
        } => CaptureOccurrence::Alternative {
            matched,
            branch_index: *branch_index,
            nested: build_captures(&scope.record, chars, &scope.layout),
        },
    }
}
