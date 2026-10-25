//! A simple functional builder for reusable regular expressions.

#![allow(unused, )]
#![deny(missing_docs)]

// ---------------------------------------------------------------------------
// Builder states
// ---------------------------------------------------------------------------

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::marker::PhantomData;
use std::sync::{Arc, OnceLock};

/// A regex construction state marker.
///
/// The builder moves through a small state machine:
/// - [`AppendOnly`]: new atoms can be appended, but no quantifier may be applied yet.
/// - [`Quantifiable`]: the most recently added atom may be quantified.
/// - [`Quantified`]: the most recently added atom was just quantified and can
///   still be made lazy.
pub trait BuilderState {}

/// Marks the start of a regex builder. New atoms can be appended, but the
/// pattern cannot be quantified.
#[derive(Debug)]
pub struct AppendOnly;

/// Marks a pattern segment that can be quantified.
#[derive(Debug)]
pub struct Quantifiable;

/// Marks a segment that was just quantified and may still be converted to a
/// lazy form.
#[derive(Debug)]
pub struct Quantified;

impl BuilderState for AppendOnly {}
impl BuilderState for Quantifiable {}
impl BuilderState for Quantified {}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Errors produced while constructing or validating a regular expression.
#[derive(Debug, PartialEq, Clone)]
pub enum BuildError {
    /// Signals that the same capture, alternative or regex name was used more
    /// than once in the same regex scope.
    DuplicateName(String),

    /// Signals that a quantifier was built with a minimum greater than the
    /// maximum.
    InvalidQuantifier {
        /// The minimum that was used.
        min: usize,
        /// The maximum that was used.
        max: usize,
    },

    /// Signals that a character range had its start after its end.
    InvalidRange {
        /// The start that was used.
        from: char,
        /// The end that was used.
        to: char,
    },

    /// Signals that an alternative expression (`or` / `any_of` and their named
    /// variants) has no branches.
    EmptyAlternative,

    /// Signals that a recursive regex reference was used to build or match
    /// before it was defined.
    UndefinedRecursion,
}

impl fmt::Display for BuildError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            BuildError::DuplicateName(name) => write!(
                f,
                "capture, alternative or regex name '{name}' is used more than once"
            ),
            BuildError::InvalidQuantifier { min, max } => write!(
                f,
                "quantifier minimum {min} is greater than its maximum {max}"
            ),
            BuildError::InvalidRange { from, to } => {
                write!(f, "range start '{from}' comes after its end '{to}'")
            }
            BuildError::EmptyAlternative => write!(f, "alternative expression has no branches"),
            BuildError::UndefinedRecursion => {
                write!(f, "recursive regex reference is used before it is defined")
            }
        }
    }
}

impl std::error::Error for BuildError {}

// ---------------------------------------------------------------------------
// Public capture results
// ---------------------------------------------------------------------------

/// A single occurrence of a capture location
///
/// The variant describes the *kind* of capture, not whether if matched: all
/// variants always represent a successful match of that occurrence.
///
/// * [`CaptureOccurrence::Group`] is a group, capture or nested regex that
///   capture its own capture locations. It carries the nested [`Captures`] of
///   this occurrence.
/// * [`CaptureOccurrence::Literal`] is a group or capture that contains no
///   capture locations of its own, so only its matched text is retained.
/// * [`CaptureOccurrence::Alternative`] is an alternative expression (`or`,
///   `any_of`  and their named variants). It records which branch was taken
///   (zero-based, in the order the branches were added) and carries the
///   captures of that branch only.
///
/// A group or capture is a `Group` if and only if its inner scope declares at
/// least one capture location (group, capture, alternative or nested regex).
/// Nested regexes (including recursive references) are always groups, and
/// alternatives are always `Alternative`, even if they declare no captures
/// themselves.
#[derive(Debug, PartialEq)]
pub enum CaptureOccurrence {
    /// An occurrence with a nested capture scope.
    Group {
        /// The complete text consumed by this occurrence, including text
        /// matched by expressions that are not individually addressable.
        matched: String,
        /// The captures declared inside this occurrence.
        nested: Captures,
    },

    /// An occurrence without nested capture locations.
    Literal(String),

    /// An occurrence of an alternative expression.
    Alternative {
        /// The complete text consumed by the taken branch.
        matched: String,
        /// The zero-based index of the branch that was taken.
        branch_index: usize,
        /// The captures declared inside the taken branch. Every branch is a
        /// scope of its own, so captures of other branches never appear here.
        nested: Captures,
    },
}

impl CaptureOccurrence {
    /// Returns the complete text matched by this occurrence.
    ///
    /// Works for every variant. For repeated captures, each occurrence returns
    /// only its own text, never a concatenation of all occurrences.
    pub fn matched_text(&self) -> &str {
        match self {
            CaptureOccurrence::Group { matched, .. }
            | CaptureOccurrence::Literal(matched)
            | CaptureOccurrence::Alternative { matched, .. } => matched,
        }
    }

    /// Returns the nested captures if this is a `Group`, otherwise `None`.
    pub fn try_group(&self) -> Option<&Captures> {
        match self {
            CaptureOccurrence::Group { nested, .. } => Some(nested),
            _ => None,
        }
    }

    /// Returns the matched text if this is a `Literal`, otherwise `None`.
    pub fn try_literal(&self) -> Option<&str> {
        match self {
            CaptureOccurrence::Literal(text) => Some(text),
            _ => None,
        }
    }

    /// Returns the taken branch index and the captures of that branch if this
    /// is an `Alternative`, otherwise `None`.
    pub fn try_alternative(&self) -> Option<(usize, &Captures)> {
        match self {
            CaptureOccurrence::Alternative {
                branch_index,
                nested,
                ..
            } => Some((*branch_index, nested)),
            _ => None,
        }
    }

    /// Returns the zero-based index of the taken branch if this is an
    /// `Alternative`, otherwise `None`.
    pub fn taken_branch(&self) -> Option<usize> {
        self.try_alternative().map(|(branch_index, _)| branch_index)
    }

