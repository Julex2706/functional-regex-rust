use regex_builder::{BuildError, RegexBuilder};

#[test]
fn recursive_pattern_matches_balanced_parentheses() {
    let expression = RegexBuilder::recursive();
    let body = RegexBuilder::new()
        .literal("(")
        .recurse(&expression)
        .zero_or_more()
        .literal(")");
    let balanced = expression.define(body).unwrap();

    let captures = balanced.full_match("(()())").unwrap().unwrap();
    let inner = captures.at_index(0).unwrap();

    assert_eq!(inner.len(), 2);
    assert_eq!(inner[0].matched_text(), "()");
    assert_eq!(inner[1].matched_text(), "()");
    assert!(inner[0].expect_group().at_index(0).unwrap().is_empty());
    assert!(balanced.full_match("(()").unwrap().is_none());
    assert!(balanced.full_match("").unwrap().is_none());
}

#[test]
fn empty_base_case_allows_balanced_parentheses_to_stop_recursing() {
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

    for input in ["", "()", "(())", "(()())"] {
        assert!(balanced.full_match(input).unwrap().is_some(), "{input}");
    }
    assert!(balanced.full_match("(()").unwrap().is_none());
}

#[test]
fn left_recursive_expression_matches_left_nested_input() {
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
fn indirect_recursion_works_after_references_are_defined() {
    let even = RegexBuilder::recursive();
    let odd = RegexBuilder::recursive();

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
fn undefined_reference_is_reported_when_matching() {
    let reference = RegexBuilder::recursive();
    let regex = RegexBuilder::new()
        .literal("a")
        .recurse(&reference)
        .build()
        .unwrap();

    assert_eq!(regex.full_match("a"), Err(BuildError::UndefinedRecursion));
}

#[test]
fn named_recursive_references_are_available_in_captures() {
    let reference = RegexBuilder::recursive();
    let body = RegexBuilder::new().or(|branches| {
        branches
            .branch(|b| b.literal("x"))
            .branch(|b| b.literal("x").named_recurse("inner", &reference))
    });
    let regex = reference.define(body).unwrap();
    let captures = regex.full_match("xx").unwrap().unwrap();
    let recursive_scope = captures.at_index(0).unwrap()[0].expect_alternative().1;

    assert_eq!(
        recursive_scope.with_name("inner").unwrap()[0].matched_text(),
        "x"
    );
}
