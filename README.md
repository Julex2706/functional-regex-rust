# Functional Regex Rust

A small, functional-style regular-expression builder written in Rust. Patterns
are assembled with a typestate builder, compiled with `build`, and matched
against an entire input string with `full_match`. Successful matches include
structured capture results, including repeated captures and the branch chosen
by an alternative.

## Example

```rust
use regex_builder::RegexBuilder;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let id = RegexBuilder::new()
        .literal("id=")
        .capture("number", |part| part.any_digit().one_or_more())
        .build()?;

    if let Some(captures) = id.full_match("id=123")? {
        let number = captures
            .with_name("number")
            .expect("the pattern declares this capture")
            .first()
            .expect("the capture matched");
        println!("{}", number.matched_text());
    }

    Ok(())
}
```

`full_match` only succeeds when the pattern consumes the whole input. It
returns `Ok(None)` when there is no match and
`Err(BuildError::UndefinedRecursion)` if a recursive reference is still
undefined. Construction and validation errors are returned by `build` or
`define`. A capture accessor returns `None` for an unknown name or index, an
empty slice when the declared location was not reached, or the ordered
occurrences when it was matched.

An empty pattern is valid and matches only the empty string:

```rust
use regex_builder::RegexBuilder;

let empty = RegexBuilder::new().build().unwrap();
assert!(empty.full_match("").unwrap().is_some());
assert!(empty.full_match("anything").unwrap().is_none());
```

`build()` validates the pattern and returns a `Result`. `full_match()` also
returns a `Result`, because unresolved recursive references are checked when
matching:

```rust
use regex_builder::{BuildError, RegexBuilder};

let invalid = RegexBuilder::new().any_digit().between(3, 1).build();
assert_eq!(
    invalid.unwrap_err(),
    BuildError::InvalidQuantifier { min: 3, max: 1 }
);

let reference = RegexBuilder::recursive();
let pending = RegexBuilder::new().recurse(&reference).build().unwrap();
assert_eq!(
    pending.full_match("").unwrap_err(),
    BuildError::UndefinedRecursion
);
```

## Literals and quantifiers

Methods append to the pattern in order. A quantifier applies to the expression
immediately before it. Repetitions are greedy by default; `lazy()` changes
that quantifier to prefer fewer repetitions.

For example, the `one_or_more()` below applies only to the digit atom, not to
the preceding literal:

```rust
use regex_builder::RegexBuilder;

let tagged_digits = RegexBuilder::new()
    .literal("id=")
    .any_digit()
    .one_or_more()
    .build()
    .unwrap();
assert!(tagged_digits.full_match("id=12").unwrap().is_some());
assert!(tagged_digits.full_match("id=id=12").unwrap().is_none());
```

```rust
use regex_builder::RegexBuilder;

let code = RegexBuilder::new()
    .any_uppercase()
    .any_hex_digit()
    .exactly(2)
    .build()
    .unwrap();
assert!(code.full_match("A7f").unwrap().is_some());
assert!(code.full_match("a7f").unwrap().is_none());
```

Use `optional`, `zero_or_more`, `one_or_more`, `exactly`, `at_least`,
`at_most`, or `between` for common repetition counts. `quantify(min, max)`
provides the same range directly; an invalid range is returned as
`BuildError::InvalidQuantifier`.

An optional part is useful when a separator or sign may be present:

```rust
use regex_builder::RegexBuilder;

let signed_integer = RegexBuilder::new()
    .literal("-")
    .optional()
    .any_digit()
    .one_or_more()
    .build()
    .unwrap();
assert!(signed_integer.full_match("42").unwrap().is_some());
assert!(signed_integer.full_match("-42").unwrap().is_some());
assert!(signed_integer.full_match("--42").unwrap().is_none());
```

For a lazy repetition, place `.lazy()` immediately after the quantifier.
This example puts two repetitions next to each other, so their different
priorities are visible: greedy gives the first capture both `a`s, while lazy
leaves them for the second capture.

```rust
use regex_builder::RegexBuilder;

let greedy = RegexBuilder::new()
    .capture("first", |b| b.literal("a").zero_or_more())
    .capture("second", |b| b.literal("a").zero_or_more())
    .build()
    .unwrap();
let lazy = RegexBuilder::new()
    .capture("first", |b| b.literal("a").zero_or_more().lazy())
    .capture("second", |b| b.literal("a").zero_or_more())
    .build()
    .unwrap();
let greedy_captures = greedy.full_match("aa").unwrap().unwrap();
let lazy_captures = lazy.full_match("aa").unwrap().unwrap();
assert_eq!(greedy_captures.with_name("first").unwrap()[0].matched_text(), "aa");
assert_eq!(lazy_captures.with_name("first").unwrap()[0].matched_text(), "");
assert_eq!(lazy_captures.with_name("second").unwrap()[0].matched_text(), "aa");
```

