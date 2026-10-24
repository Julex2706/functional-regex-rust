# RegexBuilder

A dependency-free regular expression builder for Rust.

RegexBuilder provides a type-state API for constructing regular expression patterns through method calls. Patterns support literal text, character classes, alternatives, groups, named and unnamed captures, quantifiers, and character-set boundaries.

Calling `build()` validates the pattern and produces a `Regex`. The resulting pattern can be reused to match multiple inputs and retrieve captured text.

## Features

- No external dependencies.
- Type-state API for compile-time enforcement of valid builder operations.
- Literal string matching.
- Character classes and inclusive character ranges.
- Alternatives and nested groups.
- Named and unnamed captures.
- Greedy and lazy quantifiers.
- Custom character-set boundaries.
- Build-time validation of invalid ranges, invalid quantifiers, and duplicate capture names.
- Whole-input matching.
- Reusable, immutable compiled patterns.

## Contents

- Installation
- Basic usage
- Architecture
- Builder states
- Literals
- Character sets
- Character classes
- Alternatives
- Groups
- Captures
- Quantifiers
- Greedy and lazy matching
- Boundaries
- Building patterns
- Matching
- Capture results
- Errors
- Matching semantics
- Limitations
- API reference

## Installation

Add the library to Cargo.toml using its package name:

```toml
[dependencies]
regex_builder = { path = "../regex_builder" }
```

Replace the path with the library’s location, or use the appropriate version declaration when the package is published.

The examples below assume the public API is exposed through the `regex` module:

```rust
use regex_builder::regex::RegexBuilder;
```

## Basic usage

The following example matches a literal prefix followed by one or more ASCII word characters and captures the latter.

```rust
use regex_builder::regex::RegexBuilder;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let regex = RegexBuilder::new()
        .literal("user=")
        .capture_named("username", |r| r.any_word().one_or_more())
        .build()?;
    let captures = regex
        .collect("user=alice_42")
        .expect("input must match");
    assert_eq!(captures.named("username"), Some("alice_42"));
    assert!(regex.collect("user=").is_none());
    assert!(regex.collect("user=alice!").is_none());
    Ok(())
}
```

The pattern consists of two expressions:

1. The literal `user=`.
2. A named capture containing one or more ASCII word characters.

`build()` returns a `Result<Regex, Error>`. `collect()` returns `Some(Captures)` if the entire input matches and `None` otherwise.

## Architecture

The library separates pattern construction, validation, and matching into distinct types.

| Type | Responsibility |
| --- | --- |
| `RegexBuilder<S>` | Constructs a pattern using a type-state parameter |
| `AppendOnly` | Marks a builder whose current expression cannot be quantified |
| `Quantifiable` | Marks a builder whose last expression can be quantified |
| `QuantifiedGreedy` | Marks a builder whose last quantifier can be made lazy |
| `CharSet` | Constructs character sets |
| `Alternatives` | Constructs ordered alternatives |
| `Regex` | Stores a finished pattern and matches inputs |
| `Captures` | Stores captured text |
| `Error` | Represents pattern construction errors |

The builder stores an internal pattern tree. Each operation appends an expression or wraps the last expression in a larger expression. During `build()`, the pattern is validated and unnamed captures are assigned their final indices.

The finished `Regex` stores the pattern tree, the number of unnamed captures, and the list of named captures.

## Builder states

The builder uses a generic state parameter to restrict available operations at compile time.

### State definitions

| State | Meaning |
| --- | --- |
| `AppendOnly` | No expression is currently eligible for quantification |
| `Quantifiable` | The last expression can be quantified |
| `QuantifiedGreedy` | The last expression has a greedy quantifier and can be made lazy |

### State transitions

| Operation | Input state | Output state |
| --- | --- | --- |
| `RegexBuilder::new()` | `None` | `AppendOnly` |
| `literal()` | Any state | `Quantifiable` |
| `any_char_of()` | Any state | `Quantifiable` |
| `any_except()` | Any state | `Quantifiable` |
| `group()` | Any state | `Quantifiable` |
| `capture()` | Any state | `Quantifiable` |
| `capture_named()` | Any state | `Quantifiable` |
| `or()` | Any state | `Quantifiable` |
| `border_of()` | Any state | `AppendOnly` |
| `non_border_of()` | Any state | `AppendOnly` |
| `quantify()` | `Quantifiable` | `QuantifiedGreedy` |
| `lazy()` | `QuantifiedGreedy` | `AppendOnly` |
| `build()` | Any state | `Result<Regex, Error>` |

