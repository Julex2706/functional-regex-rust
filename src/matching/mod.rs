//! The match phase: running a built regex against an input.
//!
//! - `record`: the internal bookkeeping collected while matching.
//! - `engine`: the backtracking matcher over the pattern tree.
//! - `full_match`: the public entry point, `Regex::full_match`.
//! - `results`: conversion of internal records into public
//!   [`Captures`](crate::Captures).
//!
//! The recursion-specific support used by the engine lives in `recursion`.

mod engine;
mod full_match;
mod record;
mod results;

pub(crate) use record::{MatchStates, NestedScope, RecordedKind, RecordedOccurrence, ScopeRecord};
