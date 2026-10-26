//! Typestate markers of the regex builder.
//!
//! The markers carry no data. They exist so that the compiler rejects invalid
//! call sequences, for example quantifying before anything was appended, or
//! making a non-quantified atom lazy.

/// A regex construction state marker.
///
/// The builder moves through a small state machine:
/// - [`AppendOnly`]: new atoms can be appended, but no quantifier may be applied yet.
/// - [`Quantifiable`]: the most recently added atom may be quantified.
/// - [`Quantified`]: the most recently added atom was just quantified and can
///   still be made lazy.
pub trait BuilderState {}

/// Marks the start of a regex builder. New atoms can be appended, but the
/// pattern cannot be quantified.
#[derive(Debug)]
pub struct AppendOnly;

/// Marks a pattern segment that can be quantified.
#[derive(Debug)]
pub struct Quantifiable;

/// Marks a segment that was just quantified and may still be converted to a
/// lazy form.
#[derive(Debug)]
pub struct Quantified;

impl BuilderState for AppendOnly {}
impl BuilderState for Quantifiable {}
impl BuilderState for Quantified {}
