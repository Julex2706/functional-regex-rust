//! The built phase: from finished builder to compiled [`Regex`].
//!
//! - `regex`: the [`Regex`] type.
//! - `layout`: the static description of the capture locations of a scope.
//! - `compile`: scope validation and index assignment.
//! - `build`: `RegexBuilder::build`, the public hand-off from the builder.
//!
//! Running a built regex is the job of the `matching` module.

mod build;
mod compile;
mod layout;
mod regex;

pub(crate) use layout::ScopeLayout;
pub use regex::Regex;
