use std::collections::{HashMap, HashSet};
use std::fmt;
use std::marker::PhantomData;

// ---------- States ----------
//
//  RegexBuilder::new() ─────────────────────────────► AppendOnly
//  any state + literal / any_char_of / any_except /
//              group / capture / capture_named / or ► Quantifiable
//  any state + border_of / non_border_of ──────────► AppendOnly
//  Quantifiable + quantify(min, max) ─────────────► QuantifiedGreedy
//  QuantifiedGreedy + lazy() ─────────────────────► AppendOnly
//  any state + build() ─────────────────────────────► Regex (separate type, see below)
//
//  (All helpers at the bottom are built from these and follow the same transitions.)

pub trait State {}

/// Nothing to quantify: empty, after a border, or the last quantifier was made lazy.
pub struct AppendOnly;
/// The last expression is plain: it can be quantified.
pub struct Quantifiable;
/// The last expression has a greedy quantifier: it can become lazy.
pub struct QuantifiedGreedy;

impl State for AppendOnly {}
impl State for Quantifiable {}
impl State for QuantifiedGreedy {}

// ---------- Errors ----------

/// Everything that can go wrong.
/// `build` reports mistakes in the pattern (if there are several, the first one
/// made, in call order).
#[derive(Debug, PartialEq, Clone)]
pub enum Error {
    /// The same capture name is used by more than one `capture_named`.
    DuplicateName(String),
    /// `quantify(min, max)` with min > max.
    InvalidQuantifier { min: usize, max: usize },
    /// `range(from, to)` with from > to.
    InvalidRange { from: char, to: char },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::DuplicateName(name) => {
                write!(f, "capture name '{name}' is used more than once")
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

// ---------- Result type ----------

/// What `collect` returns.
/// Unnamed captures are indexed in order. Named captures take no index.
/// A capture inside a repetition keeps its LAST value (like Ruby).
/// A capture that did not take part in the match is "".
#[derive(Debug, PartialEq)]
pub struct Captures {
    unnamed: Vec<String>,
    named: HashMap<String, String>,
}

impl Captures {
    /// The n-th unnamed capture (0-based).
    pub fn get(&self, index: usize) -> Option<&str> {
        self.unnamed.get(index).map(String::as_str)
    }

    /// The capture with this name.
    pub fn named(&self, name: impl Into<String>) -> Option<&str> {
        self.named.get(&name.into()).map(String::as_str)
    }
}

// ---------- Pattern tree (private) ----------

/// Where a capture is stored: unnamed ones by index, named ones by name.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Slot {
    Index(usize),
    Name(String),
}

/// Captured spans of one match path. Inserting overrides, so the last capture wins.
type Spans = HashMap<Slot, (usize, usize)>;
/// Every way a pattern can match from a position, best first:
/// (end position, capture spans so far).
type Matches = Vec<(usize, Spans)>;

#[derive(Debug)]
enum SetItem {
    Chars(Vec<char>),
    Range(char, char), // inclusive, from <= to
}

impl SetItem {
    fn contains(&self, c: char) -> bool {
        match self {
            SetItem::Chars(chars) => chars.contains(&c),
            SetItem::Range(from, to) => *from <= c && c <= *to,
        }
    }
}

#[derive(Debug)]
enum Pat {
    Literal(Vec<char>),
    Set {
        items: Vec<SetItem>,
        negated: bool,
    },
    /// Zero-width. Border (\b): exactly one side of the position is in the set.
    /// Non-border (\B, `negated`): both sides or neither side are in the set.
    /// The start and end of the input count as "not in the set".
    Border {
        items: Vec<SetItem>,
        negated: bool,
    },
    Sequence(Vec<Pat>),
    Alternatives(Vec<Pat>),
    /// Unnamed captures get their real index in `numbered`.
    Capture {
        slot: Slot,
        inner: Box<Pat>,
    },
    Repeat {
        inner: Box<Pat>,
        min: usize,
        max: Option<usize>,
        lazy: bool,
    },
}

impl Pat {
    fn run(&self, s: &[char], i: usize, spans: Spans) -> Matches {
        match self {
            Pat::Literal(l) if s[i..].starts_with(l) => vec![(i + l.len(), spans)],
            Pat::Literal(_) => vec![],
            Pat::Set { items, negated } => match s.get(i) {
                Some(c) if items.iter().any(|item| item.contains(*c)) != *negated => {
                    vec![(i + 1, spans)]
                }
                _ => vec![],
            },
            Pat::Border { items, negated } => {
                let inside = |c: &char| items.iter().any(|item| item.contains(*c));
                let before = i.checked_sub(1).map_or(false, |p| inside(&s[p]));
                let after = s.get(i).map_or(false, inside);
                if (before != after) != *negated {
                    vec![(i, spans)]
                } else {
                    vec![]
                }
            }
            Pat::Sequence(items) => items.iter().fold(vec![(i, spans)], |states, item| {
                states
                    .into_iter()
                    .flat_map(|(i, spans)| item.run(s, i, spans))
                    .collect()
            }),
            Pat::Alternatives(items) => items
                .iter()
                .flat_map(|item| item.run(s, i, spans.clone()))
                .collect(),
            Pat::Capture { slot, inner } => inner
                .run(s, i, spans)
                .into_iter()
                .map(|(end, mut spans)| {
                    spans.insert(slot.clone(), (i, end)); // overrides an earlier value
                    (end, spans)
                })
                .collect(),
            Pat::Repeat {
                inner,
                min,
                max,
                lazy,
            } => inner.repeat(*min, *max, *lazy, s, i, spans, 0),
        }
    }

