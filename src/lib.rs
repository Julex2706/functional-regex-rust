use std::collections::{HashMap, HashSet};
use std::fmt;
use std::marker::PhantomData;
use std::sync::Arc;

// ---------------------------------------------------------------------------
// Builder states
// ---------------------------------------------------------------------------

/// A regex construction state marker.
///
/// The builder moves through a small state machine:
/// - `AppendOnly`: new atoms can be appended, but no quantifier may be applied yet.
/// - `Quantifiable`: the most recently added atom is eligible to be quantified.
/// - `QuantifiedGreedy`: the last atom has been quantified and can still be made lazy.
pub trait State {}

/// Marks the start of a regex builder. New atoms can be appended, but the
/// pattern cannot yet be quantified.
pub struct AppendOnly;

/// Marks a pattern segment that is ready to receive a quantifier.
pub struct Quantifiable;

/// Marks a quantified segment that may still be converted to a lazy form.
pub struct QuantifiedGreedy;

impl State for AppendOnly {}
impl State for Quantifiable {}
impl State for QuantifiedGreedy {}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Errors produced while constructing or validating a regular expression.
#[derive(Debug, PartialEq, Clone)]
pub enum Error {
    /// Signals that the same capture or regex name was used more than once in
    /// the same regex scope.
    DuplicateName(String),

    /// Signals that a quantifier was built with a minimum greater than the
    /// maximum.
    InvalidQuantifier { min: usize, max: usize },

    /// Signals that a character range had its start after its end.
    InvalidRange { from: char, to: char },

    /// Signals that an alternative expression (`or` / `any_of`) has no branches.
    EmptyAlternative,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::DuplicateName(name) => {
                write!(f, "capture or regex name '{name}' is used more than once")
            }
            Error::InvalidQuantifier { min, max } => {
                write!(
                    f,
                    "quantifier minimum {min} is greater than its maximum {max}"
                )
            }
            Error::InvalidRange { from, to } => {
                write!(f, "range start '{from}' comes after its end '{to}'")
            }
            Error::EmptyAlternative => {
                write!(f, "alternative expression has no branches")
            }
        }
    }
}

impl std::error::Error for Error {}

// ---------------------------------------------------------------------------
// Public capture results
// ---------------------------------------------------------------------------

/// A single occurrence of a capture location.
///
/// The variant describes the *kind* of capture, not whether it matched: both
/// variants always represent a successful match of that occurrence.
///
/// * [`CaptureType::Group`] is a capture (or nested regex) that contains its
///   own capture locations. It carries the nested [`Captures`] of this
///   occurrence.
/// * [`CaptureType::Literal`] is a capture that contains no capture locations
///   of its own, so only its matched text is retained.
///
/// A capture is a `Group` if and only if its inner scope declares at least one
/// capture location (capture or nested regex). Nested regexes are always
/// groups, even if they declare no captures themselves.
#[derive(Debug, PartialEq)]
pub enum CaptureType {
    /// A capture with a nested capture scope.
    Group {
        /// The complete text consumed by this group occurrence, including text
        /// matched by expressions that are not individually addressable.
        full: String,
        /// The captures declared inside this occurrence.
        captures: Captures,
    },

    /// A capture without nested capture locations.
    Literal(String),
}

impl CaptureType {
    /// Returns the complete text matched by this occurrence.
    ///
    /// Works for both variants. For repeated captures, each occurrence returns
    /// only its own text, never a concatenation of all occurrences.
    pub fn full(&self) -> &str {
        match self {
            CaptureType::Group { full, .. } | CaptureType::Literal(full) => full,
        }
    }

    /// Returns the nested captures if this is a `Group`, otherwise `None`.
    pub fn as_group(&self) -> Option<&Captures> {
        match self {
            CaptureType::Group { captures, .. } => Some(captures),
            CaptureType::Literal(_) => None,
        }
    }

    /// Returns the matched text if this is a `Literal`, otherwise `None`.
    pub fn as_literal(&self) -> Option<&str> {
        match self {
            CaptureType::Literal(text) => Some(text),
            CaptureType::Group { .. } => None,
        }
    }

