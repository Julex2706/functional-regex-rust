use std::collections::{HashMap, HashSet};
use std::fmt;
use std::marker::PhantomData;

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
        }
    }
}

impl std::error::Error for Error {}

/// A single set of capture results produced by a successful match.
///
/// The structure is scoped to one regex level. Unnamed captures and unnamed
/// nested regexes each have their own zero-based index order. Named captures and
/// named nested regexes share the same name namespace at that level.
#[derive(Debug, PartialEq)]
pub struct Captures {
    unnamed: Vec<String>,
    named: HashMap<String, String>,
    unnamed_regexes: Vec<Captures>,
    named_regexes: HashMap<String, Captures>,
}

impl Captures {
    /// Returns the value of the nth unnamed capture, counting from zero.
    pub fn get(&self, index: usize) -> Option<&str> {
        self.unnamed.get(index).map(String::as_str)
    }

    /// Returns the value associated with a named capture.
    pub fn named(&self, name: impl AsRef<str>) -> Option<&str> {
        self.named.get(name.as_ref()).map(String::as_str)
    }

    /// Returns the captures produced by the nth unnamed nested regex.
    pub fn get_regex(&self, index: usize) -> Option<&Captures> {
        self.unnamed_regexes.get(index)
    }

    /// Returns the captures produced by a named nested regex.
    pub fn named_regex(&self, name: impl AsRef<str>) -> Option<&Captures> {
        self.named_regexes.get(name.as_ref())
    }
}

/// A slot used to store a capture span in a single regex scope.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Slot {
    /// A capture identified by its zero-based index.
    Index(usize),

    /// A capture identified by a user-defined name.
    Name(String),
}

/// A slot used to store nested regex match data in a single regex scope.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum RegexSlot {
    /// A nested regex identified by its zero-based index.
    Index(usize),

    /// A nested regex identified by a user-defined name.
    Name(String),
}

/// A map of capture spans keyed by slot.
type Spans = HashMap<Slot, (usize, usize)>;

/// A sequence of possible match states for a pattern.
type Matches = Vec<(usize, MatchData)>;

/// Match data collected while evaluating a regex pattern.
#[derive(Debug, Clone, Default)]
struct MatchData {
    /// Spans captured by the current regex level.
    spans: Spans,

    /// Nested regexes captured by the current regex level.
    regexes: HashMap<RegexSlot, NestedMatch>,
}

/// A nested regex match with its metadata.
#[derive(Debug, Clone)]
struct NestedMatch {
    /// The captured data for the nested regex match.
    data: Box<MatchData>,

    /// The metadata describing the nested regex capture structure.
    metadata: RegexMetadata,
}

/// Static metadata describing the captures and nested regex names for one level.
#[derive(Debug, Clone)]
struct RegexMetadata {
    /// Number of unnamed captures in this regex scope.
    unnamed_count: usize,

    /// Ordered list of named captures in this regex scope.
    names: Vec<String>,

    /// Number of unnamed nested regexes in this regex scope.
    unnamed_regex_count: usize,