    /// Returns the nested captures of a `Group`.
    ///
    /// # Panics
    ///
    /// Panics if this is not a `Group`. Use [`CaptureOccurrence::try_group`]
    /// for a non-panicking alternative.
    pub fn expect_group(&self) -> &Captures {
        self.try_group()
            .expect("expect_group() called on an occurrence that is not a Group")
    }

    /// Returns the matched text of a `Literal`.
    ///
    /// # Panics
    ///
    /// Panics if this is not a `Literal`. Use [`CaptureOccurrence::try_literal`]
    /// for a non-panicking alternative, or [`CaptureOccurrence::matched_text`]
    /// which works for every variant.
    pub fn expect_literal(&self) -> &str {
        self.try_literal()
            .expect("expect_literal() called on an occurrence that is not a Literal")
    }

    /// Returns the taken branch index and the captures of that branch of an
    /// `Alternative`.
    ///
    /// # Panics
    ///
    /// Panics if this is not an `Alternative`. Use
    /// [`CaptureOccurrence::try_alternative`] for a non-panicking alternative.
    pub fn expect_alternative(&self) -> (usize, &Captures) {
        self.try_alternative()
            .expect("expect_alternative() called on an occurrence that is not an Alternative")
    }
}

/// The capture results of one regex scope.
///
/// Unnamed groups, unnamed alternatives and unnamed nested regexes share a
/// single zero-based index space. Named captures, named alternatives and named
/// nested regexes share a single name namespace. Both accessors return the
/// same kind of result: the ordered occurrences of that capture location.
///
/// Accessors distinguish three situations:
///
/// * `None`: the location does not exist in this scope (index out of range or
///   unknown name).
/// * `Some(&[])`: the location exists but its route was not taken.
/// * `Some(&[..])`: one entry per occurrence, in order of production.
#[derive(Debug, PartialEq)]
pub struct Captures {
    indexed: Vec<Vec<CaptureOccurrence>>,
    by_name: HashMap<String, Vec<CaptureOccurrence>>,
}

impl Captures {
    /// Returns the occurrences of the nth unnamed group, unnamed alternative,
    /// or unnamed nested regex in this scope, counting from zero.
    pub fn at_index(&self, index: usize) -> Option<&[CaptureOccurrence]> {
        self.indexed.get(index).map(Vec::as_slice)
    }

    /// Returns the occurrences of a named capture, named alternative or named
    /// nested regex in this scope.
    pub fn with_name(&self, name: impl AsRef<str>) -> Option<&[CaptureOccurrence]> {
        self.by_name.get(name.as_ref()).map(Vec::as_slice)
    }
}

// ---------------------------------------------------------------------------
// Internal match bookkeeping
// ---------------------------------------------------------------------------

/// A capture location within a single scope.
///
/// Groups, captures, alternatives and nested regexes all use this type, which
/// is what gives them a shared index space and a shared name namespace.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum CaptureSlot {
    /// A location identified by its zero-based index.
    Index(usize),

    /// A location identified by a user-defined name.
    Name(String),
}

/// A sequence of possible match states for a pattern: end position plus the
/// captures collected on the way there.
type MatchStates = Vec<(usize, ScopeRecord)>;

/// The recursions currently being evaluated, keyed by (cell address, input
/// position). An entry that is looked up again while it is still being
/// evaluated is a left recursion; it is answered with the entry's seed.
type RecursionMemo = HashMap<(usize, usize), SeedEntry>;

/// Evaluation state of one recursion at one input position.
struct SeedEntry {
    /// The matches found so far. Starts empty and grows until it reaches a
    /// fixed point.
    seed: MatchStates,

    /// Whether the seed was consulted during the current evaluation, i.e.
    /// whether the recursion is left-recursive here.
    used: bool,
}

/// Captures collected for one scope while evaluating a pattern.
///
/// Each slot maps to its occurrences in production order. Slots that were
/// never reached simply have no entry; they are materialized as empty vectors
/// when the final [`Captures`] is built.
#[derive(Debug, Clone, Default)]
struct ScopeRecord {
    occurrences: HashMap<CaptureSlot, Vec<RecordedOccurrence>>,
}

/// One recorded occurrence of a slot.
#[derive(Debug, Clone)]
struct RecordedOccurrence {
    /// Half-open character span `[start, end)` consumed by this occurrence.
    /// Retaining it lets `matched_text()` return the exact matched text.
    char_span: (usize, usize),

    /// What kind of occurrence this is, together with its nested scope if it
    /// has one.
    kind: RecordedKind,
}

/// The kind of recorded occurrence.
#[derive(Debug, Clone)]
enum RecordedKind {
    /// A capture without nested capture locations: only the text is needed.
    Literal,

    /// A group-like occurrence with a nested scope.
    Group(NestedScope),

    /// An alternative occurrence: the taken branch and that branch's scope.
    Alternative {
        branch_index: usize,
        scope: NestedScope,
    },
}

/// The nested scope belonging to one group or alternative occurrence.
#[derive(Debug, Clone)]
struct NestedScope {
    /// Captures recorded inside this occurrence only.
    record: Box<ScopeRecord>,

    /// Describes which locations exist in this scope.
    layout: Arc<ScopeLayout>,
}

/// Static description of the capture locations declared in one scope.
///
/// It is what lets untaken routes be reported as empty vectors: every
/// declared location is known even when it never matched.
#[derive(Debug, Default)]
struct ScopeLayout {
    /// Number of index slots: unnamed groups, alternatives and nested regexes.
    index_count: usize,

    /// Names of named captures, alternatives and nested regexes (one shared
    /// namespace).
    declared_names: Vec<String>,
}

impl ScopeLayout {
    /// True if the scope declares no capture locations at all.
    fn declares_nothing(&self) -> bool {
        self.index_count == 0 && self.declared_names.is_empty()
    }
}