    /// Returns the nested captures of a `Group`.
    ///
    /// # Panics
    ///
    /// Panics if this is a `Literal`. Use [`CaptureType::as_group`] for a
    /// non-panicking alternative.
    pub fn group(&self) -> &Captures {
        self.as_group()
            .expect("group() called on a Literal capture")
    }

    /// Returns the matched text of a `Literal`.
    ///
    /// # Panics
    ///
    /// Panics if this is a `Group`. Use [`CaptureType::as_literal`] for a
    /// non-panicking alternative, or [`CaptureType::full`] which works for both.
    pub fn literal(&self) -> &str {
        self.as_literal()
            .expect("literal() called on a Group capture")
    }
}

/// The capture results of one regex scope.
///
/// Unnamed captures and unnamed nested regexes share a single zero-based index
/// space. Named captures and named nested regexes share a single name
/// namespace. Both accessors return the same kind of result: the ordered
/// occurrences of that capture location.
///
/// Accessors distinguish three situations:
///
/// * `None`: the location does not exist in this scope (index out of range or
///   unknown name).
/// * `Some(&[])`: the location exists but its route was not taken.
/// * `Some(&[..])`: one entry per occurrence, in order of production.
#[derive(Debug, PartialEq)]
pub struct Captures {
    unnamed: Vec<Vec<CaptureType>>,
    named: HashMap<String, Vec<CaptureType>>,
}

impl Captures {
    /// Returns the occurrences of the nth unnamed capture or unnamed nested
    /// regex in this scope, counting from zero.
    pub fn get(&self, index: usize) -> Option<&[CaptureType]> {
        self.unnamed.get(index).map(Vec::as_slice)
    }

    /// Returns the occurrences of a named capture or named nested regex in
    /// this scope.
    pub fn named(&self, name: impl AsRef<str>) -> Option<&[CaptureType]> {
        self.named.get(name.as_ref()).map(Vec::as_slice)
    }
}

// ---------------------------------------------------------------------------
// Internal match bookkeeping
// ---------------------------------------------------------------------------

/// A capture location within a single scope.
///
/// Ordinary captures and nested regexes both use this type, which is what
/// gives them a shared index space and a shared name namespace.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Slot {
    /// A location identified by its zero-based index.
    Index(usize),

    /// A location identified by a user-defined name.
    Name(String),
}

/// A sequence of possible match states for a pattern: end position plus the
/// captures collected on the way there.
type Matches = Vec<(usize, MatchData)>;

/// Captures collected for one scope while evaluating a pattern.
///
/// Each slot maps to its occurrences in production order. Slots that were
/// never reached simply have no entry; they are materialized as empty vectors
/// when the final [`Captures`] is built.
#[derive(Debug, Clone, Default)]
struct MatchData {
    slots: HashMap<Slot, Vec<Occurrence>>,
}

/// One recorded occurrence of a slot.
#[derive(Debug, Clone)]
struct Occurrence {
    /// Half-open character span `[start, end)` consumed by this occurrence.
    /// Retaining it lets `full()` return the exact matched text.
    span: (usize, usize),

    /// The nested scope of this occurrence: `Some` for group-like
    /// occurrences, `None` for literal ones.
    scope: Option<Scope>,
}

/// The nested scope belonging to one group occurrence.
#[derive(Debug, Clone)]
struct Scope {
    /// Captures recorded inside this occurrence only.
    data: Box<MatchData>,

    /// Describes which locations exist in this scope.
    metadata: Arc<RegexMetadata>,
}

/// Static description of the capture locations declared in one scope.
///
/// It is what lets untaken routes be reported as empty vectors: every
/// declared location is known even when it never matched.
#[derive(Debug, Default)]
struct RegexMetadata {
    /// Number of index slots: unnamed captures plus unnamed nested regexes.
    unnamed_count: usize,

    /// Names of named captures and named nested regexes (one shared namespace).
    names: Vec<String>,
}

impl RegexMetadata {
    /// True if the scope declares no capture locations at all.
    fn is_empty(&self) -> bool {
        self.unnamed_count == 0 && self.names.is_empty()
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
    fn contains(&self, c: char) -> bool {
        match self {
            SetItem::Chars(chars) => chars.contains(&c),
            SetItem::Range(from, to) => *from <= c && c <= *to,
        }
    }
}

/// Pattern nodes used to build a regex.
#[derive(Debug, Clone)]
enum Pat {
    /// A literal string pattern.
    Literal(Vec<char>),