Character-class helpers, range helpers, and quantifier shortcuts follow the transitions of the underlying methods.

### Compile-time restrictions

A quantifier applies to the last expression in the builder. A second quantifier cannot be applied until another expression has been appended.

For example, the following does not compile:

```rust
RegexBuilder::new()
    .literal("a")
    .one_or_more()
    .optional();
```

After `one_or_more()`, the builder is in `QuantifiedGreedy`, which does not expose `optional()`.

Likewise, `lazy()` is only available immediately after a greedy quantifier. After calling it, the builder returns to `AppendOnly`.

These restrictions are enforced by Rust’s type system rather than runtime checks.

## Literals

### `literal(text)`

```rust
RegexBuilder::new()
    .literal("hello")
    .build()?;
```

Matches the exact sequence of characters supplied to `literal()`.

The argument implements `Into<String>`. The string is converted into a sequence of Rust `char` values.

Literal text is not interpreted as regular expression syntax. For example:

```rust
RegexBuilder::new()
    .literal(".*+?")
    .build()?;
```

Matches the literal characters `.*+?`.

An empty literal is valid and matches an empty sequence of characters.

## Character sets

`CharSet` represents a collection of character-set items. Each item is either a list of characters or an inclusive range.

A character belongs to the set if it matches at least one item.

### `CharSet::literal(chars)`

Adds every character in the supplied string to the set.

```rust
RegexBuilder::new()
    .any_char_of(|s| s.literal("aeiou"))
    .build()?;
```

This pattern matches exactly one lowercase vowel.

Multiple calls add additional characters:

```rust
RegexBuilder::new()
    .any_char_of(|s| {
        s.literal("abc")
            .literal("xyz")
    })
    .build()?;
```

The resulting set contains `a`, `b`, `c`, `x`, `y`, and `z`.

### `CharSet::range(from, to)`

Adds an inclusive range of characters.

```rust
RegexBuilder::new()
    .any_char_of(|s| s.range('a', 'z'))
    .build()?;
```

The range includes both endpoints.

Multiple ranges can be combined:

```rust
RegexBuilder::new()
    .any_char_of(|s| {
        s.range('a', 'z')
            .range('A', 'Z')
            .range('0', '9')
            .literal("_")
    })
    .build()?;
```

This matches an ASCII letter, an ASCII digit, or an underscore.

If `from > to`, the range is invalid. The builder records an `Error::InvalidRange`, which is returned by `build()`.

Character sets do not require duplicate removal. A character matching multiple items still produces a single successful character match.

### `any_char_of(build)`

Matches one character contained in the constructed set.

```rust
RegexBuilder::new()
    .any_char_of(|s| s.literal("abc"))
    .build()?;
```

### `any_except(build)`

Matches one character that is not contained in the constructed set.

```rust
RegexBuilder::new()
    .any_except(|s| s.literal("abc"))
    .build()?;
```

An empty set excludes no characters. Consequently, `any_except(|s| s)` matches any single character, including newline characters.

## Character classes

The builder provides methods for common character classes. These methods are implemented using the core character-set operations.

| Method | Equivalent set or condition |
| --- | --- |
| `any_char()` | Any single character |
| `any_digit()` | `[0-9]` |
| `any_non_digit()` | Any character outside `[0-9]` |
| `any_word()` | `[a-zA-Z0-9_]` |
| `any_non_word()` | Any character outside `[a-zA-Z0-9_]` |
| `any_whitespace()` | One of six ASCII whitespace characters |
| `any_non_whitespace()` | Any character outside that whitespace set |
| `any_hex_digit()` | `[0-9a-fA-F]` |
| `any_non_hex_digit()` | Any character outside that hexadecimal set |
| `any_letter()` | `[a-zA-Z]` |
| `any_lowercase()` | `[a-z]` |
| `any_uppercase()` | `[A-Z]` |

The six whitespace characters are space, horizontal tab, carriage return, line feed, vertical tab, and form feed.

These classes are ASCII-oriented. For example, `any_digit()` does not match Unicode digit characters outside the range `0` through `9`.

Character-class methods return `RegexBuilder<Quantifiable>` and can therefore be followed by any supported quantifier.