## Captures and alternatives

`group` and `capture` create nested capture scopes. `or` and `or_named` create
alternatives whose branches each have their own scope; an alternative
occurrence records the branch index and that branch's captures. Repeating a
capture preserves one occurrence per repetition. Unnamed groups, alternatives,
and embedded regexes share one index space within a scope; named locations
share a separate namespace.

```rust
use regex_builder::RegexBuilder;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let choice = RegexBuilder::new()
        .or_named("kind", |alternatives| {
            alternatives
                .branch(|branch| branch.literal("cat"))
                .branch(|branch| branch.literal("dog"))
        })
        .build()?;

    let captures = choice.full_match("dog")?.expect("matches");
    let occurrence = &captures.with_name("kind").expect("declared")[0];
    assert_eq!(occurrence.taken_branch(), Some(1));
    Ok(())
}
```

`group` creates an unnamed indexed capture scope; `capture` creates the same
kind of scope addressed by name. Their nested captures are retrieved from the
occurrence, not from the surrounding scope:

```rust
use regex_builder::RegexBuilder;

let pair = RegexBuilder::new()
    .group(|pair| {
        pair.capture("letter", |b| b.any_letter())
            .capture("digit", |b| b.any_digit())
    })
    .build()
    .unwrap();
let captures = pair.full_match("a7").unwrap().unwrap();
let group = &captures.at_index(0).unwrap()[0];
assert_eq!(
    group.expect_group().with_name("digit").unwrap()[0].matched_text(),
    "7"
);
```

For repeated groups, each occurrence has its own nested captures:

```rust
use regex_builder::RegexBuilder;

let pairs = RegexBuilder::new()
    .group(|pair| {
        pair.capture("letter", |b| b.any_letter())
            .capture("digit", |b| b.any_digit())
    })
    .one_or_more()
    .build()
    .unwrap();
let captures = pairs.full_match("a1b2").unwrap().unwrap();
let groups = captures.at_index(0).unwrap();
assert_eq!(groups.len(), 2);
assert_eq!(groups[1].expect_group().with_name("letter").unwrap()[0].matched_text(), "b");
```

`any_of` is a convenient alternative made from literal branches. `or_named`
and `any_of_named` address the alternative by name instead of index. Untaken
captures remain visible as empty occurrence slices.

When two branches accept the same text, the earlier branch is preferred:

```rust
use regex_builder::RegexBuilder;

let regex = RegexBuilder::new()
    .or(|branches| {
        branches
            .branch(|b| b.literal("ok"))
            .branch(|b| b.literal("ok"))
    })
    .build()
    .unwrap();
let captures = regex.full_match("ok").unwrap().unwrap();
assert_eq!(captures.at_index(0).unwrap()[0].taken_branch(), Some(0));
```

Use `any_of` when each choice is a literal string rather than a custom branch:

```rust
use regex_builder::RegexBuilder;

let http_method = RegexBuilder::new()
    .any_of_named("method", vec!["GET", "POST", "DELETE"])
    .build()
    .unwrap();
let captures = http_method.full_match("POST").unwrap().unwrap();
assert_eq!(
    captures.with_name("method").unwrap()[0].matched_text(),
    "POST"
);
assert_eq!(captures.with_name("method").unwrap()[0].taken_branch(), Some(1));
```

For example, one branch can contain a capture that does not exist in the
other branch. Look inside the alternative occurrence to read the selected
branch's scope:

```rust
use regex_builder::RegexBuilder;

let value = RegexBuilder::new()
    .or_named("value", |branches| {
        branches
            .branch(|b| b.capture("word", |word| word.any_letter().one_or_more()))
            .branch(|b| b.capture("number", |number| number.any_digit().one_or_more()))
    })
    .build()
    .unwrap();
let captures = value.full_match("123").unwrap().unwrap();
let branch = captures.with_name("value").unwrap()[0].expect_alternative().1;
assert!(branch.with_name("word").is_none());
assert_eq!(branch.with_name("number").unwrap()[0].matched_text(), "123");
```

## Character sets and borders

Character-set closures can combine literal characters and inclusive ranges.
Use `any_except` for the complement of a set; an empty positive set matches
nothing, while an empty negated set accepts any character.

```rust
use regex_builder::RegexBuilder;

let identifier_start = RegexBuilder::new()
    .any_char_of(|set| set.range('a', 'z').range('A', 'Z').literal("_"))
    .build()
    .unwrap();
assert!(identifier_start.full_match("_").unwrap().is_some());
assert!(identifier_start.full_match("4").unwrap().is_none());
```

The set builder can combine several literal strings and ranges. Ranges include
both endpoints, and a reversed range is returned as
`BuildError::InvalidRange`:

```rust
use regex_builder::{BuildError, RegexBuilder};

let hex = RegexBuilder::new()
    .any_char_of(|set| set.range('0', '9').range('a', 'f').range('A', 'F'))
    .build()
    .unwrap();
assert!(hex.full_match("B").unwrap().is_some());
assert!(hex.full_match("g").unwrap().is_none());

let invalid = RegexBuilder::new().range('z', 'a').build();
assert!(matches!(invalid, Err(BuildError::InvalidRange { .. })));
```

If you need to exclude a small set, `any_except` matches any single character
outside that set. An empty positive set matches nothing, while an empty
negated set matches every character:

```rust
use regex_builder::RegexBuilder;

let no_vowels = RegexBuilder::new()
    .any_except(|set| set.literal("aeiou"))
    .build()
    .unwrap();
let impossible = RegexBuilder::new()
    .any_char_of(|set| set)
    .build()
    .unwrap();
assert!(no_vowels.full_match("z").unwrap().is_some());
assert!(no_vowels.full_match("a").unwrap().is_none());
assert!(impossible.full_match("z").unwrap().is_none());
```

`any_char()` accepts one Unicode scalar value, including non-ASCII characters:

```rust
use regex_builder::RegexBuilder;

let one_character = RegexBuilder::new().any_char().build().unwrap();
assert!(one_character.full_match("🦀").unwrap().is_some());
assert!(one_character.full_match("ab").unwrap().is_none());
```

`border_of` and `non_border_of` are zero-width checks around characters from
the supplied set. For common word semantics, use `word_border()` and
`non_word_border()`.

```rust
use regex_builder::RegexBuilder;

let word = RegexBuilder::new()
    .word_border()
    .literal("rust")
    .word_border()
    .build()
    .unwrap();
assert!(word.full_match("rust").unwrap().is_some());
assert!(word.full_match("rusty").unwrap().is_none());
```

The non-border check matches positions where both neighboring characters
agree about membership in the set. This can be useful for matching a character
inside a word without matching at the word's edge:

```rust
use regex_builder::RegexBuilder;

let underscore_inside_word = RegexBuilder::new()
    .any_word()
    .non_word_border()
    .literal("_")
    .non_word_border()
    .any_word()
    .build()
    .unwrap();
assert!(underscore_inside_word.full_match("a_b").unwrap().is_some());
assert!(underscore_inside_word.full_match("a_").unwrap().is_none());
```

The ready-made classes (`any_digit`, `any_hex_digit`, `any_letter`,
`any_word`, and their negated variants) are ASCII-based. Matching and custom
ranges use Unicode scalar values (`char`), not UTF-8 bytes.

## Reusable regexes

Build a regex once and embed it by reference with `regex` or `named_regex`.
The embedded expression occupies one capture location and its own captures
remain in a nested scope.

```rust
use regex_builder::RegexBuilder;

let digits = RegexBuilder::new().any_digit().one_or_more().build().unwrap();
let pair = RegexBuilder::new()
    .regex(&digits)
    .literal("-")
    .named_regex("right", &digits)
    .build()
    .unwrap();
let captures = pair.full_match("12-34").unwrap().unwrap();
assert_eq!(captures.at_index(0).unwrap()[0].matched_text(), "12");
assert_eq!(captures.with_name("right").unwrap()[0].matched_text(), "34");
```

`Regex` is cloneable. Clones can be matched independently:

```rust
use regex_builder::RegexBuilder;

let original = RegexBuilder::new().literal("ready").build().unwrap();
let copy = original.clone();
assert!(original.full_match("ready").unwrap().is_some());
assert!(copy.full_match("ready").unwrap().is_some());
assert!(copy.full_match("not ready").unwrap().is_none());
```

Named and unnamed locations share their respective namespace throughout one
scope. A name may be reused in an inner scope, but not twice in the same one:

```rust
use regex_builder::{BuildError, RegexBuilder};

let duplicate = RegexBuilder::new()
    .capture("item", |b| b.literal("a"))
    .capture("item", |b| b.literal("b"))
    .build();
assert_eq!(
    duplicate.unwrap_err(),
    BuildError::DuplicateName("item".to_owned())
);
```

## Reading capture results

`at_index` and `with_name` return `None` for a location that does not exist,
an empty slice for a declared location that was not reached, or its ordered
occurrences. `matched_text` works for every occurrence; `try_group`,
`try_literal`, and `try_alternative` inspect its kind without panicking.
`Display` prints the capture tree, and alternate formatting (`{:#}`) keeps
long matched text unshortened.

