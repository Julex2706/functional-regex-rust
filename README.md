# Regex Builder

A Rust library for constructing regular expressions with an explicit builder API. It supports literal patterns, character sets, captures, named captures, nested reusable regexes, alternatives, and quantified expressions. The builder validates duplicate names, invalid quantifiers, and invalid ranges before a compiled regex is returned.

## Overview

Regex Builder is designed for structured pattern construction. Instead of writing a raw pattern string, you build the expression using Rust methods. This makes it easier to wire together complex patterns, keep capture names organized, and validate the pattern as it is assembled.

The main entry point is:

```rust
use regex_builder::RegexBuilder;
```

The typical workflow is:

1. Create a builder with `RegexBuilder::new()`
2. Append pattern atoms
3. Add captures, groups, and alternatives as needed
4. Apply quantifiers when appropriate
5. Call `build()`
6. Match the full input with `collect()`

## Features

- Literal string matching
- Character set construction
- Negated character sets
- Capture groups
- Named capture groups
- Alternation branches
- Nested reusable regexes
- Named nested regexes
- Quantifiers
- Lazy repetition
- Full-match capture extraction
- Duplicate-name validation
- Invalid quantifier validation
- Invalid range validation

## Installation

Add the crate to your Cargo manifest:

```toml
[dependencies]
regex_builder = "0.1"
```

Then import it in your Rust code:

```rust
use regex_builder::RegexBuilder;
```

## Core API

### RegexBuilder

The main builder type is `RegexBuilder<S>`, where `S` is a state marker used to enforce valid builder transitions.

### State flow

The builder progresses through a small state machine:

- `AppendOnly`
- `Quantifiable`
- `QuantifiedGreedy`

This ensures that certain methods are only available in the correct stage of pattern construction.

## Basic usage

Create a simple pattern and collect its captures:

```rust
use regex_builder::RegexBuilder;

let regex = RegexBuilder::new()
    .literal("hello")
    .build()
    .unwrap();

let captures = regex.collect("hello").unwrap();
assert_eq!(captures.get(0), Some("hello"));
```

The `collect()` method matches the entire input. A match only succeeds when the pattern consumes the full string:

```rust
let regex = RegexBuilder::new()
    .literal("abc")
    .build()
    .unwrap();

assert!(regex.collect("abc").is_some());
assert!(regex.collect("abcd").is_none());
```

## Building patterns

### Literals

Add literal text to the pattern:

```rust
let regex = RegexBuilder::new()
    .literal("test")
    .build()
    .unwrap();

assert!(regex.collect("test").is_some());
```

### Character sets

Create a character class with `any_char_of()`:

```rust
let regex = RegexBuilder::new()
    .any_char_of(|s| s.range('a', 'z'))
    .build()
    .unwrap();

assert!(regex.collect("m").is_some());
assert!(regex.collect("9").is_none());
```

Exclude characters with `any_except()`:

```rust
let regex = RegexBuilder::new()
    .any_except(|s| s.range('0', '9'))
    .build()
    .unwrap();

assert!(regex.collect("a").is_some());
assert!(regex.collect("7").is_none());
```

### Character set builder

The character set builder supports:

- `literal(...)`
- `range(from, to)`

Example:

```rust
let regex = RegexBuilder::new()
    .any_char_of(|s| {
        s.literal("aeiou")
            .range('0', '9')
    })
    .build()
    .unwrap();

assert!(regex.collect("a").is_some());
assert!(regex.collect("7").is_some());
assert!(regex.collect("z").is_none());
```

### Common character helpers

The library includes convenience methods for common patterns:

- `any_char()`
- `any_digit()`
- `any_non_digit()`
- `any_word()`
- `any_non_word()`
- `any_whitespace()`
- `any_non_whitespace()`
- `any_hex_digit()`
- `any_non_hex_digit()`
- `any_letter()`
- `any_lowercase()`
- `any_uppercase()`

Example:

```rust
let regex = RegexBuilder::new()
    .any_digit()
    .build()
    .unwrap();

assert!(regex.collect("5").is_some());
assert!(regex.collect("A").is_none());
```

### Borders

Use border checks to match word-like transitions:

```rust
let regex = RegexBuilder::new()
    .word_border()
    .build()
    .unwrap();

assert!(regex.collect("").is_some());
```

The library also provides:

- `border_of(...)`
- `non_border_of(...)`
- `word_border()`
- `non_word_border()`

## Captures

The `collect()` method returns a `Captures` value representing matched groups.

### Unnamed captures

Create an unnamed capture with `capture(...)`:

```rust
let regex = RegexBuilder::new()
    .capture(|r| r.literal("abc"))
    .build()
    .unwrap();

let captures = regex.collect("abc").unwrap();
assert_eq!(captures.get(0), Some("abc"));
```

Unnamed captures are numbered from zero.

### Named captures

Create a named capture with `capture_named(name, ...)`:

```rust
let regex = RegexBuilder::new()
    .capture_named("value", |r| r.literal("abc"))
    .build()
    .unwrap();

let captures = regex.collect("abc").unwrap();
assert_eq!(captures.named("value"), Some("abc"));
```

Named captures are stored in the same regex scope as other named captures and named nested regexes.

### Capture access

The `Captures` type exposes the following accessors:

- `get(index) -> Option<&str>`
- `named(name) -> Option<&str>`
- `get_regex(index) -> Option<&Captures>`
- `named_regex(name) -> Option<&Captures>`

Example:

```rust
let regex = RegexBuilder::new()
    .capture_named("first", |r| r.literal("abc"))
    .capture_named("second", |r| r.literal("def"))
    .build()
    .unwrap();

let captures = regex.collect("abcdef").unwrap();

assert_eq!(captures.named("first"), Some("abc"));
assert_eq!(captures.named("second"), Some("def"));
```

## Nested regexes

Reusable regexes can be inserted into another pattern with `regex(...)`:

```rust
let inner = RegexBuilder::new()
    .literal("abc")
    .build()
    .unwrap();

let regex = RegexBuilder::new()
    .regex(&inner)
    .build()
    .unwrap();

let captures = regex.collect("abc").unwrap();
assert!(captures.get_regex(0).is_some());
```

The nested regex keeps its own capture namespace and numbering space.

### Named nested regexes

A nested regex can also be assigned a name:

```rust
let inner = RegexBuilder::new()
    .literal("abc")
    .build()
    .unwrap();

let regex = RegexBuilder::new()
    .regex_named("nested", &inner)
    .build()
    .unwrap();

let captures = regex.collect("abc").unwrap();
assert!(captures.named_regex("nested").is_some());
```

## Alternatives

Create alternatives with `or(...)`:

```rust
let regex = RegexBuilder::new()
    .or(|alts| {
        alts.branch(|r| r.literal("cat"))
            .branch(|r| r.literal("dog"))
    })
    .build()
    .unwrap();

assert!(regex.collect("cat").is_some());
assert!(regex.collect("dog").is_some());
assert!(regex.collect("bird").is_none());
```

The `Alternatives` builder allows multiple `branch(...)` entries.

## Groups

Use `group(...)` to add a grouping without creating a capture:

```rust
let regex = RegexBuilder::new()
    .group(|r| r.literal("abc"))
    .build()
    .unwrap();

assert!(regex.collect("abc").is_some());
```

This is useful when you need to structure a pattern without affecting capture indexes.

## Quantifiers

Quantifiers are applied to the most recently appended expression.

### Supported quantifiers

- `quantify(min, max)`
- `optional()`
- `zero_or_more()`
- `one_or_more()`
- `exactly(n)`
- `at_least(n)`
- `at_most(n)`
- `between(min, max)`

Example:

```rust
let regex = RegexBuilder::new()
    .literal("a")
    .one_or_more()
    .build()
    .unwrap();

assert!(regex.collect("a").is_some());
assert!(regex.collect("aaa").is_some());
assert!(regex.collect("").is_none());
```