```rust
let regex = RegexBuilder::new()
    .any_digit()
    .exactly(3)
    .build()?;
assert!(regex.collect("123").is_some());
assert!(regex.collect("12a").is_none());
assert!(regex.collect("１２３").is_none());
```

The last input contains full-width Unicode digits, which are not members of `[0-9]`.

## Alternatives

Alternatives represent a choice between multiple expressions.

### `or(build)`

The `or()` method accepts a closure that constructs an `Alternatives` value.

```rust
let regex = RegexBuilder::new()
    .or(|alts| {
        alts.branch(|r| r.literal("cat"))
            .branch(|r| r.literal("dog"))
            .branch(|r| r.literal("fox"))
    })
    .build()?;
```

Each `branch()` accepts a closure that builds an expression starting from a fresh `RegexBuilder<AppendOnly>`.

Branches may contain nested groups, captures, character classes, quantifiers, and further alternatives.

The resulting alternatives are represented by a single pattern node and can be quantified as a whole.

### `any_of(options)`

`any_of()` is a convenience method for alternatives consisting of literal strings.

```rust
let regex = RegexBuilder::new()
    .any_of(vec!["cat", "dog", "fox"])
    .build()?;
```

The method accepts a `Vec<T>` where `T: Into<String>`.

Internally, it constructs alternatives using `or()` and `branch()`.

### Alternative ordering

Alternatives are evaluated in declaration order. The engine retains match paths in that order, so earlier alternatives are preferred when multiple alternatives can match.

### Empty alternatives

An `or()` with no branches matches nothing.

```rust
let regex = RegexBuilder::new()
    .or(|alts| alts)
    .build()?;
assert!(regex.collect("").is_none());
```

A branch containing no expressions matches the empty string.

```rust
let regex = RegexBuilder::new()
    .or(|alts| alts.branch(|r| r))
    .build()?;
assert!(regex.collect("").is_some());
```

## Groups

### `group(build)`

Creates a non-capturing group.

```rust
let regex = RegexBuilder::new()
    .group(|r| {
        r.literal("ab")
            .or(|alts| {
                alts.branch(|r| r.literal("cd"))
                    .branch(|r| r.literal("ef"))
            })
    })
    .one_or_more()
    .build()?;
```

The group combines the inner pattern into one expression. A quantifier applied after the group affects the entire grouped expression.

Groups do not create capture entries.

The inner builder starts in `AppendOnly`. The resulting inner nodes are wrapped in a `Pat::Sequence`, which is then inserted into the outer pattern.

## Captures

Captures store the text matched by a subexpression. The library supports unnamed captures and named captures.

### `capture(build)`

Creates an unnamed capture.

```rust
let regex = RegexBuilder::new()
    .literal("hello ")
    .capture(|r| r.any_word().one_or_more())
    .build()?;
let captures = regex.collect("hello world").unwrap();
assert_eq!(captures.get(0), Some("world"));
```

Unnamed captures are numbered from zero in opening order. Nested captures are numbered according to their opening order as well.

```rust
let regex = RegexBuilder::new()
    .capture(|r| {
        r.literal("a")
            .capture(|r| r.literal("b"))
    })
    .build()?;
let captures = regex.collect("ab").unwrap();
assert_eq!(captures.get(0), Some("ab"));
assert_eq!(captures.get(1), Some("b"));
```

The outer capture opens first and receives index `0`. The inner capture receives index `1`.

### `capture_named(name, build)`

Creates a named capture.

```rust
let regex = RegexBuilder::new()
    .literal("name=")
    .capture_named("name", |r| r.any_word().one_or_more())
    .build()?;
let captures = regex.collect("name=alice").unwrap();
assert_eq!(captures.named("name"), Some("alice"));
```

Named captures do not occupy unnamed capture indices.

The name is supplied as a value implementing `Into<String>`.

### Duplicate names

Each named capture must have a unique name within the pattern.

```rust
let result = RegexBuilder::new()
    .capture_named("value", |r| r.literal("a"))
    .capture_named("value", |r| r.literal("b"))
    .build();
assert!(matches!(result, Err(Error::DuplicateName(_))));
```

Duplicate names are detected during `build()`. The returned error contains the duplicated name.

Name comparison uses exact string equality and is case-sensitive.

### Captures in alternatives

All captures defined in the pattern are included in the resulting `Captures`, regardless of which alternative matched.

