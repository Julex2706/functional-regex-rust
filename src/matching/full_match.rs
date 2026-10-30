//! `Regex::full_match`: the public entry point of the match phase.

use std::collections::HashMap;

use super::record::ScopeRecord;
use super::results::build_captures;
use crate::built::Regex;
use crate::captures::Captures;
use crate::error::BuildError;
use crate::pattern::PatternNode;

impl Regex {
    /// Attempts to match the entire input and returns the captures for the
    /// successful match.
    ///
    /// The regex must consume the whole input string from start to finish. If
    /// it does, the hierarchical capture data is returned; otherwise `None`.
    /// When several matches are possible, the first one in greedy/lazy
    /// priority order wins. Matching operates on Unicode scalar values
    /// (`char`), rather than UTF-8 bytes.
    ///
    /// Top-level repetitions are explored one match at a time, so later
    /// repetition paths are not evaluated once a full match is found.
    /// Repetition is traversed iteratively, but nested pattern structure and
    /// recursive references still use the call stack. The engine explores
    /// possible paths by backtracking and imposes no time, memory, or
    /// recursion-depth limit, so ambiguous patterns can consume substantial
    /// resources.
    ///
    /// A failed whole-input match is `Ok(None)`, while an invalid regex
    /// construction or undefined recursive reference is an `Err`.
    ///
    /// # Examples
    ///
    /// ```
    /// use regex_builder::RegexBuilder;
    ///
    /// let yes_or_no = RegexBuilder::new()
    ///     .any_of(vec!["yes", "no"])
    ///     .build()
    ///     .unwrap();
    ///
    /// assert!(yes_or_no.full_match("yes").unwrap().is_some());
    /// assert!(yes_or_no.full_match("maybe").unwrap().is_none());
    /// ```
    ///
    /// # Errors
    ///
    /// Returns [`BuildError::UndefinedRecursion`] if the regex depends on a
    /// recursive reference that has not been defined yet.
    pub fn full_match(&self, input: impl Into<String>) -> Result<Option<Captures>, BuildError> {
        self.ensure_defined()?;

        let chars: Vec<char> = input.into().chars().collect();

        let mut memo = HashMap::new();
        let winning_data = if let PatternNode::Sequence(nodes) = &self.root {
            if let Some((
                repeat_index,
                PatternNode::Repeat {
                    inner,
                    min,
                    max,
                    lazy,
                },
            )) = nodes
                .iter()
                .enumerate()
                .find(|(_, node)| matches!(node, PatternNode::Repeat { .. }))
            {
                let prefixes = PatternNode::match_sequence(
                    &nodes[..repeat_index],
                    &chars,
                    0,
                    ScopeRecord::default(),
                    &mut memo,
                );
                let suffix = &nodes[repeat_index + 1..];
                let mut winner = None;

                'prefixes: for (position, prefix_data) in prefixes {
                    let mut repetitions =
                        inner.match_repeat(*min, *max, *lazy, &chars, position, prefix_data);
                    while let Some((end, repeated_data)) = repetitions.next_match(&mut memo) {
                        if let Some((_, data)) = PatternNode::match_sequence(
                            suffix,
                            &chars,
                            end,
                            repeated_data,
                            &mut memo,
                        )
                        .into_iter()
                        .find(|(end, _)| *end == chars.len())
                        {
                            winner = Some(data);
                            break 'prefixes;
                        }
                    }
                }

                winner
            } else {
                self.root
                    .match_at(&chars, 0, ScopeRecord::default(), &mut memo)
                    .into_iter()
                    .find(|(end, _)| *end == chars.len())
                    .map(|(_, data)| data)
            }
        } else {
            self.root
                .match_at(&chars, 0, ScopeRecord::default(), &mut memo)
                .into_iter()
                .find(|(end, _)| *end == chars.len())
                .map(|(_, data)| data)
        };

        Ok(winning_data.map(|data| build_captures(&data, &chars, &self.layout)))
    }
}
