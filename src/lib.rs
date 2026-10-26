//! A simple functional builder for reusable regular expressions.
//!
//! Patterns are assembled with a typestate [`RegexBuilder`], compiled into a
//! [`Regex`], and matched against a whole input with [`Regex::full_match`].
//! A successful match yields hierarchical [`Captures`] that keep every
//! occurrence of repeated captures, the branch taken by alternatives, and
//! untaken routes.
//!
//! # Architecture
//!
//! The crate is organised by phase. Only the *public API* modules export
//! anything; the *core private* modules are crate-internal.
//!
//! Public API:
//! - `error`: [`BuildError`].
//! - `builder` (builder phase): the typestate [`RegexBuilder`], the set and
//!   alternative sub-builders, and the state markers.
//! - `built` (built phase): the compiled [`Regex`], scope numbering and
//!   validation (`build`).
//! - `captures`: [`Captures`] and [`CaptureOccurrence`], the public results.
//! - `recursion`: [`RecursiveRegex`] and everything else that is specific to
//!   recursive regexes.
//!
//! Core private:
//! - `pattern`: the pattern tree that builders produce and matching consumes.
//! - `matching` (match phase): the matching engine, its bookkeeping records,
//!   and the conversion of records into public [`Captures`].
#![allow(unused)]
#![deny(missing_docs)]

mod builder;
mod built;
mod captures;
mod error;
mod matching;
mod pattern;
mod recursion;

pub use builder::{
    AlternativesBuilder, AppendOnly, BuilderState, CharSetBuilder, Quantifiable, Quantified,
    RegexBuilder,
};
pub use built::Regex;
pub use captures::{CaptureOccurrence, Captures};
pub use error::BuildError;
pub use recursion::RecursiveRegex;