A capture that does not participate in the successful match is represented by an empty string.

For example, if one branch contains a capture and another branch does not, the capture from the unused branch is returned as `""`.

### Captures in repetitions

When a capture is evaluated repeatedly, each successful evaluation overwrites the previously stored span for that capture. The final result therefore contains the last captured value.

```rust
let regex = RegexBuilder::new()
    .capture(|r| r.any_digit())
    .one_or_more()
    .build()?;
let captures = regex.collect("123").unwrap();
assert_eq!(captures.get(0), Some("3"));
```

This behavior applies to both named and unnamed captures.

## Quantifiers

Quantifiers control how many times the last expression may match. They are available only when the builder is in the `Quantifiable` state.

### Quantifier methods

| Method | Minimum | Maximum |
| --- | --- | --- |
| `optional()` | 0 | 1 |
| `zero_or_more()` | 0 | Unbounded |
| `one_or_more()` | 1 | Unbounded |
| `exactly(n)` | `n` | `n` |
| `at_least(n)` | `n` | Unbounded |
| `at_most(n)` | 0 | `n` |
| `between(min, max)` | `min` | `max` |
| `quantify(min, max)` | `min` | `max`, or unbounded |

### Examples

Optional expression:

```rust
RegexBuilder::new()
    .literal("col")
    .literal("o")
    .optional()
    .literal("r")
    .build()?;
```

One or more digits:

```rust
RegexBuilder::new()
    .any_digit()
    .one_or_more()
    .build()?;
```

Between two and five lowercase letters:

```rust
RegexBuilder::new()
    .any_lowercase()
    .between(2, 5)
    .build()?;
```

Exactly four characters from a set:

```rust
RegexBuilder::new()
    .any_char_of(|s| s.literal("abc"))
    .exactly(4)
    .build()?;
```

### `quantify(min, max)`

The general quantifier accepts a minimum and an optional maximum.

- `Some(max)` specifies an upper bound.
- `None` specifies an unbounded maximum.

```rust
.quantify(0, Some(1)) // Zero or one occurrence
.quantify(0, None)    // Zero or more occurrences
.quantify(1, None)    // One or more occurrences
.quantify(3, Some(3)) // Exactly three occurrences
```

If `min > max`, the builder records an `Error::InvalidQuantifier`. The error is returned by `build()`.

## Greedy and lazy matching

Quantifiers are greedy by default. Greedy repetition tries longer matches before shorter ones.

Calling `lazy()` immediately after a quantifier changes the preference to shorter matches first.

```rust
let greedy = RegexBuilder::new()
    .literal("a")
    .zero_or_more()
    .build()?;
let lazy = RegexBuilder::new()
    .literal("a")
    .zero_or_more()
    .lazy()
    .build()?;
```

The `lazy()` method does not change the repetition bounds. It changes the ordering of the available match paths.

The difference becomes observable when the quantified expression is followed by another expression that must consume part of the input.

```rust
let greedy = RegexBuilder::new()
    .literal("a")
    .zero_or_more()
    .literal("a")
    .literal("b")
    .build()?;
let lazy = RegexBuilder::new()
    .literal("a")
    .zero_or_more()
    .lazy()
    .literal("a")
    .literal("b")
    .build()?;
assert!(greedy.collect("aaaab").is_some());
assert!(lazy.collect("aaaab").is_some());
```

Both patterns accept the entire input. The greedy and lazy quantifiers explore repetition counts in different orders while allowing subsequent expressions to match.

The engine’s ordered match paths determine which successful path is selected and therefore which capture spans are returned.

## Boundaries

Boundaries are zero-width assertions. They test the characters adjacent to a position without consuming input.

The builder supports boundaries defined by arbitrary character sets.

### `border_of(build)`

Matches a position where exactly one adjacent character belongs to the specified set.

```rust
RegexBuilder::new()
    .border_of(|s| s.range('a', 'z'))
    .build()?;
```

A position matches when the preceding character is in the set and the following character is not, or vice versa.

### `non_border_of(build)`

Matches a position where both adjacent characters belong to the set or neither belongs to it.

```rust
RegexBuilder::new()
    .non_border_of(|s| s.range('a', 'z'))
    .build()?;
```

### Word-boundary helpers

| Method | Definition |
| --- | --- |
| `word_border()` | Boundary relative to `[a-zA-Z0-9_]` |
| `non_word_border()` | Non-boundary relative to `[a-zA-Z0-9_]` |

