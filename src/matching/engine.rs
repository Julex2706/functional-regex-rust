//! The matching engine.
//!
//! [`PatternNode::match_at`] returns *every* way a pattern can match at a
//! position, in priority order (greedy before lazy, earlier alternative
//! branches first). Capture-bearing nodes run their inner pattern against a
//! fresh [`ScopeRecord`], which is what makes them separate scopes, and
//! append one occurrence per way of matching. [`PatternNode::match_repeat`]
//! handles quantifiers and threads the capture data through every iteration.
//!
//! Recursive references are evaluated here too, using the support types from
//! the `recursion` module (seeded left-recursion handling).

use std::sync::Arc;

use super::record::{MatchStates, NestedScope, RecordedKind, RecordedOccurrence, ScopeRecord};
use crate::pattern::PatternNode;
use crate::recursion::{distinct_ends, wrap_recursive, RecursionMemo, SeedEntry};

impl PatternNode {
    /// Attempts to run this pattern from `position` in the input.
    ///
    /// Returns every way the pattern can match, in priority order, each with
    /// the capture data collected so far for the *current* scope. `memo`
    /// tracks the recursions currently being evaluated.
    pub(crate) fn match_at(
        &self,
        input: &[char],
        position: usize,
        scope_data: ScopeRecord,
        memo: &mut RecursionMemo,
    ) -> MatchStates {
        match self {
            PatternNode::Literal(text) if input[position..].starts_with(text) => {
                vec![(position + text.len(), scope_data)]
            }
            PatternNode::Literal(_) => vec![],

            PatternNode::CharClass { items, negated } => match input.get(position) {
                Some(ch) if items.iter().any(|item| item.accepts(*ch)) != *negated => {
                    vec![(position + 1, scope_data)]
                }
                _ => vec![],
            },

            PatternNode::Boundary { items, negated } => {
                let is_in_set = |ch: &char| items.iter().any(|item| item.accepts(*ch));
                let inside_before = position
                    .checked_sub(1)
                    .map_or(false, |prev| is_in_set(&input[prev]));
                let inside_after = input.get(position).map_or(false, is_in_set);

                if (inside_before != inside_after) != *negated {
                    vec![(position, scope_data)]
                } else {
                    vec![]
                }
            }

            PatternNode::Sequence(nodes) => {
                nodes
                    .iter()
                    .fold(vec![(position, scope_data)], |states, node| {
                        states
                            .into_iter()
                            .flat_map(|(state_position, state_data)| {
                                node.match_at(input, state_position, state_data, memo)
                            })
                            .collect()
                    })
            }

            // Every branch runs against a fresh `ScopeRecord`, so each branch is
            // a scope of its own. Each way a branch can match becomes one
            // occurrence that remembers the branch index. Branches are tried
            // in order, which gives earlier branches priority.
            PatternNode::Alternation {
                slot,
                branches,
                branch_layouts,
            } => {
                let mut results = MatchStates::new();

                for (branch_index, branch_pattern) in branches.iter().enumerate() {
                    for (end, branch_data) in
                        branch_pattern.match_at(input, position, ScopeRecord::default(), memo)
                    {
                        let mut updated = scope_data.clone();

                        updated.occurrences.entry(slot.clone()).or_default().push(
                            RecordedOccurrence {
                                char_span: (position, end),
                                kind: RecordedKind::Alternative {
                                    branch_index,
                                    scope: NestedScope {
                                        record: Box::new(branch_data),
                                        layout: branch_layouts[branch_index].clone(),
                                    },
                                },
                            },
                        );

                        results.push((end, updated));
                    }
                }

                results
            }

            // The inner pattern runs against a fresh `ScopeRecord`, which is what
            // makes the capture a separate scope. The finished inner data is
            // attached to a new occurrence appended to this scope's slot, so
            // repeated captures accumulate instead of overwriting.
            PatternNode::Capturing {
                slot,
                inner,
                layout,
            } => {
                inner
                    .match_at(input, position, ScopeRecord::default(), memo)
                    .into_iter()
                    .map(|(end, inner_data)| {
                        let mut updated = scope_data.clone();

                        updated.occurrences.entry(slot.clone()).or_default().push(
                            RecordedOccurrence {
                                char_span: (position, end),
                                kind: if layout.declares_nothing() {
                                    RecordedKind::Literal
                                } else {
                                    RecordedKind::Group(NestedScope {
                                        record: Box::new(inner_data),
                                        layout: layout.clone(),
                                    })
                                },
                            },
                        );

                        (end, updated)
                    })
                    .collect()
            }

            PatternNode::Embedded {
                slot,
                inner,
                layout,
            } => {
                inner
                    .match_at(input, position, ScopeRecord::default(), memo)
                    .into_iter()
                    .map(|(end, inner_data)| {
                        let mut updated = scope_data.clone();

                        updated.occurrences.entry(slot.clone()).or_default().push(
                            RecordedOccurrence {
                                char_span: (position, end),
                                kind: RecordedKind::Group(NestedScope {
                                    record: Box::new(inner_data),
                                    layout: layout.clone(),
                                }),
                            },
                        );

                        (end, updated)
                    })
                    .collect()
            }

            // Like `Embedded`, but the body comes from the shared cell.
            //
            // Entering the same recursion at the same position while it is
            // still being evaluated means it is left-recursive there. That
            // inner entry is answered with the matches found so far (the
            // seed, initially none). The body is then evaluated again with
            // the grown seed until it stops reaching new end positions, which
            // makes the recursion terminate and still match left-nested input.
            PatternNode::Recursive { slot, cell } => {
                let Some(definition) = cell.definition.get() else {
                    return vec![]; // unreachable: definedness is checked before matching
                };

                let key = (Arc::as_ptr(cell) as usize, position);

                if let Some(entry) = memo.get_mut(&key) {
                    entry.used = true;
                    let seed = entry.seed.clone();
                    return wrap_recursive(slot, position, seed, &definition.layout, &scope_data);
                }

                memo.insert(
                    key,
                    SeedEntry {
                        seed: vec![],
                        used: false,
                    },
                );

                let inner_results = loop {
                    let results = definition.root.match_at(
                        input,
                        position,
                        ScopeRecord::default(),
                        memo,
                    );

                    let entry = memo
                        .get_mut(&key)
                        .expect("a recursion entry exists while it is being evaluated");

                    if !entry.used || distinct_ends(&results) <= distinct_ends(&entry.seed) {
                        break results;
                    }

                    entry.seed = results;
                    entry.used = false;
                };

                memo.remove(&key);

                wrap_recursive(
                    slot,
                    position,
                    inner_results,
                    &definition.layout,
                    &scope_data,
                )
            }

            PatternNode::Repeat {
                inner,
                min,
                max,
                lazy,
            } => inner.match_repeat(*min, *max, *lazy, input, position, scope_data, 0, memo),
        }
    }

    /// Recursively evaluates a repeated pattern while handling greedy and lazy
    /// expansion rules.
    ///
    /// The capture data is threaded through every iteration, so occurrences of
    /// captures inside the repeated pattern are appended in iteration order.
    fn match_repeat(
        &self,
        min: usize,
        max: Option<usize>,
        lazy: bool,
        input: &[char],
        position: usize,
        scope_data: ScopeRecord,
        completed: usize,
        memo: &mut RecursionMemo,
    ) -> MatchStates {
        let stopped: MatchStates = if completed >= min {
            vec![(position, scope_data.clone())]
        } else {
            vec![]
        };

        let extended: MatchStates = if max.map_or(true, |limit| completed < limit) {
            self.match_at(input, position, scope_data, memo)
                .into_iter()
                .filter(|(end, _)| !(*end == position && completed >= min))
                .flat_map(|(end, next_data)| {
                    self.match_repeat(min, max, lazy, input, end, next_data, completed + 1, memo)
                })
                .collect()
        } else {
            vec![]
        };

        if lazy {
            [stopped, extended].concat()
        } else {
            [extended, stopped].concat()
        }
    }
}
