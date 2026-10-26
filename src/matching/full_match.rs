//! `Regex::full_match`: the public entry point of the match phase.

use std::collections::HashMap;

use super::record::ScopeRecord;
use super::results::build_captures;
use crate::built::Regex;
use crate::captures::Captures;
use crate::error::BuildError;

impl Regex {
    /// Attempts to match the entire input and returns the captures for the
    /// successful match.
    ///
    /// The regex must consume the whole input string from start to finish. If
    /// it does, the hierarchical capture data is returned; otherwise `None`.
    /// When several matches are possible, the first one in greedy/lazy
    /// priority order wins.
    ///
    /// # Errors
    ///
    /// Returns [`BuildError::UndefinedRecursion`] if the regex depends on a
    /// recursive reference that has not been defined yet.
    pub fn full_match(&self, input: impl Into<String>) -> Result<Option<Captures>, BuildError> {
        self.ensure_defined()?;

        let chars: Vec<char> = input.into().chars().collect();

        let winning_data = self
            .root
            .match_at(&chars, 0, ScopeRecord::default(), &mut HashMap::new())
            .into_iter()
            .find(|(end, _)| *end == chars.len())
            .map(|(_, data)| data);

        Ok(winning_data.map(|data| build_captures(&data, &chars, &self.layout)))
    }
}