    /// Match `self` repeatedly. `done` = repetitions so far.
    fn repeat(
        &self,
        min: usize,
        max: Option<usize>,
        lazy: bool,
        s: &[char],
        i: usize,
        spans: Spans,
        done: usize,
    ) -> Matches {
        let stop_here: Matches = if done >= min {
            vec![(i, spans.clone())]
        } else {
            vec![]
        };
        let keep_going: Matches = if max.map_or(true, |m| done < m) {
            self.run(s, i, spans)
                .into_iter()
                .filter(|(end, _)| !(*end == i && done >= min)) // no empty loops
                .flat_map(|(end, spans)| self.repeat(min, max, lazy, s, end, spans, done + 1))
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

    /// Give each UNNAMED capture its index by opening order.
    /// Named captures take no index. Returns the next free index.
    fn numbered(self, next: usize) -> (Pat, usize) {
        match self {
            Pat::Sequence(items) => {
                let (items, after) = Pat::numbered_all(items, next);
                (Pat::Sequence(items), after)
            }
            Pat::Alternatives(items) => {
                let (items, after) = Pat::numbered_all(items, next);
                (Pat::Alternatives(items), after)
            }
            Pat::Capture {
                slot: named @ Slot::Name(_),
                inner,
            } => {
                let (inner, after) = (*inner).numbered(next);
                (
                    Pat::Capture {
                        slot: named,
                        inner: Box::new(inner),
                    },
                    after,
                )
            }
            Pat::Capture {
                slot: Slot::Index(_),
                inner,
            } => {
                let (inner, after) = (*inner).numbered(next + 1);
                (
                    Pat::Capture {
                        slot: Slot::Index(next),
                        inner: Box::new(inner),
                    },
                    after,
                )
            }
            Pat::Repeat {
                inner,
                min,
                max,
                lazy,
            } => {
                let (inner, after) = (*inner).numbered(next);
                (
                    Pat::Repeat {
                        inner: Box::new(inner),
                        min,
                        max,
                        lazy,
                    },
                    after,
                )
            }
            other => (other, next),
        }
    }

    fn numbered_all(items: Vec<Pat>, next: usize) -> (Vec<Pat>, usize) {
        items
            .into_iter()
            .fold((vec![], next), |(mut done, n), item| {
                let (item, n) = item.numbered(n);
                done.push(item);
                (done, n)
            })
    }

    /// Every capture name in the pattern, in opening order (duplicates included).
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
            Pat::Literal(_) | Pat::Set { .. } | Pat::Border { .. } => vec![],
        }
    }
}

// ---------- Character set builder (for any_char_of / any_except / borders) ----------

/// The members of a character set. A character matches if it is in ANY of the items.
pub struct CharSet {
    items: Vec<SetItem>,
    error: Option<Error>,
}

impl CharSet {
    /// Add every character of `chars` as a member.
    pub fn literal(mut self, chars: impl Into<String>) -> CharSet {
        self.items
            .push(SetItem::Chars(chars.into().chars().collect()));
        self
    }