    /// Ordered list of named nested regexes in this regex scope.
    regex_names: Vec<String>,
}

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

    /// A capture group around another pattern.
    Capture {
        slot: Slot,
        inner: Box<Pat>,
    },

    /// An embedded, reusable regex.
    Regex {
        slot: RegexSlot,
        inner: Box<Pat>,
        metadata: RegexMetadata,
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

            Pat::Capture { slot, inner } => inner
                .run(s, i, data)
                .into_iter()
                .map(|(end, mut data)| {
                    data.spans.insert(slot.clone(), (i, end));
                    (end, data)
                })
                .collect(),

            Pat::Regex {
                slot,
                inner,
                metadata,
            } => inner
                .run(s, i, MatchData::default())
                .into_iter()
                .map(|(end, inner_data)| {
                    let mut data = data.clone();

                    data.regexes.insert(
                        slot.clone(),
                        NestedMatch {
                            data: Box::new(inner_data),
                            metadata: metadata.clone(),
                        },
                    );

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

    /// Assigns capture and nested regex indexes to the current regex level.
    ///
    /// Numbering is local to the current scope and does not descend into nested
    /// regexes, because each nested regex has its own numbering space.
    fn numbered(self, next: usize, next_regex: usize) -> (Pat, usize, usize) {
        match self {
            Pat::Sequence(items) => {
                let (items, n, r) = Self::numbered_all(items, next, next_regex);
                (Pat::Sequence(items), n, r)
            }

            Pat::Alternatives(items) => {
                let (items, n, r) = Self::numbered_all(items, next, next_regex);
                (Pat::Alternatives(items), n, r)
            }

            Pat::Capture {
                slot: Slot::Name(name),
                inner,
            } => {
                let (inner, n, r) = inner.numbered(next, next_regex);
                (
                    Pat::Capture {
                        slot: Slot::Name(name),
                        inner: Box::new(inner),
                    },
                    n,
                    r,
                )
            }

            Pat::Capture {
                slot: Slot::Index(_),
                inner,
            } => {
                let (inner, n, r) = inner.numbered(next + 1, next_regex);
                (
                    Pat::Capture {
                        slot: Slot::Index(next),
                        inner: Box::new(inner),
                    },
                    n,
                    r,
                )
            }

            Pat::Regex {
                slot: RegexSlot::Name(name),
                inner,
                metadata,
            } => (
                Pat::Regex {
                    slot: RegexSlot::Name(name),
                    inner,
                    metadata,
                },
                next,
                next_regex,
            ),

            Pat::Regex {
                slot: RegexSlot::Index(_),
                inner,
                metadata,
            } => (
                Pat::Regex {
                    slot: RegexSlot::Index(next_regex),
                    inner,
                    metadata,
                },
                next,
                next_regex + 1,
            ),

            Pat::Repeat {
                inner,
                min,
                max,
                lazy,
            } => {
                let (inner, n, r) = inner.numbered(next, next_regex);
                (
                    Pat::Repeat {
                        inner: Box::new(inner),
                        min,
                        max,
                        lazy,
                    },
                    n,
                    r,
                )
            }

            other => (other, next, next_regex),
        }
    }

    /// Numbers all patterns in a vector in order.
    fn numbered_all(items: Vec<Pat>, next: usize, next_regex: usize) -> (Vec<Pat>, usize, usize) {
        items
            .into_iter()
            .fold((vec![], next, next_regex), |(mut done, n, r), item| {
                let (item, n, r) = item.numbered(n, r);
                done.push(item);
                (done, n, r)
            })
    }

    /// Collects all named captures declared at the current regex level.
    ///
    /// This intentionally excludes nested regex contents because nested regexes
    /// keep separate capture namespaces.
    fn capture_names(&self) -> Vec<String> {
        match self {
            Pat::Sequence(items) | Pat::Alternatives(items) => {
                items.iter().flat_map(Pat::capture_names).collect()
            }

            Pat::Capture {
                slot: Slot::Name(name),
                inner,
            } => std::iter::once(name.clone())
                .chain(inner.capture_names())
                .collect(),

            Pat::Capture {
                slot: Slot::Index(_),
                inner,
            } => inner.capture_names(),

            Pat::Repeat { inner, .. } => inner.capture_names(),

            Pat::Literal(_) | Pat::Set { .. } | Pat::Border { .. } | Pat::Regex { .. } => {
                vec![]
            }
        }
    }

    /// Collects all named nested regexes declared at the current regex level.
    fn regex_names(&self) -> Vec<String> {
        match self {
            Pat::Sequence(items) | Pat::Alternatives(items) => {
                items.iter().flat_map(Pat::regex_names).collect()
            }

            Pat::Capture { inner, .. } | Pat::Repeat { inner, .. } => inner.regex_names(),

            Pat::Regex {
                slot: RegexSlot::Name(name),
                ..
            } => vec![name.clone()],

            Pat::Literal(_) | Pat::Set { .. } | Pat::Border { .. } | Pat::Regex { .. } => {
                vec![]
            }
        }
    }
}

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
    /// branch can be built independently.
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
    /// The compiled pattern tree.
    pattern: Pat,

    /// Metadata describing how capture and nested regex slots are numbered.
    metadata: RegexMetadata,
}

impl Regex {
    /// Attempts to match the entire input and returns the captures for the
    /// successful match.
    ///
    /// The regex must consume the whole input string from start to finish. If it
    /// does, the capture data is returned.
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

/// Builds a public `Captures` object from a match result and the regex metadata.
fn make_captures(data: &MatchData, chars: &[char], metadata: &RegexMetadata) -> Captures {
    let text = |slot: Slot| -> String {
        data.spans
            .get(&slot)
            .map(|&(a, b)| chars[a..b].iter().collect())
            .unwrap_or_default()
    };

    let unnamed = (0..metadata.unnamed_count)
        .map(|i| text(Slot::Index(i)))
        .collect();

    let named = metadata
        .names
        .iter()
        .map(|name| (name.clone(), text(Slot::Name(name.clone()))))
        .collect();

    let unnamed_regexes = (0..metadata.unnamed_regex_count)
        .map(|i| {
            data.regexes
                .get(&RegexSlot::Index(i))
                .map(|nested| make_captures(&nested.data, chars, &nested.metadata))
                .unwrap_or_else(|| empty_captures(&data.regexes, i))
        })
        .collect();

    let named_regexes = metadata
        .regex_names
        .iter()
        .filter_map(|name| {
            data.regexes
                .get(&RegexSlot::Name(name.clone()))
                .map(|nested| {
                    (
                        name.clone(),
                        make_captures(&nested.data, chars, &nested.metadata),
                    )
                })
                .or_else(|| None)
        })
        .collect();

    Captures {
        unnamed,
        named,
        unnamed_regexes,
        named_regexes,
    }
}

/// Produces empty capture values for nested regexes that did not participate in
/// the final match.
fn empty_captures(regexes: &HashMap<RegexSlot, NestedMatch>, index: usize) -> Captures {
    match regexes.get(&RegexSlot::Index(index)) {
        Some(nested) => {
            let metadata = &nested.metadata;
            Captures {
                unnamed: vec![String::new(); metadata.unnamed_count],
                named: metadata
                    .names
                    .iter()
                    .map(|name| (name.clone(), String::new()))
                    .collect(),
                unnamed_regexes: (0..metadata.unnamed_regex_count)
                    .map(|i| empty_captures(&nested.data.regexes, i))
                    .collect(),
                named_regexes: metadata
                    .regex_names
                    .iter()
                    .map(|name| {
                        let captures = nested
                            .data
                            .regexes
                            .get(&RegexSlot::Name(name.clone()))
                            .map(|n| make_captures(&n.data, &[], &n.metadata))
                            .unwrap_or_else(|| Captures {
                                unnamed: vec![],
                                named: HashMap::new(),
                                unnamed_regexes: vec![],
                                named_regexes: HashMap::new(),
                            });
                        (name.clone(), captures)
                    })
                    .collect(),
            }
        }
        None => Captures {
            unnamed: vec![],
            named: HashMap::new(),
            unnamed_regexes: vec![],
            named_regexes: HashMap::new(),
        },
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
    /// The nested regex keeps its own capture namespace and does not affect the
    /// outer level's numbering.
    pub fn regex(self, regex: &Regex) -> RegexBuilder<Quantifiable> {
        self.push(Pat::Regex {
            slot: RegexSlot::Index(0),
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
            slot: RegexSlot::Name(name.into()),
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
    pub fn group<T: State>(
        self,
        build: impl FnOnce(RegexBuilder<AppendOnly>) -> RegexBuilder<T>,
    ) -> RegexBuilder<Quantifiable> {
        self.nest(build, |inner| inner)
    }

    /// Creates an unnamed capture group.
    pub fn capture<T: State>(
        self,
        build: impl FnOnce(RegexBuilder<AppendOnly>) -> RegexBuilder<T>,
    ) -> RegexBuilder<Quantifiable> {
        self.nest(build, |inner| Pat::Capture {
            slot: Slot::Index(0),
            inner: Box::new(inner),
        })
    }

    /// Creates a named capture group.
    pub fn capture_named<T: State>(
        self,
        name: impl Into<String>,
        build: impl FnOnce(RegexBuilder<AppendOnly>) -> RegexBuilder<T>,
    ) -> RegexBuilder<Quantifiable> {
        let name = name.into();

        self.nest(build, move |inner| Pat::Capture {
            slot: Slot::Name(name),
            inner: Box::new(inner),
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

        let mut next = self.push(Pat::Alternatives(alternatives.branches));
        next.record(alternatives.error);
        next
    }

    /// Finalizes the builder and turns it into a compiled regex.
    ///
    /// This step validates duplicate names and errors encountered while building
    /// the pattern, then assigns indices for all captures and nested regexes.
    pub fn build(self) -> Result<Regex, Error> {
        if let Some(error) = self.error {
            return Err(error);
        }

        let pattern = Pat::Sequence(self.nodes);
        let names = pattern.capture_names();
        let regex_names = pattern.regex_names();

        let mut seen = HashSet::new();

        for name in names.iter().chain(regex_names.iter()) {
            if !seen.insert(name.as_str()) {
                return Err(Error::DuplicateName(name.clone()));
            }
        }

        let (pattern, unnamed_count, unnamed_regex_count) = pattern.numbered(0, 0);

        Ok(Regex {
            pattern,
            metadata: RegexMetadata {
                unnamed_count,
                names,
                unnamed_regex_count,
                regex_names,
            },
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
