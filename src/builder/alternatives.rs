//! [`AlternativesBuilder`]: the closure-based builder for alternative
//! branches, used by `or`, `or_named`, `any_of` and `any_of_named`.
//!
//! Every branch is built with a fresh [`RegexBuilder`], which is what makes
//! each branch a capture scope of its own.

use super::regex_builder::RegexBuilder;
use super::state::{AppendOnly, BuilderState};
use crate::error::BuildError;
use crate::pattern::PatternNode;

/// A builder for a sequence of alternative branches.
pub struct AlternativesBuilder {
    /// The branches in the alternative expression.
    pub(crate) branches: Vec<PatternNode>,

    /// The first error encountered in any branch.
    pub(crate) error: Option<BuildError>,
}

impl AlternativesBuilder {
    /// Adds a branch to the alternative list.
    ///
    /// The provided closure receives a fresh `RegexBuilder<AppendOnly>` so the
    /// branch can be built independently. Branches are numbered from zero in
    /// the order they are added, and that number is reported in the result.
    /// Every branch is a scope of its own: captures inside a branch are
    /// reached through the alternative's occurrence, and only the taken
    /// branch's captures are present there.
    pub fn branch<T: BuilderState>(
        mut self,
        build_branch: impl FnOnce(RegexBuilder<AppendOnly>) -> RegexBuilder<T>,
    ) -> AlternativesBuilder {
        let built = build_branch(RegexBuilder::new());
        self.branches.push(PatternNode::Sequence(built.nodes));
        self.error = self.error.or(built.error);
        self
    }
}

/// Adds one literal branch per option, in order.
pub(crate) fn add_literal_branches<T: Into<String>>(
    builder: AlternativesBuilder,
    options: Vec<T>,
) -> AlternativesBuilder {
    options.into_iter().fold(builder, |builder, option| {
        builder.branch(|branch| branch.literal(option))
    })
}