    /// A set or negated set matcher.
    Set {
        items: Vec<SetItem>,
        negated: bool,
    },

    /// A border matcher such as word boundary style checks.
    Border {
        items: Vec<SetItem>,
        negated: bool,
    },

    /// A concatenation of multiple subpatterns.
    Sequence(Vec<Pat>),

    /// A list of alternative subpatterns.
    Alternatives(Vec<Pat>),

    /// A capture group. Its inner pattern is a scope of its own.
    Capture {
        slot: Slot,
        inner: Box<Pat>,
        /// Describes the inner scope. A default placeholder until `build`
        /// compiles the pattern and fills in the real value.
        metadata: Arc<RegexMetadata>,
    },

    /// An embedded, already compiled, reusable regex (a scope of its own).
    Regex {
        slot: Slot,
        inner: Box<Pat>,
        metadata: Arc<RegexMetadata>,
    },

    /// A quantified subpattern.
    Repeat {
        inner: Box<Pat>,
        min: usize,
        max: Option<usize>,
        lazy: bool,
    },
}

impl Pat {
    /// Attempts to run this pattern from position `i` in the input.
    ///
    /// Returns every way the pattern can match, in priority order, each with
    /// the capture data collected so far for the *current* scope.
    fn run(&self, s: &[char], i: usize, data: MatchData) -> Matches {
        match self {
            Pat::Literal(l) if s[i..].starts_with(l) => {
                vec![(i + l.len(), data)]
            }
            Pat::Literal(_) => vec![],

            Pat::Set { items, negated } => match s.get(i) {
                Some(c) if items.iter().any(|item| item.contains(*c)) != *negated => {
                    vec![(i + 1, data)]
                }
                _ => vec![],
            },

            Pat::Border { items, negated } => {
                let inside = |c: &char| items.iter().any(|item| item.contains(*c));
                let before = i.checked_sub(1).map_or(false, |p| inside(&s[p]));
                let after = s.get(i).map_or(false, inside);

                if (before != after) != *negated {
                    vec![(i, data)]
                } else {
                    vec![]
                }
            }

            Pat::Sequence(items) => items.iter().fold(vec![(i, data)], |states, item| {
                states
                    .into_iter()
                    .flat_map(|(position, data)| item.run(s, position, data))
                    .collect()
            }),

            Pat::Alternatives(items) => items
                .iter()
                .flat_map(|item| item.run(s, i, data.clone()))
                .collect(),

            // The inner pattern runs against a fresh `MatchData`, which is what
            // makes the capture a separate scope. The finished inner data is
            // attached to a new occurrence appended to this scope's slot, so
            // repeated captures accumulate instead of overwriting.
            Pat::Capture {
                slot,
                inner,
                metadata,
            } => inner
                .run(s, i, MatchData::default())
                .into_iter()
                .map(|(end, inner_data)| {
                    let mut data = data.clone();

                    data.slots
                        .entry(slot.clone())
                        .or_default()
                        .push(Occurrence {
                            span: (i, end),
                            // Group only if the scope declares locations;
                            // otherwise only the text is needed (Literal).
                            scope: (!metadata.is_empty()).then(|| Scope {
                                data: Box::new(inner_data),
                                metadata: metadata.clone(),
                            }),
                        });

                    (end, data)
                })
                .collect(),

            // Nested regexes behave like captures that are always groups.
            Pat::Regex {
                slot,
                inner,
                metadata,
            } => inner
                .run(s, i, MatchData::default())
                .into_iter()
                .map(|(end, inner_data)| {
                    let mut data = data.clone();

                    data.slots
                        .entry(slot.clone())
                        .or_default()
                        .push(Occurrence {
                            span: (i, end),
                            scope: Some(Scope {
                                data: Box::new(inner_data),
                                metadata: metadata.clone(),
                            }),
                        });

                    (end, data)
                })
                .collect(),

            Pat::Repeat {
                inner,
                min,
                max,
                lazy,
            } => inner.repeat(*min, *max, *lazy, s, i, data, 0),
        }
    }

