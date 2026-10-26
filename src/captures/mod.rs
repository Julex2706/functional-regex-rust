//! The public result of a successful match.
//!
//! - `scope` holds [`Captures`], the results of one regex scope.
//! - `occurrence` holds [`CaptureOccurrence`], one occurrence of one capture
//!   location, with typed accessors.
//! - `display` holds the human-readable tree rendering of both.
//!
//! The internal records that these types are built from live in the
//! `matching` module.

mod display;
mod occurrence;
mod scope;

pub use occurrence::CaptureOccurrence;
pub use scope::Captures;