At the beginning and end of the input, the missing adjacent character is treated as outside the set.

```rust
let regex = RegexBuilder::new()
    .word_border()
    .literal("cat")
    .word_border()
    .build()?;
assert!(regex.collect("cat").is_some());
assert!(regex.collect("cat!").is_none());
assert!(regex.collect("scat").is_none());
```

The whole-input matching rule applies. These assertions do not enable searching for a substring inside a larger input.

Boundaries return the builder to `AppendOnly`, because they do not consume characters.

## Building patterns

### `build()`

Consumes the builder and returns:

```rust
Result<Regex, Error>
```

The operation performs the following steps:

1. Returns the first previously recorded construction error, if any.
2. Collects all named capture names in opening order.
3. Detects duplicate capture names.
4. Assigns indices to unnamed captures in opening order.
5. Constructs the finished `Regex`.

A successful build produces an immutable pattern that can be used repeatedly.

```rust
let regex = RegexBuilder::new()
    .literal("id=")
    .any_digit()
    .one_or_more()
    .build()?;
```

The builder cannot be used after it has been consumed by `build()`.

## Matching

### `Regex::collect(input)`

Attempts to match the entire input and returns:

```rust
Option<Captures>
```

- `Some(captures)` indicates a successful full-input match.
- `None` indicates that no complete match exists.

```rust
let regex = RegexBuilder::new()
    .literal("hello")
    .build()?;
assert!(regex.collect("hello").is_some());
assert!(regex.collect("hello!").is_none());
assert!(regex.collect("say hello").is_none());
```

The method accepts an argument implementing `Into<String>`.

The current implementation converts the input into a `Vec<char>` before matching. Pattern positions and capture spans are consequently measured in Unicode scalar values rather than UTF-8 byte offsets.

The finished pattern is not modified by matching and can be reused:

```rust
let regex = RegexBuilder::new()
    .any_digit()
    .one_or_more()
    .build()?;
for input in ["123", "4567", "42"] {
    assert!(regex.collect(input).is_some());
}
```

There is no substring-search operation in the current API. The entire input must match the pattern.

## Capture results

`Captures` stores unnamed and named capture results in separate collections.

```rust
pub struct Captures {
    unnamed: Vec<String>,
    named: HashMap<String, String>,
}
```

Both fields are private.

### `get(index)`

Returns an unnamed capture by zero-based index.

```rust
captures.get(0)
```

The return type is `Option<&str>`. An invalid index returns `None`.

### `named(name)`

Returns a named capture by name.

```rust
captures.named("username")
```

The return type is `Option<&str>`. An unknown name returns `None`.

The method accepts `impl Into<String>`.

### Missing captures

A capture defined in the pattern but not participating in the successful match is represented by an empty string.

This representation does not distinguish between a capture that matched an empty string and a capture that did not participate.

## Errors

The public `Error` enum represents invalid pattern construction.

| Variant | Cause |
| --- | --- |
| `DuplicateName(String)` | A named capture name occurs more than once |
| `InvalidQuantifier { min, max }` | The minimum repetition count exceeds the maximum |
| `InvalidRange { from, to }` | The range’s starting character is greater than its ending character |

### `DuplicateName`

```rust
Error::DuplicateName("value".to_string())
```

Returned when multiple named captures use the same name.

### `InvalidQuantifier`

```rust
Error::InvalidQuantifier { min: 5, max: 2 }
```

Returned when a quantifier specifies a minimum greater than its maximum.

### `InvalidRange`

```rust
Error::InvalidRange {
    from: 'z',
    to: 'a',
}
```

Returned when a character range has reversed endpoints.

### Error ordering

The builder retains the first construction error recorded during pattern construction. Later errors do not replace it.

During `build()`, a recorded construction error takes precedence over duplicate capture name validation.

The `Error` enum implements `Debug`, `Display`, `PartialEq`, `Clone`, and `std::error::Error`.

## Matching semantics

The following behaviors describe the implementation.