    /// Recursively evaluates a repeated pattern while handling greedy and lazy
    /// expansion rules.
    ///
    /// The capture data is threaded through every iteration, so occurrences of
    /// captures inside the repeated pattern are appended in iteration order.
    fn repeat(
        &self,
        min: usize,
        max: Option<usize>,
        lazy: bool,
        s: &[char],
        i: usize,
        data: MatchData,
        done: usize,
    ) -> Matches {
        let stop_here: Matches = if done >= min {
            vec![(i, data.clone())]
        } else {
            vec![]
        };

        let keep_going: Matches = if max.map_or(true, |m| done < m) {
            self.run(s, i, data)
                .into_iter()
                .filter(|(end, _)| !(*end == i && done >= min))
                .flat_map(|(end, data)| self.repeat(min, max, lazy, s, end, data, done + 1))
                .collect()
        } else {
            vec![]
        };

        if lazy {
            [stop_here, keep_going].concat()
        } else {
            [keep_going, stop_here].concat()
        }
    }

    /// Validates and numbers one scope.
    ///
    /// Checks for duplicate names among the locations declared directly in
    /// this scope, assigns indexes, and recursively compiles the scopes of
    /// nested captures. Returns the compiled pattern together with the
    /// metadata describing this scope.
    fn compile_scope(self) -> Result<(Pat, RegexMetadata), Error> {
        let names = self.scope_names();
        let mut seen = HashSet::new();

        for name in &names {
            if !seen.insert(name.as_str()) {
                return Err(Error::DuplicateName(name.clone()));
            }
        }

        let mut next = 0;
        let pat = self.compile(&mut next)?;

        Ok((
            pat,
            RegexMetadata {
                unnamed_count: next,
                names,
            },
        ))
    }

    /// Assigns indexes within the current scope.
    ///
    /// Unnamed captures and unnamed nested regexes draw from the same counter,
    /// so they share one index space. Nested capture scopes restart their own
    /// numbering; nested regexes are already compiled and are not descended
    /// into.
    fn compile(self, next: &mut usize) -> Result<Pat, Error> {
        Ok(match self {
            Pat::Sequence(items) => Pat::Sequence(Self::compile_all(items, next)?),
            Pat::Alternatives(items) => Pat::Alternatives(Self::compile_all(items, next)?),

            Pat::Capture { slot, inner, .. } => {
                let slot = match slot {
                    Slot::Index(_) => Slot::Index(take_index(next)),
                    named => named,
                };

                let (inner, metadata) = (*inner).compile_scope()?;

                Pat::Capture {
                    slot,
                    inner: Box::new(inner),
                    metadata: Arc::new(metadata),
                }
            }

            Pat::Regex {
                slot,
                inner,
                metadata,
            } => Pat::Regex {
                slot: match slot {
                    Slot::Index(_) => Slot::Index(take_index(next)),
                    named => named,
                },
                inner,
                metadata,
            },

            Pat::Repeat {
                inner,
                min,
                max,
                lazy,
            } => Pat::Repeat {
                inner: Box::new(inner.compile(next)?),
                min,
                max,
                lazy,
            },

            other => other,
        })
    }

    /// Compiles every pattern in a vector, in order, sharing one counter.
    fn compile_all(items: Vec<Pat>, next: &mut usize) -> Result<Vec<Pat>, Error> {
        items.into_iter().map(|item| item.compile(next)).collect()
    }

    /// Collects the names declared directly in the current scope.
    ///
    /// Named captures and named nested regexes share one namespace. Contents
    /// of captures and nested regexes are excluded because each opens its own
    /// scope with its own namespace.
    fn scope_names(&self) -> Vec<String> {
        match self {
            Pat::Sequence(items) | Pat::Alternatives(items) => {
                items.iter().flat_map(Pat::scope_names).collect()
            }

            Pat::Repeat { inner, .. } => inner.scope_names(),

            Pat::Capture {
                slot: Slot::Name(name),
                ..
            }
            | Pat::Regex {
                slot: Slot::Name(name),
                ..
            } => vec![name.clone()],

            _ => vec![],
        }
    }
}

/// Returns the current index and advances the counter.
fn take_index(next: &mut usize) -> usize {
    let index = *next;
    *next += 1;
    index
}

// ---------------------------------------------------------------------------
// Set and alternative builders
// ---------------------------------------------------------------------------

/// A builder for a character class.
pub struct CharSet {
    /// The raw character items in the set.
    items: Vec<SetItem>,

