use regex_builder::{BuildError, RegexBuilder};

#[test]
fn embedded_regexes_keep_capture_scopes_isolated_and_can_be_named() {
    let digits = RegexBuilder::new()
        .capture("digit", |b| b.any_digit())
        .build()
        .unwrap();
    let parent = RegexBuilder::new()
        .regex(&digits)
        .literal("-")
        .named_regex("right", &digits)
        .build()
        .unwrap();
    let captures = parent.full_match("1-2").unwrap().unwrap();

    assert_eq!(captures.at_index(0).unwrap()[0].matched_text(), "1");
    assert_eq!(
        captures.at_index(0).unwrap()[0]
            .expect_group()
            .with_name("digit")
            .unwrap()[0]
            .matched_text(),
        "1"
    );
    assert_eq!(captures.with_name("right").unwrap()[0].matched_text(), "2");
    assert!(digits.full_match("3").unwrap().is_some());
}

#[test]
fn a_regex_can_be_cloned_and_matched_independently() {
    let original = RegexBuilder::new().literal("ready").build().unwrap();
    let copy = original.clone();

    assert!(original.full_match("ready").unwrap().is_some());
    assert!(copy.full_match("ready").unwrap().is_some());
    assert!(copy.full_match("not ready").unwrap().is_none());
}

#[test]
fn names_share_a_namespace_within_a_scope_but_not_across_scopes() {
    let duplicate = RegexBuilder::new()
        .capture("item", |b| b.literal("a"))
        .named_regex("item", &RegexBuilder::new().literal("b").build().unwrap())
        .build();
    let separate_scopes = RegexBuilder::new()
        .capture("item", |outer| {
            outer.capture("item", |inner| inner.literal("a"))
        })
        .build();

    assert_eq!(
        duplicate.unwrap_err(),
        BuildError::DuplicateName("item".to_owned())
    );
    assert!(separate_scopes.is_ok());
}