| Behavior | Semantics |
| --- | --- |
| Input matching | The entire input must match |
| Alternative ordering | Branches are evaluated in declaration order |
| Greedy repetition | Longer repetition paths are preferred |
| Lazy repetition | Shorter repetition paths are preferred |
| Unnamed capture numbering | Zero-based opening order |
| Named capture numbering | Named captures do not occupy unnamed indices |
| Repeated captures | The last captured span is retained |
| Unmatched captures | Represented by an empty string |
| Character ranges | Inclusive |
| Character classes | ASCII-oriented where specified |
| Boundary positions | The start and end of input count as outside the set |
| Empty literals | Valid and match an empty sequence |
| Empty alternatives | Match nothing |
| Empty branches | Match an empty sequence |

### Repetition and empty matches

Repetition stops extending a match path when the repeated expression consumes no characters and the minimum repetition count has already been satisfied. This prevents unbounded repetition from producing an infinite sequence of empty matches.

## Limitations

The implementation provides a custom regular expression engine with a deliberately defined API. It does not implement the complete syntax or behavior of established regex engines.

The current public API does not include:

- Substring searching.
- Match iteration.
- Replacement operations.
- Lookahead or lookbehind assertions.
- Backreferences.
- A parser for conventional regex pattern strings.
- Unicode-defined word, digit, or whitespace classes.
- Flags for case-insensitive or multiline matching.

The matching engine operates on Unicode scalar values, but its convenience character classes use ASCII definitions. These are separate properties: Unicode input is supported, while the definitions of the predefined classes remain ASCII-oriented.

## API reference

### Construction and matching

| Method | Description | Return type |
| --- | --- | --- |
| `RegexBuilder::new()` | Creates an empty builder | `RegexBuilder<AppendOnly>` |
| `build()` | Validates and finishes a pattern | `Result<Regex, Error>` |
| `Regex::collect(input)` | Matches the entire input | `Option<Captures>` |
| `Captures::get(index)` | Retrieves an unnamed capture | `Option<&str>` |
| `Captures::named(name)` | Retrieves a named capture | `Option<&str>` |

### Expression construction

| Method | Description |
| --- | --- |
| `literal(text)` | Literal text |
| `any_char_of(build)` | One character from a set |
| `any_except(build)` | One character outside a set |
| `group(build)` | Non-capturing group |
| `capture(build)` | Unnamed capture |
| `capture_named(name, build)` | Named capture |
| `or(build)` | Ordered alternatives |
| `any_of(options)` | Alternatives from literal strings |

### Character matching

| Method | Description |
| --- | --- |
| `range(from, to)` | One character in an inclusive range |
| `any_char()` | Any single character |
| `any_digit()` | ASCII digit |
| `any_non_digit()` | Non-digit |
| `any_word()` | ASCII word character |
| `any_non_word()` | Non-word character |
| `any_whitespace()` | ASCII whitespace character |
| `any_non_whitespace()` | Non-whitespace character |
| `any_hex_digit()` | ASCII hexadecimal digit |
| `any_non_hex_digit()` | Non-hexadecimal character |
| `any_letter()` | ASCII letter |
| `any_lowercase()` | ASCII lowercase letter |
| `any_uppercase()` | ASCII uppercase letter |

### Character-set construction

| Method | Description |
| --- | --- |
| `CharSet::literal(chars)` | Adds individual characters |
| `CharSet::range(from, to)` | Adds an inclusive range |

### Boundaries

| Method | Description |
| --- | --- |
| `border_of(build)` | Boundary relative to a custom set |
| `non_border_of(build)` | Non-boundary relative to a custom set |
| `word_border()` | ASCII word boundary |
| `non_word_border()` | ASCII non-word boundary |

### Quantifiers

| Method | Description |
| --- | --- |
| `quantify(min, max)` | General repetition bounds |
| `optional()` | Zero or one occurrence |
| `zero_or_more()` | Zero or more occurrences |
| `one_or_more()` | One or more occurrences |
| `exactly(n)` | Exactly `n` occurrences |
| `at_least(n)` | At least `n` occurrences |
| `at_most(n)` | At most `n` occurrences |
| `between(min, max)` | Between `min` and `max` occurrences |
| `lazy()` | Changes the preceding greedy quantifier to lazy |

### Public types

- `RegexBuilder<S: State>`
- `Regex`
- `Captures`
- `CharSet`
- `Alternatives`
- `State`
- `AppendOnly`
- `Quantifiable`
- `QuantifiedGreedy`
- `Error`

### Error variants

- `Error::DuplicateName(String)`
- `Error::InvalidQuantifier { min, max }`
- `Error::InvalidRange { from, to }`
