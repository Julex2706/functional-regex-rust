//! Scope compilation: validation and index assignment.
//!
//! Compilation works scope by scope. A scope is the top level, the inside of
//! a group or capture, or one alternative branch. For each scope it checks
//! that names are unique, numbers the unnamed locations from zero, and then
//! compiles the nested scopes independently. Embedded regexes and recursive
//! references are not descended into: they were compiled separately.

use std::collections::HashSet;
use std::sync::Arc;

use super::ScopeLayout;
use crate::error::BuildError;
use crate::pattern::{CaptureSlot, PatternNode};

impl PatternNode {
    /// Validates and numbers one scope.
    ///
    /// Checks for duplicate names among the locations declared directly in
    /// this scope, assigns indexes, and recursively compiles the scopes of
    /// nested groups, captures and alternative branches. Returns the compiled
    /// pattern together with the layout describing this scope.
    pub(crate) fn compile_scope(self) -> Result<(PatternNode, ScopeLayout), BuildError> {
        let declared_names = self.declare_scope_names();
        let mut seen_names = HashSet::new();

        for name in &declared_names {
            if !seen_names.insert(name.as_str()) {
                return Err(BuildError::DuplicateName(name.clone()));
            }
        }

        let mut next_index = 0;
        let compiled = self.assign_indexes(&mut next_index)?;

        Ok((
            compiled,
            ScopeLayout {
                index_count: next_index,
                declared_names,
            },
        ))
    }

    /// Assigns indexes within the current scope.
    ///
    /// Unnamed groups, unnamed alternatives, unnamed nested regexes and
    /// unnamed recursive references draw from the same counter, so they share
    /// one index space. Nested group, capture and branch scopes restart their
    /// own numbering; nested regexes and recursive references are already
    /// compiled (or compiled separately) and are not descended into.
    fn assign_indexes(self, counter: &mut usize) -> Result<PatternNode, BuildError> {
        Ok(match self {
            PatternNode::Sequence(nodes) => {
                PatternNode::Sequence(Self::assign_indexes_in_order(nodes, counter)?)
            }

            PatternNode::Alternation { slot, branches, .. } => {
                let slot = match slot {
                    CaptureSlot::Index(_) => CaptureSlot::Index(next_index(counter)),
                    named_slot => named_slot,
                };

                let (branches, branch_layouts) = branches
                    .into_iter()
                    .map(|branch| {
                        branch
                            .compile_scope()
                            .map(|(compiled, layout)| (compiled, Arc::new(layout)))
                    })
                    .collect::<Result<Vec<_>, _>>()?
                    .into_iter()
                    .unzip();

                PatternNode::Alternation {
                    slot,
                    branches,
                    branch_layouts,
                }
            }

            PatternNode::Capturing { slot, inner, .. } => {
                let slot = match slot {
                    CaptureSlot::Index(_) => CaptureSlot::Index(next_index(counter)),
                    named_slot => named_slot,
                };

                let (inner, layout) = inner.compile_scope()?;

                PatternNode::Capturing {
                    slot,
                    inner: Box::new(inner),
                    layout: Arc::new(layout),
                }
            }

            PatternNode::Embedded {
                slot,
                inner,
                layout,
            } => PatternNode::Embedded {
                slot: match slot {
                    CaptureSlot::Index(_) => CaptureSlot::Index(next_index(counter)),
                    named_slot => named_slot,
                },
                inner,
                layout,
            },

            PatternNode::Recursive { slot, cell } => PatternNode::Recursive {
                slot: match slot {
                    CaptureSlot::Index(_) => CaptureSlot::Index(next_index(counter)),
                    named_slot => named_slot,
                },
                cell,
            },

            PatternNode::Repeat {
                inner,
                min,
                max,
                lazy,
            } => PatternNode::Repeat {
                inner: Box::new(inner.assign_indexes(counter)?),
                min,
                max,
                lazy,
            },

            other => other,
        })
    }

    /// Assigns indexes to every pattern in a vector, in order, sharing one
    /// counter.
    fn assign_indexes_in_order(
        nodes: Vec<PatternNode>,
        counter: &mut usize,
    ) -> Result<Vec<PatternNode>, BuildError> {
        nodes
            .into_iter()
            .map(|node| node.assign_indexes(counter))
            .collect()
    }

    /// Collects the names declared directly in the current scope.
    ///
    /// Named captures, named alternatives, named nested regexes and named
    /// recursive references share one namespace. Contents of groups,
    /// captures, alternative branches, nested regexes and recursive
    /// references are excluded because each opens its own scope with its own
    /// namespace.
    fn declare_scope_names(&self) -> Vec<String> {
        match self {
            PatternNode::Sequence(nodes) => nodes
                .iter()
                .flat_map(PatternNode::declare_scope_names)
                .collect(),

            PatternNode::Repeat { inner, .. } => inner.declare_scope_names(),

            PatternNode::Capturing {
                slot: CaptureSlot::Name(name),
                ..
            }
            | PatternNode::Embedded {
                slot: CaptureSlot::Name(name),
                ..
            }
            | PatternNode::Recursive {
                slot: CaptureSlot::Name(name),
                ..
            }
            | PatternNode::Alternation {
                slot: CaptureSlot::Name(name),
                ..
            } => vec![name.clone()],

            _ => vec![],
        }
    }
}

/// Returns the current index and advances the counter.
fn next_index(counter: &mut usize) -> usize {
    let index = *counter;
    *counter += 1;
    index
}