    /// The first error produced while building the character class.
    error: Option<Error>,
}

impl CharSet {
    /// Adds a literal string to the current set.
    pub fn literal(mut self, chars: impl Into<String>) -> CharSet {
        self.items
            .push(SetItem::Chars(chars.into().chars().collect()));
        self
    }

    /// Adds a range to the current set.
    ///
    /// If `from > to`, the builder records an invalid range error instead of
    /// silently producing nonsense.
    pub fn range(mut self, from: char, to: char) -> CharSet {
        if from > to {
            self.error = self.error.or(Some(Error::InvalidRange { from, to }));
        } else {
            self.items.push(SetItem::Range(from, to));
        }
        self
    }
}

/// A builder for a sequence of alternative branches.
pub struct Alternatives {
    /// The branches in the alternative expression.
    branches: Vec<Pat>,

    /// The first error encountered in any branch.
    error: Option<Error>,
}

impl Alternatives {
    /// Adds a branch to the alternative list.
    ///
    /// The provided closure receives a fresh `RegexBuilder<AppendOnly>` so the
    /// branch can be built independently. Captures inside a branch belong to
    /// the scope that contains the alternatives; a branch that is not taken
    /// leaves its captures as empty vectors in the result.
    pub fn branch<T: State>(
        mut self,
        build: impl FnOnce(RegexBuilder<AppendOnly>) -> RegexBuilder<T>,
    ) -> Alternatives {
        let inner = build(RegexBuilder::new());
        self.branches.push(Pat::Sequence(inner.nodes));
        self.error = self.error.or(inner.error);
        self
    }
}

// ---------------------------------------------------------------------------
// Regex builder
// ---------------------------------------------------------------------------

/// A stateful builder for regex patterns.
pub struct RegexBuilder<S: State> {
    /// The sequence of compiled pattern nodes currently assembled.
    nodes: Vec<Pat>,

    /// The first validation error encountered while building.
    error: Option<Error>,

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
}

/// A fully compiled regex pattern with metadata.
#[derive(Debug, Clone)]
pub struct Regex {
    /// The compiled pattern tree, with all indexes assigned.
    pattern: Pat,

    /// Describes the capture locations of the top-level scope.
    metadata: Arc<RegexMetadata>,
}

impl Regex {
    /// Attempts to match the entire input and returns the captures for the
    /// successful match.
    ///
    /// The regex must consume the whole input string from start to finish. If
    /// it does, the hierarchical capture data is returned; otherwise `None`.
    /// When several matches are possible, the first one in greedy/lazy
    /// priority order wins.
    pub fn collect(&self, input: impl Into<String>) -> Option<Captures> {
        let chars: Vec<char> = input.into().chars().collect();

        let data = self
            .pattern
            .run(&chars, 0, MatchData::default())
            .into_iter()
            .find(|(end, _)| *end == chars.len())
            .map(|(_, data)| data)?;

        Some(make_captures(&data, &chars, &self.metadata))
    }
}

/// Builds the public [`Captures`] for one scope.
///
/// Every location declared in `metadata` is materialized: locations that never
/// matched become empty vectors, which is what distinguishes an untaken route
/// from an occurrence that matched the empty string.
fn make_captures(data: &MatchData, chars: &[char], metadata: &RegexMetadata) -> Captures {
    let occurrences = |slot: Slot| -> Vec<CaptureType> {
        data.slots
            .get(&slot)
            .map(|list| list.iter().map(|o| make_occurrence(o, chars)).collect())
            .unwrap_or_default()
    };

    Captures {
        unnamed: (0..metadata.unnamed_count)
            .map(|i| occurrences(Slot::Index(i)))
            .collect(),
        named: metadata
            .names
            .iter()
            .map(|name| (name.clone(), occurrences(Slot::Name(name.clone()))))
            .collect(),
    }
}

/// Converts one recorded occurrence into its public form.
///
/// The text is sliced from the exact span the occurrence consumed, so `full()`
/// is correct per occurrence and includes everything matched inside a group.
fn make_occurrence(occurrence: &Occurrence, chars: &[char]) -> CaptureType {
    let (start, end) = occurrence.span;
    let full: String = chars[start..end].iter().collect();

    match &occurrence.scope {
        Some(scope) => CaptureType::Group {
            full,
            captures: make_captures(&scope.data, chars, &scope.metadata),
        },
        None => CaptureType::Literal(full),
    }
}

impl<S: State> RegexBuilder<S> {
    /// Converts the current builder into a different state type while preserving
    /// all assembled nodes and validation state.
    fn into_state<T: State>(self) -> RegexBuilder<T> {
        RegexBuilder {
            nodes: self.nodes,
            error: self.error,
            _state: PhantomData,
        }
    }