    /// Add every character from `from` to `to`, both included.
    /// from > to is an `Error::InvalidRange`, reported by `build`.
    pub fn range(mut self, from: char, to: char) -> CharSet {
        if from > to {
            self.error = self.error.or(Some(Error::InvalidRange { from, to }));
        } else {
            self.items.push(SetItem::Range(from, to));
        }
        self
    }
}

// ---------- Alternatives builder (for `or`) ----------

pub struct Alternatives {
    branches: Vec<Pat>,
    error: Option<Error>,
}

impl Alternatives {
    /// Add one alternative.
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

// ---------- Builder ----------

/// A pattern under construction. It knows nothing about any input.
/// Call `build` to finish it, then `collect` on the result as often as you like.
pub struct RegexBuilder<S: State> {
    nodes: Vec<Pat>,
    error: Option<Error>, // first mistake made while building
    _state: PhantomData<fn() -> S>,
}

impl RegexBuilder<AppendOnly> {
    pub fn new() -> RegexBuilder<AppendOnly> {
        RegexBuilder {
            nodes: vec![],
            error: None,
            _state: PhantomData,
        }
    }
}

// ---------- Finished pattern ----------

/// A checked, finished pattern. A separate type, so none of the builder methods
/// exist on it. It never changes, so `collect` only needs `&self`:
/// build once, collect as many inputs as you want.
#[derive(Debug)]
pub struct Regex {
    pattern: Pat,
    unnamed_count: usize,
    names: Vec<String>,
}

impl Regex {
    /// The whole input must match.
    /// Returns the captures if the pattern matches all of the input.
    pub fn collect(&self, input: impl Into<String>) -> Option<Captures> {
        let chars: Vec<char> = input.into().chars().collect();
        let spans = self
            .pattern
            .run(&chars, 0, Spans::new())
            .into_iter()
            .find(|(end, _)| *end == chars.len())
            .map(|(_, spans)| spans)?;

        let text = |slot: Slot| -> String {
            spans
                .get(&slot)
                .map(|&(a, b)| chars[a..b].iter().collect())
                .unwrap_or_default()
        };
        Some(Captures {
            unnamed: (0..self.unnamed_count)
                .map(|i| text(Slot::Index(i)))
                .collect(),
            named: self
                .names
                .iter()
                .map(|name| (name.clone(), text(Slot::Name(name.clone()))))
                .collect(),
        })
    }
}

// ===== Core: available in every state =====

impl<S: State> RegexBuilder<S> {
    fn into_state<T: State>(self) -> RegexBuilder<T> {
        RegexBuilder {
            nodes: self.nodes,
            error: self.error,
            _state: PhantomData,
        }
    }

    /// Remember a mistake. The first one stays.
    fn record(&mut self, error: Option<Error>) {
        self.error = self.error.take().or(error);
    }

    fn push(self, pat: Pat) -> RegexBuilder<Quantifiable> {
        let mut next = self.into_state::<Quantifiable>();
        next.nodes.push(pat);
        next
    }

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

    /// Borders are zero-width, so there is nothing to quantify afterwards.
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

    pub fn literal(self, text: impl Into<String>) -> RegexBuilder<Quantifiable> {
        self.push(Pat::Literal(text.into().chars().collect()))
    }