Example with bounded repetition:

```rust
let regex = RegexBuilder::new()
    .literal("ab")
    .between(1, 3)
    .build()
    .unwrap();

assert!(regex.collect("ab").is_some());
assert!(regex.collect("abab").is_some());
assert!(regex.collect("ababab").is_some());
assert!(regex.collect("").is_none());
```

### Lazy quantifiers

Make the last quantified pattern lazy:

```rust
let regex = RegexBuilder::new()
    .literal("a")
    .one_or_more()
    .lazy()
    .build()
    .unwrap();
```

The lazy flag affects the repetition strategy for the last quantified atom.

## Full-match behavior

The library evaluates patterns against the entire input. This is a full-match engine, not a substring matcher.

```rust
let regex = RegexBuilder::new()
    .literal("abc")
    .build()
    .unwrap();

assert!(regex.collect("abc").is_some());
assert!(regex.collect("xabc").is_none());
assert!(regex.collect("ab").is_none());
```

This design is useful when validating exact content, such as email fragments, numeric strings, or structured tokens.

## Error handling

The builder returns a `Result<Regex, Error>` from `build()`.

### Error types

```rust
#[derive(Debug, PartialEq, Clone)]
pub enum Error {
    DuplicateName(String),
    InvalidQuantifier { min: usize, max: usize },
    InvalidRange { from: char, to: char },
}
```

### Duplicate names

Duplicate names are rejected in the same scope:

```rust
let result = RegexBuilder::new()
    .capture_named("value", |r| r.literal("a"))
    .capture_named("value", |r| r.literal("b"))
    .build();

assert!(result.is_err());
```

### Invalid quantifier

A minimum greater than a maximum is not allowed:

```rust
let result = RegexBuilder::new()
    .literal("a")
    .quantify(5, Some(3))
    .build();

assert!(result.is_err());
```

### Invalid range

Character ranges must be ordered correctly:

```rust
let result = RegexBuilder::new()
    .any_char_of(|s| s.range('z', 'a'))
    .build();

assert!(result.is_err());
```

## Examples

### Example: letters only

```rust
let regex = RegexBuilder::new()
    .capture(|r| r.any_letter().one_or_more())
    .build()
    .unwrap();

assert!(regex.collect("hello").is_some());
assert!(regex.collect("123").is_none());
```

### Example: digits only

```rust
let regex = RegexBuilder::new()
    .capture(|r| r.any_digit().one_or_more())
    .build()
    .unwrap();

assert!(regex.collect("123").is_some());
assert!(regex.collect("abc").is_none());
```

### Example: named value

```rust
let regex = RegexBuilder::new()
    .capture_named("number", |r| r.any_digit().one_or_more())
    .build()
    .unwrap();

let captures = regex.collect("42").unwrap();
assert_eq!(captures.named("number"), Some("42"));
```

### Example: alternation

```rust
let regex = RegexBuilder::new()
    .or(|alts| {
        alts.branch(|r| r.literal("red"))
            .branch(|r| r.literal("blue"))
            .branch(|r| r.literal("green"))
    })
    .build()
    .unwrap();

assert!(regex.collect("red").is_some());
assert!(regex.collect("blue").is_some());
assert!(regex.collect("green").is_some());
assert!(regex.collect("yellow").is_none());
```

## Design notes

### Scope isolation

Nested regexes keep their own capture namespace and do not merge into the parent regex capture list.

This prevents accidental name collisions and keeps nested subpatterns independent.

### Capture numbering

Unnamed captures are assigned in order of appearance within a regex scope. Nested regexes maintain their own numbering separately.

### Validation during construction

This builder validates common invalid states early. That reduces runtime confusion and makes invalid patterns fail at compile-style construction time instead of later during evaluation.

## Summary

Regex Builder is a structured, validation-aware regex construction library for Rust. It provides a controlled path from pattern assembly to capture extraction while enforcing correct state transitions and naming rules. It is suited for builders that require precise capture handling, nested regex reuse, and explicit validation before execution.