    /// Stores the first error encountered while building the regex.
    fn record(&mut self, error: Option<Error>) {
        self.error = self.error.take().or(error);
    }

    /// Adds a pattern node and advances the builder into the quantifiable state.
    fn push(self, pat: Pat) -> RegexBuilder<Quantifiable> {
        let mut next = self.into_state::<Quantifiable>();
        next.nodes.push(pat);
        next
    }

    /// Builds a nested pattern, preserving the outer builder state while wrapping
    /// the inner result in a given pattern structure.
    fn nest<T: State>(
        self,
        build: impl FnOnce(RegexBuilder<AppendOnly>) -> RegexBuilder<T>,
        wrap: impl FnOnce(Pat) -> Pat,
    ) -> RegexBuilder<Quantifiable> {
        let inner = build(RegexBuilder::new());
        let mut next = self.push(wrap(Pat::Sequence(inner.nodes)));
        next.record(inner.error);
        next
    }

    /// Builds a character-set based pattern and tracks any validation error.
    fn set(
        self,
        build: impl FnOnce(CharSet) -> CharSet,
        negated: bool,
    ) -> RegexBuilder<Quantifiable> {
        let set = build(CharSet {
            items: vec![],
            error: None,
        });

        let mut next = self.push(Pat::Set {
            items: set.items,
            negated,
        });

        next.record(set.error);
        next
    }

    /// Builds a border pattern and converts the builder back to append-only mode.
    fn border(
        self,
        build: impl FnOnce(CharSet) -> CharSet,
        negated: bool,
    ) -> RegexBuilder<AppendOnly> {
        let set = build(CharSet {
            items: vec![],
            error: None,
        });

        let mut next = self
            .push(Pat::Border {
                items: set.items,
                negated,
            })
            .into_state::<AppendOnly>();

        next.record(set.error);
        next
    }

    /// Adds a literal text atom to the pattern.
    pub fn literal(self, text: impl Into<String>) -> RegexBuilder<Quantifiable> {
        self.push(Pat::Literal(text.into().chars().collect()))
    }

    /// Appends a reusable regex without consuming it.
    ///
    /// The nested regex occupies the next free index in the current scope,
    /// sharing the index space with ordinary captures. Its own captures stay
    /// isolated inside its result and are reached through the occurrence.
    pub fn regex(self, regex: &Regex) -> RegexBuilder<Quantifiable> {
        self.push(Pat::Regex {
            slot: Slot::Index(0), // placeholder, assigned in `build`
            inner: Box::new(regex.pattern.clone()),
            metadata: regex.metadata.clone(),
        })
    }

    /// Appends a reusable regex under a name.
    ///
    /// The name is registered in the current scope's name namespace together
    /// with named captures and other named nested regexes.
    pub fn regex_named(self, name: impl Into<String>, regex: &Regex) -> RegexBuilder<Quantifiable> {
        self.push(Pat::Regex {
            slot: Slot::Name(name.into()),
            inner: Box::new(regex.pattern.clone()),
            metadata: regex.metadata.clone(),
        })
    }

    /// Matches any one character in the set produced by the closure.
    pub fn any_char_of(self, build: impl FnOnce(CharSet) -> CharSet) -> RegexBuilder<Quantifiable> {
        self.set(build, false)
    }

    /// Matches any one character not in the set produced by the closure.
    pub fn any_except(self, build: impl FnOnce(CharSet) -> CharSet) -> RegexBuilder<Quantifiable> {
        self.set(build, true)
    }

    /// Matches a border around characters in the set produced by the closure.
    pub fn border_of(self, build: impl FnOnce(CharSet) -> CharSet) -> RegexBuilder<AppendOnly> {
        self.border(build, false)
    }

