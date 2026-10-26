//! Tests for recursive regexes: balanced nesting, left recursion, indirect
//! recursion through another regex, and the undefined-reference error.

use crate::{BuildError, RegexBuilder};

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
