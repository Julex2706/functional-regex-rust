//! [`ScopeLayout`]: the static description of a scope's capture locations.

/// Static description of the capture locations declared in one scope.
///
/// It is what lets untaken routes be reported as empty vectors: every
/// declared location is known even when it never matched.
#[derive(Debug, Default)]
pub(crate) struct ScopeLayout {
    /// Number of index slots: unnamed groups, alternatives and nested regexes.
    pub(crate) index_count: usize,

    /// Names of named captures, alternatives and nested regexes (one shared
    /// namespace).
    pub(crate) declared_names: Vec<String>,
}

impl ScopeLayout {
    /// True if the scope declares no capture locations at all.
    pub(crate) fn declares_nothing(&self) -> bool {
        self.index_count == 0 && self.declared_names.is_empty()
    }
}