    /// Matches a non-border position relative to the set produced by the closure.
    pub fn non_border_of(self, build: impl FnOnce(CharSet) -> CharSet) -> RegexBuilder<AppendOnly> {
        self.border(build, true)
    }

    /// Creates a subpattern group without introducing a capture.
    ///
    /// Captures declared inside belong to the enclosing scope.
    pub fn group<T: State>(
        self,
        build: impl FnOnce(RegexBuilder<AppendOnly>) -> RegexBuilder<T>,
    ) -> RegexBuilder<Quantifiable> {
        self.nest(build, |inner| inner)
    }

    /// Creates an unnamed capture group.
    ///
    /// The capture takes the next free index in the current scope. Everything
    /// declared inside the closure forms a nested scope with its own indexes
    /// and names. Repeating the capture records one occurrence per iteration.
    pub fn capture<T: State>(
        self,
        build: impl FnOnce(RegexBuilder<AppendOnly>) -> RegexBuilder<T>,
    ) -> RegexBuilder<Quantifiable> {
        self.nest(build, |inner| Pat::Capture {
            slot: Slot::Index(0), // placeholder, assigned in `build`
            inner: Box::new(inner),
            metadata: Arc::default(), // placeholder, filled in `build`
        })
    }

    /// Creates a named capture group.
    ///
    /// Behaves like [`RegexBuilder::capture`], but the location is addressed by
    /// name in the shared name namespace of the current scope.
    pub fn capture_named<T: State>(
        self,
        name: impl Into<String>,
        build: impl FnOnce(RegexBuilder<AppendOnly>) -> RegexBuilder<T>,
    ) -> RegexBuilder<Quantifiable> {
        let name = name.into();

        self.nest(build, move |inner| Pat::Capture {
            slot: Slot::Name(name),
            inner: Box::new(inner),
            metadata: Arc::default(), // placeholder, filled in `build`
        })
    }

    /// Creates a pattern containing one or more alternative branches.
    pub fn or(
        self,
        build: impl FnOnce(Alternatives) -> Alternatives,
    ) -> RegexBuilder<Quantifiable> {
        let alternatives = build(Alternatives {
            branches: vec![],
            error: None,
        });

        let empty = alternatives.branches.is_empty();

        let mut next = self.push(Pat::Alternatives(alternatives.branches));
        next.record(alternatives.error);

        if empty {
            next.record(Some(Error::EmptyAlternative));
        }

        next
    }

    /// Finalizes the builder and turns it into a compiled regex.
    ///
    /// Reports any error recorded while building, then validates duplicate
    /// names and assigns indexes scope by scope: the top level first, then each
    /// capture group's inner scope independently.
    pub fn build(self) -> Result<Regex, Error> {
        if let Some(error) = self.error {
            return Err(error);
        }

        let (pattern, metadata) = Pat::Sequence(self.nodes).compile_scope()?;

        Ok(Regex {
            pattern,
            metadata: Arc::new(metadata),
        })
    }
}

impl RegexBuilder<Quantifiable> {
    /// Applies a repetition range to the most recently added expression.
    ///
    /// The `min` value is the minimum number of repetitions, and `max` is the
    /// optional maximum. If `min > max`, an `InvalidQuantifier` error is stored.
    pub fn quantify(self, min: usize, max: Option<usize>) -> RegexBuilder<QuantifiedGreedy> {
        let mut next = self.into_state::<QuantifiedGreedy>();

        next.record(
            max.filter(|max| min > *max)
                .map(|max| Error::InvalidQuantifier { min, max }),
        );

        let last = next
            .nodes
            .pop()
            .expect("Quantifiable state guarantees an expression to repeat");

        next.nodes.push(Pat::Repeat {
            inner: Box::new(last),
            min,
            max,
            lazy: false,
        });

        next
    }

    /// Matches zero or one repetition of the previous expression.
    pub fn optional(self) -> RegexBuilder<QuantifiedGreedy> {
        self.quantify(0, Some(1))
    }

    /// Matches zero or more repetitions of the previous expression.
    pub fn zero_or_more(self) -> RegexBuilder<QuantifiedGreedy> {
        self.quantify(0, None)
    }

    /// Matches one or more repetitions of the previous expression.
    pub fn one_or_more(self) -> RegexBuilder<QuantifiedGreedy> {
        self.quantify(1, None)
    }

