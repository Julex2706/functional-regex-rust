//! The builder phase: assembling a pattern with a typestate builder.
//!
//! - `state`: the [`BuilderState`] markers that gate which methods exist.
//! - `regex_builder`: the [`RegexBuilder`] type itself and its constructor.
//! - `internals`: crate-private helpers shared by all builder methods.
//! - `atoms`: literals, nested regexes and character sets.
//! - `groups`: groups, captures and alternatives.
//! - `quantifiers`: repetition, available only in the right states.
//! - `shortcuts`: predefined classes such as digits, words and borders.
//! - `char_set` and `alternatives`: the closure-based sub-builders.
//!
//! The builder ends at `build` (see the `built` module) or at
//! [`RecursiveRegex::define`](crate::RecursiveRegex::define).

mod alternatives;
mod atoms;
mod char_set;
mod groups;
mod internals;
mod quantifiers;
mod regex_builder;
mod shortcuts;
mod state;

pub use alternatives::AlternativesBuilder;
pub use char_set::CharSetBuilder;
pub use regex_builder::RegexBuilder;
pub use state::{AppendOnly, BuilderState, Quantifiable, Quantified};