    /// One character that is in the set: [...]
    pub fn any_char_of(self, build: impl FnOnce(CharSet) -> CharSet) -> RegexBuilder<Quantifiable> {
        self.set(build, false)
    }

    /// One character that is NOT in the set: [^...]
    /// (`any_except(|s| s)` = any single character)
    pub fn any_except(self, build: impl FnOnce(CharSet) -> CharSet) -> RegexBuilder<Quantifiable> {
        self.set(build, true)
    }

    /// A position where exactly one side is in the set: \b for the word set.
    /// Zero-width. The start and end of the input count as "not in the set".
    pub fn border_of(self, build: impl FnOnce(CharSet) -> CharSet) -> RegexBuilder<AppendOnly> {
        self.border(build, false)
    }

    /// A position where both sides or neither side are in the set: \B for the word set.
    pub fn non_border_of(self, build: impl FnOnce(CharSet) -> CharSet) -> RegexBuilder<AppendOnly> {
        self.border(build, true)
    }

    /// (?:...)
    pub fn group<T: State>(
        self,
        build: impl FnOnce(RegexBuilder<AppendOnly>) -> RegexBuilder<T>,
    ) -> RegexBuilder<Quantifiable> {
        self.nest(build, |inner| inner)
    }

    /// (...)  Takes the next unnamed index.
    pub fn capture<T: State>(
        self,
        build: impl FnOnce(RegexBuilder<AppendOnly>) -> RegexBuilder<T>,
    ) -> RegexBuilder<Quantifiable> {
        self.nest(build, |inner| Pat::Capture {
            slot: Slot::Index(0),
            inner: Box::new(inner),
        })
    }

    /// (?<name>...)  Takes no index. Using a name twice is an `Error::DuplicateName`.
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

    /// (a|b|c): one node, so it can be quantified.
    /// No branches = matches nothing.
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

    /// Finish the pattern. Reports the mistakes made while building it
    /// (the first one made), then duplicate capture names.
    pub fn build(self) -> Result<Regex, Error> {
        if let Some(error) = self.error {
            return Err(error);
        }

        let pattern = Pat::Sequence(self.nodes);
        let names = pattern.capture_names();
        let mut seen = HashSet::new();
        if let Some(duplicate) = names.iter().find(|name| !seen.insert(name.as_str())) {
            return Err(Error::DuplicateName(duplicate.clone()));
        }

        let (pattern, unnamed_count) = pattern.numbered(0);
        Ok(Regex {
            pattern,
            unnamed_count,
            names,
        })
    }
}

// ===== Core: quantifiers =====

impl RegexBuilder<Quantifiable> {
    /// Repeat the last expression `min` to `max` times (None = unbounded). Greedy.
    ///   quantify(0, Some(1)) = ?     quantify(0, None) = *     quantify(1, None) = +
    /// min > max is an `Error::InvalidQuantifier`, reported by `build`.
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
}

impl RegexBuilder<QuantifiedGreedy> {
    /// Make the last quantifier lazy: ??  *?  +?
    pub fn lazy(self) -> RegexBuilder<AppendOnly> {
        let mut next = self.into_state::<AppendOnly>();
        if let Some(Pat::Repeat { lazy, .. }) = next.nodes.last_mut() {
            *lazy = true;
        }
        next
    }
}

// ===== Helpers: built only from the functions above =====

// ----- The word set, shared by \w \W \b \B -----

impl CharSet {
    /// [a-zA-Z0-9_]
    fn word(self) -> CharSet {
        self.range('a', 'z')
            .range('A', 'Z')
            .range('0', '9')
            .literal("_")
    }
}

// ----- Alternatives, ranges, character classes and borders (ASCII only, like Ruby) -----

impl<S: State> RegexBuilder<S> {
    /// (a|b|c) from plain strings.
    pub fn any_of<T: Into<String>>(self, options: Vec<T>) -> RegexBuilder<Quantifiable> {
        self.or(|alts| {
            options
                .into_iter()
                .fold(alts, |alts, o| alts.branch(|r| r.literal(o)))
        })
    }