```rust
use regex_builder::RegexBuilder;

let regex = RegexBuilder::new()
    .capture("item", |b| b.literal("book"))
    .build()
    .unwrap();
let captures = regex.full_match("book").unwrap().unwrap();
let occurrence = &captures.with_name("item").unwrap()[0];
assert_eq!(occurrence.try_literal(), Some("book"));
assert!(captures.with_name("missing").is_none());
println!("{captures}");
```

The display form is intended for inspection and debugging. It shows
locations, occurrence kinds, and nested captures:

```rust
use regex_builder::RegexBuilder;

let regex = RegexBuilder::new()
    .capture("digit", |b| b.any_digit())
    .build()
    .unwrap();
let captures = regex.full_match("7").unwrap().unwrap();
assert_eq!(format!("{captures}"), "\"digit\":\n  - literal \"7\"");
```

When a capture accessor returns an empty slice, that means the location was
declared but its path was not taken. This differs from a capture that matched
the empty string, which still has an occurrence:

```rust
use regex_builder::RegexBuilder;

let regex = RegexBuilder::new()
    .capture("present", |b| b.literal(""))
    .capture("optional", |b| b.literal("x"))
    .optional()
    .build()
    .unwrap();
let captures = regex.full_match("").unwrap().unwrap();
assert_eq!(captures.with_name("present").unwrap().len(), 1);
assert_eq!(captures.with_name("present").unwrap()[0].matched_text(), "");
assert!(captures.with_name("optional").unwrap().is_empty());
```

## Character behavior

Matching and ranges operate on Rust `char` values (Unicode scalar values), not
UTF-8 bytes. The predefined letter, word, and whitespace shortcuts are
intentionally ASCII-based: letters are `A-Z` and `a-z`; word characters also
include digits and underscore; whitespace includes space, tab, carriage return,
line feed, vertical tab, and form feed. Use `range`, `any_char_of`, or
`any_except` to define other sets. A positive empty set matches no character;
a negated empty set matches any character.

## Recursive patterns

Start by identifying the smallest valid value, then describe how larger values
are constructed from smaller ones. For balanced parentheses, the empty string
is the base case; wrapping an empty body produces `()`. The recursive case
wraps smaller balanced values in parentheses. Create a placeholder with
`recursive()`, refer to it with `recurse()`, and finish the definition with
`define()`:

```rust
use regex_builder::RegexBuilder;

let balanced = RegexBuilder::recursive();
let body = RegexBuilder::new().or(|branches| {
    branches
        .branch(|b| {
            b.literal("(")
                .recurse(&balanced)
                .zero_or_more()
                .literal(")")
        })
        .branch(|b| b.literal(""))
});
let balanced = balanced.define(body).unwrap();

assert!(balanced.full_match("").unwrap().is_some());
assert!(balanced.full_match("()").unwrap().is_some());
assert!(balanced.full_match("(())").unwrap().is_some());
assert!(balanced.full_match("(()())").unwrap().is_some());
assert!(balanced.full_match("(()").unwrap().is_none());
```

The empty branch is the base case: it stops the recursive descent. The
parenthesized branch wraps a smaller value. The `zero_or_more()` lets one
pair contain several balanced children, as in `(()())`. Without a base case,
the pattern would have no way to finish.

Here is a second example that uses a named recursive reference. Its recursive
capture is available by name inside the selected alternative's scope:

```rust
use regex_builder::RegexBuilder;

let item = RegexBuilder::recursive();
let body = RegexBuilder::new().or(|branches| {
    branches
        .branch(|b| b.literal("x"))
        .branch(|b| b.literal("[").named_recurse("inside", &item).literal("]"))
});
let item = item.define(body).unwrap();
let captures = item.full_match("[x]").unwrap().unwrap();
let chosen_branch = captures.at_index(0).unwrap()[0].expect_alternative().1;
assert_eq!(
    chosen_branch.with_name("inside").unwrap()[0].matched_text(),
    "x"
);
```

Building a regex that still depends on an undefined reference succeeds, but
`full_match` returns `BuildError::UndefinedRecursion` until all references are
defined. Left recursion is also supported:

```rust
use regex_builder::RegexBuilder;

let expression = RegexBuilder::recursive();
let body = RegexBuilder::new().or(|branches| {
    branches
        .branch(|b| b.recurse(&expression).literal("+").any_digit())
        .branch(|b| b.any_digit())
});
let expression = expression.define(body).unwrap();
assert!(expression.full_match("1+2+3").unwrap().is_some());
assert!(expression.full_match("1+").unwrap().is_none());
```

## Matching cost

Top-level repetitions are explored one match at a time, so later repetition
paths are not evaluated after a full match is found. Repetition is traversed
iteratively, but deeply nested pattern structure and recursive references
still use the call stack. The matcher explores possible paths by backtracking
and does not impose a time, memory, or recursion-depth limit. Ambiguous
patterns and deep recursion can consume substantial resources.