/// Counts the distinct end positions among a set of match states.
fn distinct_ends(states: &MatchStates) -> usize {
    states
        .iter()
        .map(|(end, _)| *end)
        .collect::<HashSet<_>>()
        .len()
}

/// Records one occurrence of a recursive reference for every state of the
/// recursion's body, on top of the capture data of the current scope.
fn wrap_recursive(
    slot: &CaptureSlot,
    position: usize,
    states: MatchStates,
    layout: &Arc<ScopeLayout>,
    scope_data: &ScopeRecord,
) -> MatchStates {
    states
        .into_iter()
        .map(|(end, inner_data)| {
            let mut updated = scope_data.clone();

            updated
                .occurrences
                .entry(slot.clone())
                .or_default()
                .push(RecordedOccurrence {
                    char_span: (position, end),
                    kind: RecordedKind::Group(NestedScope {
                        record: Box::new(inner_data),
                        layout: layout.clone(),
                    }),
                });

            (end, updated)
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Recursive regexes
// ---------------------------------------------------------------------------

/// Write-once storage for the body of a recursive regex.
struct RecursiveCell {
    definition: OnceLock<RecursiveDefinition>,
}

/// The compiled body of a recursive regex.
struct RecursiveDefinition {
    root: PatternNode,
    layout: Arc<ScopeLayout>,
}

// Manual impl: the body may contain this very cell, so deriving `Debug`
// would recurse forever.
impl fmt::Debug for RecursiveCell {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("RecursiveCell")
            .field("defined", &self.definition.get().is_some())
            .finish()
    }
}

/// A reference to a regex that may refer to itself.
///
/// Created with [`RegexBuilder::recursive`], embedded with
/// [`RegexBuilder::recurse`] or [`RegexBuilder::named_recurse`] (even before
/// it is defined) and finished exactly once with [`RecursiveRegex::define`].
///
/// An occurrence of a recursive reference is always a
/// [`CaptureOccurrence::Group`] whose nested captures are those of the
/// recursive regex's own top-level scope.
///
/// Left recursion (for example `expr = expr "+" digit | digit`) is supported:
/// the recursion is re-evaluated until it stops reaching new end positions.
#[derive(Debug)]
pub struct RecursiveRegex {
    cell: Arc<RecursiveCell>,
}

impl RecursiveRegex {
    /// Defines the recursive regex and returns it as a [`Regex`].
    ///
    /// Consumes the reference, so it can only be defined once. The builder
    /// may embed this reference (or other recursive references) directly or
    /// indirectly. Other references may still be undefined here; using the
    /// result to build or match is an error until they are defined.
    ///
    /// Because this consumes the reference, build the `builder` first and
    /// then call `define`; the builder only borrows the reference while it
    /// is being constructed.
    ///
    /// # Errors
    ///
    /// Returns any error recorded by the builder, or a scope validation error
    /// such as [`BuildError::DuplicateName`].
    pub fn define<T: BuilderState>(
        self,
        builder: RegexBuilder<T>,
    ) -> Result<Regex, BuildError> {
        let (root, layout) = builder.finish()?;
        let layout = Arc::new(layout);

        // `self` is consumed, so the cell cannot have been defined before.
        let _ = self.cell.definition.set(RecursiveDefinition {
            root: root.clone(),
            layout: layout.clone(),
        });

        Ok(Regex { root, layout })
    }
}

// ---------------------------------------------------------------------------
// Pattern tree
// ---------------------------------------------------------------------------

/// A single item inside a set expression.
#[derive(Debug, Clone)]
enum SetItem {
    /// A literal set of characters.
    Chars(Vec<char>),

    /// A character range.
    Range(char, char),
}

impl SetItem {
    /// Returns true if the given character belongs to this set item.
    fn accepts(&self, ch: char) -> bool {
        match self {
            SetItem::Chars(chars) => chars.contains(&ch),
            SetItem::Range(from, to) => *from <= ch && ch <= *to,
        }
    }
}

/// Pattern nodes used to build a regex.
#[derive(Debug, Clone)]
enum PatternNode {
    /// A literal string pattern.
    Literal(Vec<char>),

    /// A set or negated set matcher.
    CharClass { items: Vec<SetItem>, negated: bool },

    /// A border matcher such as word boundary style checks.
    Boundary { items: Vec<SetItem>, negated: bool },

    /// A concatenation of multiple subpatterns.
    Sequence(Vec<PatternNode>),

    /// A list of alternative branches. The alternative is a capture location
    /// ([`CaptureSlot::Index`] or [`CaptureSlot::Name`]) and every branch is a
    /// scope of its own.
    Alternation {
        slot: CaptureSlot,
        branches: Vec<PatternNode>,
        /// Describes the scope of each branch, one entry per branch. Empty
        /// placeholder until `build` compiles the pattern and fills in the
        /// real values.
        branch_layouts: Vec<Arc<ScopeLayout>>,
    },

    /// A group ([`CaptureSlot::Index`]) or capture ([`CaptureSlot::Name`]). Its
    /// inner pattern is a scope of its own.
    Capturing {
        slot: CaptureSlot,
        inner: Box<PatternNode>,
        /// Describes the inner scope. A default placeholder until `build`
        /// compiles the pattern and fills in the real value.
        layout: Arc<ScopeLayout>,
    },

    /// An embedded, already compiled, reusable regex (a scope of its own).
    Embedded {
        slot: CaptureSlot,
        inner: Box<PatternNode>,
        layout: Arc<ScopeLayout>,
    },

    /// A reference to a recursive regex (a scope of its own). The body and
    /// its layout are looked up in the cell at match time.
    Recursive {
        slot: CaptureSlot,
        cell: Arc<RecursiveCell>,
    },

    /// A quantified subpattern.
    Repeat {
        inner: Box<PatternNode>,
        min: usize,
        max: Option<usize>,
        lazy: bool,
    },
}

impl PatternNode {
    /// Attempts to run this pattern from `position` in the input.
    ///
    /// Returns every way the pattern can match, in priority order, each with
    /// the capture data collected so far for the *current* scope. `memo`
    /// tracks the recursions currently being evaluated.
    fn match_at(
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

    /// Returns true if this pattern depends on a recursive reference that is
    /// not defined yet.
    ///
    /// Definitions are followed transitively; `seen` holds the cells already
    /// visited, which also breaks cycles.
    fn has_undefined(&self, seen: &mut HashSet<*const RecursiveCell>) -> bool {
        match self {
            PatternNode::Literal(_)
            | PatternNode::CharClass { .. }
            | PatternNode::Boundary { .. } => false,

            PatternNode::Sequence(nodes) => nodes.iter().any(|node| node.has_undefined(seen)),

            PatternNode::Alternation { branches, .. } => {
                branches.iter().any(|branch| branch.has_undefined(seen))
            }

            PatternNode::Capturing { inner, .. }
            | PatternNode::Embedded { inner, .. }
            | PatternNode::Repeat { inner, .. } => inner.has_undefined(seen),

            PatternNode::Recursive { cell, .. } => match cell.definition.get() {
                None => true,
                Some(definition) => {
                    seen.insert(Arc::as_ptr(cell)) && definition.root.has_undefined(seen)
                }
            },
        }
    }

    /// Validates and numbers one scope.
    ///
    /// Checks for duplicate names among the locations declared directly in
    /// this scope, assigns indexes, and recursively compiles the scopes of
    /// nested groups, captures and alternative branches. Returns the compiled
    /// pattern together with the layout describing this scope.
    fn compile_scope(self) -> Result<(PatternNode, ScopeLayout), BuildError> {
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

// ---------------------------------------------------------------------------
// Set and alternative builders
// ---------------------------------------------------------------------------

/// A builder for a character class.
pub struct CharSetBuilder {
    /// The raw character items in the set.
    items: Vec<SetItem>,

    /// The first error produced while building the character class.
    error: Option<BuildError>,
}

impl CharSetBuilder {
    /// Adds a literal string to the current set.
    pub fn literal(mut self, chars: impl Into<String>) -> CharSetBuilder {
        self.items
            .push(SetItem::Chars(chars.into().chars().collect()));
        self
    }

    /// Adds a range to the current set.
    ///
    /// If `from > to`, the builder records an invalid range error instead of
    /// silently producing nonsense.
    pub fn range(mut self, from: char, to: char) -> CharSetBuilder {
        if from > to {
            self.error = self.error.or(Some(BuildError::InvalidRange { from, to }));
        } else {
            self.items.push(SetItem::Range(from, to));
        }
        self
    }
}

/// A builder for a sequence of alternative branches.
pub struct AlternativesBuilder {
    /// The branches in the alternative expression.
    branches: Vec<PatternNode>,

    /// The first error encountered in any branch.
    error: Option<BuildError>,
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
fn add_literal_branches<T: Into<String>>(
    builder: AlternativesBuilder,
    options: Vec<T>,
) -> AlternativesBuilder {
    options.into_iter().fold(builder, |builder, option| {
        builder.branch(|branch| branch.literal(option))
    })
}

// ---------------------------------------------------------------------------
// Regex builder
// ---------------------------------------------------------------------------

/// A stateful builder for regex patterns.
#[derive(Debug)]
pub struct RegexBuilder<S: BuilderState> {
    /// The sequence of compiled pattern nodes currently assembled.
    nodes: Vec<PatternNode>,

    /// The first validation error encountered while building.
    error: Option<BuildError>,

    /// Phantom state marker used to enforce the builder state machine.
    _state: PhantomData<fn() -> S>,
}

impl RegexBuilder<AppendOnly> {
    /// Creates a new regex builder in the append-only state.
    pub fn new() -> RegexBuilder<AppendOnly> {
        RegexBuilder {
            nodes: vec![],
            error: None,
            _state: PhantomData,
        }
    }

    /// Creates an undefined recursive regex reference.
    ///
    /// The reference can be embedded with [`RegexBuilder::recurse`] before it
    /// is defined. Define it exactly once with [`RecursiveRegex::define`].
    /// Using a regex that depends on a still undefined reference to build or
    /// match is a [`BuildError::UndefinedRecursion`] error.
    pub fn recursive() -> RecursiveRegex {
        RecursiveRegex {
            cell: Arc::new(RecursiveCell {
                definition: OnceLock::new(),
            }),
        }
    }
}

/// A fully compiled regex pattern with its scope layout.
#[derive(Debug, Clone)]
pub struct Regex {
    /// The compiled pattern tree, with all indexes assigned.
    root: PatternNode,

    /// Describes the capture locations of the top-level scope.
    layout: Arc<ScopeLayout>,
}

impl Regex {
    /// Fails if this regex depends on a recursive reference that has not
    /// been defined yet.
    fn ensure_defined(&self) -> Result<(), BuildError> {
        if self.root.has_undefined(&mut HashSet::new()) {
            Err(BuildError::UndefinedRecursion)
        } else {
            Ok(())
        }
    }

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

/// Builds the public [`Captures`] for one scope.
///
/// Every location declared in `layout` is materialized: locations that never
/// matched become empty vectors, which is what distinguishes an untaken route
/// from an occurrence that matched the empty string.
fn build_captures(record: &ScopeRecord, chars: &[char], layout: &ScopeLayout) -> Captures {
    let occurrences_of = |slot: CaptureSlot| {
        record
            .occurrences
            .get(&slot)
            .map(|recorded| {
                recorded
                    .iter()
                    .map(|occurrence| build_occurrence(occurrence, chars))
                    .collect()
            })
            .unwrap_or_default()
    };

    Captures {
        indexed: (0..layout.index_count)
            .map(|index| occurrences_of(CaptureSlot::Index(index)))
            .collect(),
        by_name: layout
            .declared_names
            .iter()
            .map(|name| {
                (
                    name.clone(),
                    occurrences_of(CaptureSlot::Name(name.clone())),
                )
            })
            .collect(),
    }
}

/// Converts one recorded occurrence into its public form.
///
/// The text is sliced from the exact span the occurrence consumed, so
/// `matched_text()` is correct per occurrence and includes everything matched
/// inside a group.
fn build_occurrence(occurrence: &RecordedOccurrence, chars: &[char]) -> CaptureOccurrence {
    let (start, end) = occurrence.char_span;
    let matched = chars[start..end].iter().collect();

    match &occurrence.kind {
        RecordedKind::Literal => CaptureOccurrence::Literal(matched),
        RecordedKind::Group(scope) => CaptureOccurrence::Group {
            matched,
            nested: build_captures(&scope.record, chars, &scope.layout),
        },
        RecordedKind::Alternative {
            branch_index,
            scope,
        } => CaptureOccurrence::Alternative {
            matched,
            branch_index: *branch_index,
            nested: build_captures(&scope.record, chars, &scope.layout),
        },
    }
}

impl<S: BuilderState> RegexBuilder<S> {
    /// Converts the current builder into a different state type while preserving
    /// all assembled nodes and validation state.
    fn transition<T: BuilderState>(self) -> RegexBuilder<T> {
        RegexBuilder {
            nodes: self.nodes,
            error: self.error,
            _state: PhantomData,
        }
    }

    /// Stores the first error encountered while building the regex.
    fn note_error(&mut self, error: Option<BuildError>) {
        self.error = self.error.take().or(error);
    }

    /// Adds a pattern node and advances the builder into the quantifiable state.
    fn append_node(mut self, node: PatternNode) -> RegexBuilder<Quantifiable> {
        self.nodes.push(node);
        self.transition()
    }

    /// Builds a nested pattern, preserving the outer builder state while wrapping
    /// the inner result in a given pattern structure.
    fn append_nested<T: BuilderState>(
        self,
        build_inner: impl FnOnce(RegexBuilder<AppendOnly>) -> RegexBuilder<T>,
        wrap_inner: impl FnOnce(PatternNode) -> PatternNode,
    ) -> RegexBuilder<Quantifiable> {
        let inner = build_inner(RegexBuilder::new());
        let mut next = self.append_node(wrap_inner(PatternNode::Sequence(inner.nodes)));
        next.note_error(inner.error);
        next
    }

    /// Builds a character-set based pattern and tracks any validation error.
    fn append_set(
        self,
        build_set: impl FnOnce(CharSetBuilder) -> CharSetBuilder,
        negated: bool,
    ) -> RegexBuilder<Quantifiable> {
        let set = build_set(CharSetBuilder {
            items: vec![],
            error: None,
        });

        let mut next = self.append_node(PatternNode::CharClass {
            items: set.items,
            negated,
        });

        next.note_error(set.error);
        next
    }

    /// Builds a border pattern and converts the builder back to append-only mode.
    fn append_border(
        self,
        build_set: impl FnOnce(CharSetBuilder) -> CharSetBuilder,
        negated: bool,
    ) -> RegexBuilder<AppendOnly> {
        let set = build_set(CharSetBuilder {
            items: vec![],
            error: None,
        });

        let mut next = self.append_node(PatternNode::Boundary {
            items: set.items,
            negated,
        });

        next.note_error(set.error);
        next.transition()
    }

    /// Builds an alternative pattern in the given slot and tracks any
    /// validation error.
    fn append_alternatives(
        self,
        slot: CaptureSlot,
        build_alternatives: impl FnOnce(AlternativesBuilder) -> AlternativesBuilder,
    ) -> RegexBuilder<Quantifiable> {
        let alternatives = build_alternatives(AlternativesBuilder {
            branches: vec![],
            error: None,
        });

        let has_no_branches = alternatives.branches.is_empty();

        let mut next = self.append_node(PatternNode::Alternation {
            slot,
            branches: alternatives.branches,
            branch_layouts: vec![], // placeholder, filled in `build`
        });

        next.note_error(alternatives.error);

        if has_no_branches {
            next.note_error(Some(BuildError::EmptyAlternative));
        }

        next
    }

    /// Reports any recorded error, then validates duplicate names and assigns
    /// indexes for the top-level scope. Shared by `build` and `define`.
    fn finish(self) -> Result<(PatternNode, ScopeLayout), BuildError> {
        if let Some(error) = self.error {
            return Err(error);
        }

        PatternNode::Sequence(self.nodes).compile_scope()
    }

    /// Adds a literal text atom to the pattern.
    pub fn literal(self, text: impl Into<String>) -> RegexBuilder<Quantifiable> {
        self.append_node(PatternNode::Literal(text.into().chars().collect()))
    }

    /// Appends a reusable regex without consuming it.
    ///
    /// The nested regex occupies the next free index in the current scope,
    /// sharing the index space with unnamed groups and alternatives. Its own
    /// captures stay isolated inside its result and are reached through the
    /// occurrence.
    pub fn regex(self, regex: &Regex) -> RegexBuilder<Quantifiable> {
        self.append_node(PatternNode::Embedded {
            slot: CaptureSlot::Index(0), // placeholder, assigned in `build`
            inner: Box::new(regex.root.clone()),
            layout: regex.layout.clone(),
        })
    }

    /// Appends a reusable regex under a name.
    ///
    /// The name is registered in the current scope's name namespace together
    /// with named captures, named alternatives and other named nested regexes.
    pub fn named_regex(self, name: impl Into<String>, regex: &Regex) -> RegexBuilder<Quantifiable> {
        self.append_node(PatternNode::Embedded {
            slot: CaptureSlot::Name(name.into()),
            inner: Box::new(regex.root.clone()),
            layout: regex.layout.clone(),
        })
    }

    /// Appends a reference to a recursive regex, which may still be undefined.
    ///
    /// The reference occupies the next free index in the current scope,
    /// sharing the index space with unnamed groups, alternatives and nested
    /// regexes. Its occurrences are always groups carrying the captures of the
    /// recursive regex's top-level scope.
    pub fn recurse(self, recursive: &RecursiveRegex) -> RegexBuilder<Quantifiable> {
        self.append_node(PatternNode::Recursive {
            slot: CaptureSlot::Index(0), // placeholder, assigned in `build`
            cell: recursive.cell.clone(),
        })
    }

    /// Appends a reference to a recursive regex under a name.
    ///
    /// The name is registered in the current scope's name namespace together
    /// with named captures, named alternatives and named nested regexes.
    pub fn named_recurse(
        self,
        name: impl Into<String>,
        recursive: &RecursiveRegex,
    ) -> RegexBuilder<Quantifiable> {
        self.append_node(PatternNode::Recursive {
            slot: CaptureSlot::Name(name.into()),
            cell: recursive.cell.clone(),
        })
    }

    /// Matches any one character in the set produced by the closure.
    pub fn any_char_of(
        self,
        build_set: impl FnOnce(CharSetBuilder) -> CharSetBuilder,
    ) -> RegexBuilder<Quantifiable> {
        self.append_set(build_set, false)
    }

    /// Matches any one character not in the set produced by the closure.
    pub fn any_except(
        self,
        build_set: impl FnOnce(CharSetBuilder) -> CharSetBuilder,
    ) -> RegexBuilder<Quantifiable> {
        self.append_set(build_set, true)
    }

    /// Matches a border around characters in the set produced by the closure.
    pub fn border_of(
        self,
        build_set: impl FnOnce(CharSetBuilder) -> CharSetBuilder,
    ) -> RegexBuilder<AppendOnly> {
        self.append_border(build_set, false)
    }

    /// Matches a non-border position relative to the set produced by the closure.
    pub fn non_border_of(
        self,
        build_set: impl FnOnce(CharSetBuilder) -> CharSetBuilder,
    ) -> RegexBuilder<AppendOnly> {
        self.append_border(build_set, true)
    }

    /// Creates an unnamed group.
    ///
    /// The group takes the next free index in the current scope, sharing the
    /// index space with unnamed alternatives and nested regexes. Everything
    /// declared inside the closure forms a nested scope with its own indexes
    /// and names. Repeating the group records one occurrence per iteration,
    /// each with its own nested captures.
    pub fn group<T: BuilderState>(
        self,
        build_inner: impl FnOnce(RegexBuilder<AppendOnly>) -> RegexBuilder<T>,
    ) -> RegexBuilder<Quantifiable> {
        self.append_nested(build_inner, |inner| PatternNode::Capturing {
            slot: CaptureSlot::Index(0), // placeholder, assigned in `build`
            inner: Box::new(inner),
            layout: Arc::default(), // placeholder, filled in `build`
        })
    }

    /// Creates a named capture.
    ///
    /// Behaves like [`RegexBuilder::group`], but the location is addressed by
    /// name in the shared name namespace of the current scope.
    pub fn capture<T: BuilderState>(
        self,
        name: impl Into<String>,
        build_inner: impl FnOnce(RegexBuilder<AppendOnly>) -> RegexBuilder<T>,
    ) -> RegexBuilder<Quantifiable> {
        self.append_nested(build_inner, |inner| PatternNode::Capturing {
            slot: CaptureSlot::Name(name.into()),
            inner: Box::new(inner),
            layout: Arc::default(), // placeholder, filled in `build`
        })
    }

    /// Creates an unnamed alternative with one or more branches.
    ///
    /// The alternative takes the next free index in the current scope, sharing
    /// the index space with unnamed groups and nested regexes. Each
    /// occurrence records which branch was taken (zero-based), the full text
    /// it consumed and the captures of that branch.
    pub fn or(
        self,
        build_alternatives: impl FnOnce(AlternativesBuilder) -> AlternativesBuilder,
    ) -> RegexBuilder<Quantifiable> {
        self.append_alternatives(CaptureSlot::Index(0), build_alternatives) // placeholder, assigned in `build`
    }

    /// Creates a named alternative with one or more branches.
    ///
    /// Behaves like [`RegexBuilder::or_named`], but the location is addressed by
    /// name in the shared name namespace of the current scope.
    pub fn or_named(
        self,
        name: impl Into<String>,
        build_alternatives: impl FnOnce(AlternativesBuilder) -> AlternativesBuilder,
    ) -> RegexBuilder<Quantifiable> {
        self.append_alternatives(CaptureSlot::Name(name.into()), build_alternatives)
    }

    /// Matches any one of the provided string options.
    ///
    /// Each option is one branch, numbered in the order given, so the result
    /// reports which option matched.
    pub fn any_of<T: Into<String>>(self, options: Vec<T>) -> RegexBuilder<Quantifiable> {
        self.or(|builder| add_literal_branches(builder, options))
    }

    /// Matches any one of the provided string options, under a name.
    ///
    /// Behaves like [`RegexBuilder::any_of`], but the location is addressed by
    /// name in the shared name namespace of the current scope.
    pub fn any_of_named<T: Into<String>>(
        self,
        name: impl Into<String>,
        options: Vec<T>,
    ) -> RegexBuilder<Quantifiable> {
        self.or_named(name, |builder| add_literal_branches(builder, options))
    }

    /// Finalizes the builder and turns it into a compiled regex.
    ///
    /// Reports any error recorded while building, then validates duplicate
    /// names and assigns indexes scope by scope: the top level first, then each
    /// group's, capture's and alternative branch's inner scope independently.
    ///
    /// # Errors
    ///
    /// Also returns [`BuildError::UndefinedRecursion`] if the pattern depends
    /// on a recursive reference that has not been defined yet.
    pub fn build(self) -> Result<Regex, BuildError> {
        let (root, layout) = self.finish()?;

        let regex = Regex {
            root,
            layout: Arc::new(layout),
        };

        regex.ensure_defined()?;

        Ok(regex)
    }
}

impl RegexBuilder<Quantifiable> {
    /// Applies a repetition range to the most recently added expression.
    ///
    /// The `min` value is the minimum number of repetitions, and `max` is the
    /// optional maximum. If `min > max`, an `InvalidQuantifier` error is stored.
    pub fn quantify(mut self, min: usize, max: Option<usize>) -> RegexBuilder<Quantified> {
        self.note_error(
            max.filter(|max| min > *max)
                .map(|max| BuildError::InvalidQuantifier { min, max }),
        );

        let target = self
            .nodes
            .pop()
            .expect("Quantifiable state guarantees an expression to repeat");

        self.nodes.push(PatternNode::Repeat {
            inner: Box::new(target),
            min,
            max,
            lazy: false,
        });

        self.transition()
    }

    /// Matches zero or one repetition of the previous expression.
    pub fn optional(self) -> RegexBuilder<Quantified> {
        self.quantify(0, Some(1))
    }

    /// Matches zero or more repetitions of the previous expression.
    pub fn zero_or_more(self) -> RegexBuilder<Quantified> {
        self.quantify(0, None)
    }

    /// Matches one or more repetitions of the previous expression.
    pub fn one_or_more(self) -> RegexBuilder<Quantified> {
        self.quantify(1, None)
    }

    /// Matches exactly `count` repetitions of the previous expression.
    pub fn exactly(self, count: usize) -> RegexBuilder<Quantified> {
        self.quantify(count, Some(count))
    }

    /// Matches at least `count` repetitions of the previous expression.
    pub fn at_least(self, count: usize) -> RegexBuilder<Quantified> {
        self.quantify(count, None)
    }

    /// Matches at most `count` repetitions of the previous expression.
    pub fn at_most(self, count: usize) -> RegexBuilder<Quantified> {
        self.quantify(0, Some(count))
    }

    /// Matches between `min` and `max` repetitions of the previous expression.
    pub fn between(self, min: usize, max: usize) -> RegexBuilder<Quantified> {
        self.quantify(min, Some(max))
    }
}

impl RegexBuilder<Quantified> {
    /// Makes the last quantified expression lazy instead of greedy.
    pub fn lazy(mut self) -> RegexBuilder<AppendOnly> {
        let Some(PatternNode::Repeat { lazy, .. }) = self.nodes.last_mut() else {
            unreachable!("Quantified state guarantees the last node to be of type Repeat")
        };

        *lazy = true;

        self.transition()
    }
}

impl CharSetBuilder {
    /// Builds a default word-character set using letters, digits, and underscore.
    fn word_chars(self) -> CharSetBuilder {
        self.range('a', 'z')
            .range('A', 'Z')
            .range('0', '9')
            .literal('_')
    }
}

impl<S: BuilderState> RegexBuilder<S> {
    /// Matches any character in the inclusive range from `from` to `to`.
    pub fn range(self, from: char, to: char) -> RegexBuilder<Quantifiable> {
        self.any_char_of(|set| set.range(from, to))
    }

    /// Matches any single character.
    pub fn any_char(self) -> RegexBuilder<Quantifiable> {
        self.any_except(|set| set)
    }

    /// Matches any decimal digit.
    pub fn any_digit(self) -> RegexBuilder<Quantifiable> {
        self.range('0', '9')
    }

    /// Matches any character that is not a decimal digit.
    pub fn any_non_digit(self) -> RegexBuilder<Quantifiable> {
        self.any_except(|set| set.range('0', '9'))
    }

    /// Matches any character considered part of a word.
    pub fn any_word(self) -> RegexBuilder<Quantifiable> {
        self.any_char_of(CharSetBuilder::word_chars)
    }

    /// Matches any character that is not part of a word.
    pub fn any_non_word(self) -> RegexBuilder<Quantifiable> {
        self.any_except(CharSetBuilder::word_chars)
    }

    /// Matches any whitespace character.
    pub fn any_whitespace(self) -> RegexBuilder<Quantifiable> {
        self.any_char_of(|set| set.literal(" \t\r\n\x0B\x0C"))
    }

    /// Matches any non-whitespace character.
    pub fn any_non_whitespace(self) -> RegexBuilder<Quantifiable> {
        self.any_except(|set| set.literal(" \t\r\n\x0B\x0C"))
    }

    /// Matches any hexadecimal digit.
    pub fn any_hex_digit(self) -> RegexBuilder<Quantifiable> {
        self.any_char_of(|set| set.range('0', '9').range('a', 'f').range('A', 'F'))
    }

    /// Matches any character that is not a hexadecimal digit.
    pub fn any_non_hex_digit(self) -> RegexBuilder<Quantifiable> {
        self.any_except(|set| set.range('0', '9').range('a', 'f').range('A', 'F'))
    }

    /// Matches any alphabetic character.
    pub fn any_letter(self) -> RegexBuilder<Quantifiable> {
        self.any_char_of(|set| set.range('a', 'z').range('A', 'Z'))
    }

    /// Matches any lowercase alphabetic character.
    pub fn any_lowercase(self) -> RegexBuilder<Quantifiable> {
        self.range('a', 'z')
    }

    /// Matches any uppercase alphabetic character.
    pub fn any_uppercase(self) -> RegexBuilder<Quantifiable> {
        self.range('A', 'Z')
    }

    /// Matches a word boundary.
    pub fn word_border(self) -> RegexBuilder<AppendOnly> {
        self.border_of(CharSetBuilder::word_chars)
    }

    /// Matches a non-word boundary.
    pub fn non_word_border(self) -> RegexBuilder<AppendOnly> {
        self.non_border_of(CharSetBuilder::word_chars)
    }
}

// ---------------------------------------------------------------------------
// Display
// ---------------------------------------------------------------------------

/// Matched text longer than this is shortened in `{}` output; `{:#}` prints it in full.
const DISPLAY_TEXT_LIMIT: usize = 40;

impl Captures {
    /// True if the scope declares no capture locations at all.
    fn is_empty(&self) -> bool {
        self.indexed.is_empty() && self.by_name.is_empty()
    }
}

/// Quotes and escapes matched text, shortening it unless `full` is set.
fn quoted(text: &str, full: bool) -> String {
    if full || text.chars().count() <= DISPLAY_TEXT_LIMIT {
        format!("{text:?}")
    } else {
        let short: String = text.chars().take(DISPLAY_TEXT_LIMIT - 1).collect();
        format!("{short:?}...")
    }
}

/// Appends the lines describing one scope: indexed locations in order, then
/// named locations alphabetically
fn push_captures(captures: &Captures, depth: usize, full: bool, out: &mut Vec<String>) {
    if captures.is_empty() {
        out.push(format!("{}(no captures)", "  ".repeat(depth)));
        return;
    }

    for (index, occurrences) in captures.indexed.iter().enumerate() {
        push_location(&format!("[{index}]"), occurrences, depth, full, out);
    }

    let mut names: Vec<&String> = captures.by_name.keys().collect();
    names.sort();

    for name in names {
        push_location(&format!("{name:?}"), &captures.by_name[name], depth, full, out);
    }
}

/// Appends the lines describing one capture location and its occurrences.
fn push_location(
    label: &str,
    occurrences: &[CaptureOccurrence],
    depth: usize,
    full: bool,
    out: &mut Vec<String>,
) {
    let pad = "  ".repeat(depth);

    if occurrences.is_empty() {
        out.push(format!("{pad}{label}: not taken"));
        return;
    }

    out.push(format!("{pad}{label}:"));

    for occurrence in occurrences {
        push_occurrence(occurrence, depth + 1, full, out);
    }
}

/// Appends the lines describing one occurrence. Nested scopes without any
/// capture locations are left out, since there is nothing to show.
fn push_occurrence(
    occurrence: &CaptureOccurrence,
    depth: usize,
    full: bool,
    out: &mut Vec<String>
) {
    let pad = "  ".repeat(depth);

    match occurrence {
        CaptureOccurrence::Literal(text) => {
            out.push(format!("{pad}- literal {}", quoted(text, full)));
        }

        CaptureOccurrence::Group { matched, nested } => {
            out.push(format!("{pad}- group {}", quoted(matched, full)));

            if !nested.is_empty() {
                push_captures(nested, depth + 1, full, out);
            }
        }

        CaptureOccurrence::Alternative {
            matched,
            branch_index,
            nested,
        } => {
            out.push(format!(
                "{pad}- branch {branch_index} {}",
                quoted(matched, full)
            ));

            if !nested.is_empty() {
                push_captures(nested, depth + 1, full, out);
            }
        }
    }
}

impl fmt::Display for Captures {
    /// Prints the capture tree. Use `{:#}` to print matched text in full
    /// instead of shortening long text.
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut lines = Vec::new();
        push_captures(self, 0, f.alternate(), &mut lines);
        write!(f, "{}", lines.join("\n"))
    }
}

impl fmt::Display for CaptureOccurrence {
    /// Prints one occurrence and everything nested inside it. Use `{:#}` to
    /// print matched text in full instead of shortening long text.
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut lines = Vec::new();
        push_occurrence(self, 0, f.alternate(), &mut lines);
        write!(f, "{}", lines.join("\n"))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod recursive_tests {
    use super::*;

    #[test]
    fn balanced_parentheses() {
        let expr = RegexBuilder::recursive();
        let body = RegexBuilder::new()
            .literal("(")
            .recurse(&expr)
            .zero_or_more()
            .literal(")");
        let balanced = expr.define(body).unwrap();

        let caps = balanced.full_match("(()())").unwrap().unwrap();
        let inner = caps.at_index(0).unwrap();

        assert_eq!(inner.len(), 2);
        assert_eq!(inner[0].matched_text(), "()");
        assert_eq!(inner[1].matched_text(), "()");
        assert!(inner[0].expect_group().at_index(0).unwrap().is_empty());

        assert!(balanced.full_match("(()").unwrap().is_none());
        assert!(balanced.full_match("").unwrap().is_none());
    }

    #[test]
    fn left_recursion_matches_left_nested_input() {
        let sum = RegexBuilder::recursive();
        let body = RegexBuilder::new().or(|alt| {
            alt.branch(|b| b.recurse(&sum).literal("+").any_digit())
                .branch(|b| b.any_digit())
        });
        let sum = sum.define(body).unwrap();

        assert!(sum.full_match("1").unwrap().is_some());
        assert!(sum.full_match("1+2").unwrap().is_some());
        assert!(sum.full_match("1+2+3").unwrap().is_some());
        assert!(sum.full_match("1+").unwrap().is_none());
        assert!(sum.full_match("+1").unwrap().is_none());
    }

    #[test]
    fn indirect_recursion_through_another_regex() {
        let even = RegexBuilder::recursive();
        let odd = RegexBuilder::recursive();

        // `even` is still undefined here, which `define` allows.
        let odd_body = RegexBuilder::new().literal("a").recurse(&even);
        let odd_regex = odd.define(odd_body).unwrap();

        let even_body = RegexBuilder::new().or(|alt| {
            alt.branch(|b| b.literal(""))
                .branch(|b| b.literal("a").regex(&odd_regex))
        });
        let even_regex = even.define(even_body).unwrap();

        assert!(even_regex.full_match("").unwrap().is_some());
        assert!(even_regex.full_match("aaaa").unwrap().is_some());
        assert!(even_regex.full_match("aaa").unwrap().is_none());
        assert!(odd_regex.full_match("aaa").unwrap().is_some());
    }

    #[test]
    fn undefined_reference_is_an_error() {
        let r = RegexBuilder::recursive();
        let result = RegexBuilder::new().literal("a").recurse(&r).build();

        assert_eq!(result.err(), Some(BuildError::UndefinedRecursion));
    }
}
