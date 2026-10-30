use regex_builder::{BuildError, RegexBuilder};

#[test]
fn literals_match_the_entire_input() {
    let regex = RegexBuilder::new().literal("rust").build().unwrap();

    assert!(regex.full_match("rust").unwrap().is_some());
    assert!(regex.full_match("rust!").unwrap().is_none());
    assert!(regex.full_match("Rust").unwrap().is_none());
}

#[test]
fn literal_quantifier_applies_only_to_the_previous_atom() {
    let tagged_digits = RegexBuilder::new()
        .literal("id=")
        .any_digit()
        .one_or_more()
        .build()
        .unwrap();

    assert!(tagged_digits.full_match("id=12").unwrap().is_some());
    assert!(tagged_digits.full_match("id=id=12").unwrap().is_none());
}

#[test]
fn an_empty_pattern_only_matches_an_empty_input() {
    let regex = RegexBuilder::new().build().unwrap();

    assert!(regex.full_match("").unwrap().is_some());
    assert!(regex.full_match("anything").unwrap().is_none());
}

#[test]
fn quantifier_helpers_cover_bounded_and_unbounded_counts() {
    let exactly = RegexBuilder::new().any_digit().exactly(3).build().unwrap();
    let between = RegexBuilder::new()
        .any_digit()
        .between(2, 4)
        .build()
        .unwrap();
    let optional = RegexBuilder::new().literal("-").optional().build().unwrap();
    let at_most = RegexBuilder::new().literal("a").at_most(2).build().unwrap();
    let at_least = RegexBuilder::new()
        .literal("a")
        .at_least(2)
        .build()
        .unwrap();

    assert!(exactly.full_match("123").unwrap().is_some());
    assert!(exactly.full_match("12").unwrap().is_none());
    assert!(between.full_match("12").unwrap().is_some());
    assert!(between.full_match("12345").unwrap().is_none());
    assert!(optional.full_match("").unwrap().is_some());
    assert!(optional.full_match("-").unwrap().is_some());
    assert!(at_most.full_match("aa").unwrap().is_some());
    assert!(at_most.full_match("aaa").unwrap().is_none());
    assert!(at_least.full_match("aa").unwrap().is_some());
    assert!(at_least.full_match("a").unwrap().is_none());
}

#[test]
fn optional_sign_and_lazy_separator_examples_work() {
    let signed_integer = RegexBuilder::new()
        .literal("-")
        .optional()
        .any_digit()
        .one_or_more()
        .build()
        .unwrap();
    let split = RegexBuilder::new()
        .capture("first", |b| b.any_char().zero_or_more().lazy())
        .literal(":")
        .capture("second", |b| b.any_char().zero_or_more())
        .build()
        .unwrap();

    assert!(signed_integer.full_match("42").unwrap().is_some());
    assert!(signed_integer.full_match("-42").unwrap().is_some());
    assert!(signed_integer.full_match("--42").unwrap().is_none());
    let captures = split.full_match("left:right").unwrap().unwrap();
    assert_eq!(
        captures.with_name("first").unwrap()[0].matched_text(),
        "left"
    );
    assert_eq!(
        captures.with_name("second").unwrap()[0].matched_text(),
        "right"
    );
}

#[test]
fn invalid_quantifier_and_empty_alternative_are_reported() {
    let invalid = RegexBuilder::new().literal("a").between(3, 2).build();
    let empty = RegexBuilder::new().or(|branches| branches).build();

    assert_eq!(
        invalid.unwrap_err(),
        BuildError::InvalidQuantifier { min: 3, max: 2 }
    );
    assert_eq!(empty.unwrap_err(), BuildError::EmptyAlternative);
    assert_eq!(
        BuildError::InvalidQuantifier { min: 3, max: 2 }.to_string(),
        "quantifier minimum 3 is greater than its maximum 2"
    );
}

#[test]
fn alternatives_try_branches_in_order_and_any_of_is_a_shortcut() {
    let alternatives = RegexBuilder::new()
        .any_of(vec!["cat", "car"])
        .build()
        .unwrap();
    let captures = alternatives.full_match("car").unwrap().unwrap();

    assert_eq!(captures.at_index(0).unwrap()[0].taken_branch(), Some(1));
    assert!(alternatives.full_match("cab").unwrap().is_none());

    let empty_options = RegexBuilder::new().any_of(Vec::<String>::new()).build();
    assert_eq!(empty_options.unwrap_err(), BuildError::EmptyAlternative);
}

#[test]
fn named_options_record_branch_index_and_branch_local_captures() {
    let regex = RegexBuilder::new()
        .any_of_named("result", vec!["ok", "error"])
        .build()
        .unwrap();
    let captures = regex.full_match("error").unwrap().unwrap();
    let result = &captures.with_name("result").unwrap()[0];

    assert_eq!(result.taken_branch(), Some(1));
    assert_eq!(result.matched_text(), "error");
    assert_eq!(
        format!("{}", result.expect_alternative().1),
        "(no captures)"
    );
}

#[test]
fn captures_inside_alternative_branches_stay_in_the_taken_branch() {
    let regex = RegexBuilder::new()
        .or_named("value", |branches| {
            branches
                .branch(|b| b.capture("word", |word| word.any_letter().one_or_more()))
                .branch(|b| b.capture("number", |number| number.any_digit().one_or_more()))
        })
        .build()
        .unwrap();
    let captures = regex.full_match("42").unwrap().unwrap();
    let branch = captures.with_name("value").unwrap()[0]
        .expect_alternative()
        .1;

    assert!(branch.with_name("word").is_none());
    assert_eq!(branch.with_name("number").unwrap()[0].matched_text(), "42");
}

#[test]
fn groups_have_nested_scopes_and_can_be_repeated() {
    let regex = RegexBuilder::new()
        .group(|group| {
            group
                .capture("letter", |letter| letter.any_letter())
                .capture("digit", |digit| digit.any_digit())
        })
        .one_or_more()
        .build()
        .unwrap();
    let captures = regex.full_match("a1b2").unwrap().unwrap();
    let groups = captures.at_index(0).unwrap();

    assert_eq!(groups.len(), 2);
    assert_eq!(groups[0].matched_text(), "a1");
    assert_eq!(
        groups[0].expect_group().with_name("letter").unwrap()[0].matched_text(),
        "a"
    );
    assert_eq!(groups[1].matched_text(), "b2");
}