    /// One character from `from` to `to`, both included: [a-z]
    /// from > to is an `Error::InvalidRange`, reported by `build`.
    pub fn range(self, from: char, to: char) -> RegexBuilder<Quantifiable> {
        self.any_char_of(|s| s.range(from, to))
    }

    /// Any single character, newlines included.
    pub fn any_char(self) -> RegexBuilder<Quantifiable> {
        self.any_except(|s| s)
    }

    /// \d
    pub fn any_digit(self) -> RegexBuilder<Quantifiable> {
        self.range('0', '9')
    }
    /// \D
    pub fn any_non_digit(self) -> RegexBuilder<Quantifiable> {
        self.any_except(|s| s.range('0', '9'))
    }

    /// \w
    pub fn any_word(self) -> RegexBuilder<Quantifiable> {
        self.any_char_of(CharSet::word)
    }
    /// \W
    pub fn any_non_word(self) -> RegexBuilder<Quantifiable> {
        self.any_except(CharSet::word)
    }

    /// \s
    pub fn any_whitespace(self) -> RegexBuilder<Quantifiable> {
        self.any_char_of(|s| s.literal(" \t\r\n\x0B\x0C"))
    }
    /// \S
    pub fn any_non_whitespace(self) -> RegexBuilder<Quantifiable> {
        self.any_except(|s| s.literal(" \t\r\n\x0B\x0C"))
    }

    /// \h
    pub fn any_hex_digit(self) -> RegexBuilder<Quantifiable> {
        self.any_char_of(|s| s.range('0', '9').range('a', 'f').range('A', 'F'))
    }
    /// \H
    pub fn any_non_hex_digit(self) -> RegexBuilder<Quantifiable> {
        self.any_except(|s| s.range('0', '9').range('a', 'f').range('A', 'F'))
    }

    /// [a-zA-Z]
    pub fn any_letter(self) -> RegexBuilder<Quantifiable> {
        self.any_char_of(|s| s.range('a', 'z').range('A', 'Z'))
    }
    /// [a-z]
    pub fn any_lowercase(self) -> RegexBuilder<Quantifiable> {
        self.range('a', 'z')
    }
    /// [A-Z]
    pub fn any_uppercase(self) -> RegexBuilder<Quantifiable> {
        self.range('A', 'Z')
    }

    /// \b
    pub fn word_border(self) -> RegexBuilder<AppendOnly> {
        self.border_of(CharSet::word)
    }
    /// \B
    pub fn non_word_border(self) -> RegexBuilder<AppendOnly> {
        self.non_border_of(CharSet::word)
    }
}

// ----- Quantifier shortcuts (all greedy; add .lazy() for the lazy version) -----

impl RegexBuilder<Quantifiable> {
    /// ?
    pub fn optional(self) -> RegexBuilder<QuantifiedGreedy> {
        self.quantify(0, Some(1))
    }
    /// *
    pub fn zero_or_more(self) -> RegexBuilder<QuantifiedGreedy> {
        self.quantify(0, None)
    }
    /// +
    pub fn one_or_more(self) -> RegexBuilder<QuantifiedGreedy> {
        self.quantify(1, None)
    }
    /// {n}
    pub fn exactly(self, n: usize) -> RegexBuilder<QuantifiedGreedy> {
        self.quantify(n, Some(n))
    }
    /// {n,}
    pub fn at_least(self, n: usize) -> RegexBuilder<QuantifiedGreedy> {
        self.quantify(n, None)
    }
    /// {0,n}
    pub fn at_most(self, n: usize) -> RegexBuilder<QuantifiedGreedy> {
        self.quantify(0, Some(n))
    }
    /// {min,max}
    pub fn between(self, min: usize, max: usize) -> RegexBuilder<QuantifiedGreedy> {
        self.quantify(min, Some(max))
    }
}