    /// Matches exactly `n` repetitions of the previous expression.
    pub fn exactly(self, n: usize) -> RegexBuilder<QuantifiedGreedy> {
        self.quantify(n, Some(n))
    }

    /// Matches at least `n` repetitions of the previous expression.
    pub fn at_least(self, n: usize) -> RegexBuilder<QuantifiedGreedy> {
        self.quantify(n, None)
    }

    /// Matches at most `n` repetitions of the previous expression.
    pub fn at_most(self, n: usize) -> RegexBuilder<QuantifiedGreedy> {
        self.quantify(0, Some(n))
    }

    /// Matches between `min` and `max` repetitions of the previous expression.
    pub fn between(self, min: usize, max: usize) -> RegexBuilder<QuantifiedGreedy> {
        self.quantify(min, Some(max))
    }
}

impl RegexBuilder<QuantifiedGreedy> {
    /// Makes the last quantified expression lazy instead of greedy.
    pub fn lazy(self) -> RegexBuilder<AppendOnly> {
        let mut next = self.into_state::<AppendOnly>();

        if let Some(Pat::Repeat { lazy, .. }) = next.nodes.last_mut() {
            *lazy = true;
        }

        next
    }
}

impl CharSet {
    /// Builds a default word-character set using letters, digits, and underscore.
    fn word(self) -> CharSet {
        self.range('a', 'z')
            .range('A', 'Z')
            .range('0', '9')
            .literal("_")
    }
}

impl<S: State> RegexBuilder<S> {
    /// Matches any one of the provided string options.
    pub fn any_of<T: Into<String>>(self, options: Vec<T>) -> RegexBuilder<Quantifiable> {
        self.or(|alts| {
            options
                .into_iter()
                .fold(alts, |alts, option| alts.branch(|r| r.literal(option)))
        })
    }

    /// Matches any character in the inclusive range from `from` to `to`.
    pub fn range(self, from: char, to: char) -> RegexBuilder<Quantifiable> {
        self.any_char_of(|s| s.range(from, to))
    }

    /// Matches any single character.
    pub fn any_char(self) -> RegexBuilder<Quantifiable> {
        self.any_except(|s| s)
    }

    /// Matches any decimal digit.
    pub fn any_digit(self) -> RegexBuilder<Quantifiable> {
        self.range('0', '9')
    }

    /// Matches any character that is not a decimal digit.
    pub fn any_non_digit(self) -> RegexBuilder<Quantifiable> {
        self.any_except(|s| s.range('0', '9'))
    }

    /// Matches any character considered part of a word.
    pub fn any_word(self) -> RegexBuilder<Quantifiable> {
        self.any_char_of(CharSet::word)
    }

    /// Matches any character that is not part of a word.
    pub fn any_non_word(self) -> RegexBuilder<Quantifiable> {
        self.any_except(CharSet::word)
    }

    /// Matches any whitespace character.
    pub fn any_whitespace(self) -> RegexBuilder<Quantifiable> {
        self.any_char_of(|s| s.literal(" \t\r\n\x0B\x0C"))
    }

    /// Matches any non-whitespace character.
    pub fn any_non_whitespace(self) -> RegexBuilder<Quantifiable> {
        self.any_except(|s| s.literal(" \t\r\n\x0B\x0C"))
    }

    /// Matches any hexadecimal digit.
    pub fn any_hex_digit(self) -> RegexBuilder<Quantifiable> {
        self.any_char_of(|s| s.range('0', '9').range('a', 'f').range('A', 'F'))
    }

    /// Matches any character that is not a hexadecimal digit.
    pub fn any_non_hex_digit(self) -> RegexBuilder<Quantifiable> {
        self.any_except(|s| s.range('0', '9').range('a', 'f').range('A', 'F'))
    }

    /// Matches any alphabetic character.
    pub fn any_letter(self) -> RegexBuilder<Quantifiable> {
        self.any_char_of(|s| s.range('a', 'z').range('A', 'Z'))
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
        self.border_of(CharSet::word)
    }

    /// Matches a non-word boundary.
    pub fn non_word_border(self) -> RegexBuilder<AppendOnly> {
        self.non_border_of(CharSet::word)
    }
}
