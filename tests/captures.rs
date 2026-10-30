use regex_builder::{CaptureOccurrence, RegexBuilder};

#[test]
fn named_and_indexed_accessors_distinguish_unknown_untaken_and_matched() {
    let regex = RegexBuilder::new()
        .or_named("choice", |branches| {
            branches
                .branch(|b| b.literal("on"))
                .branch(|b| b.literal("off"))
        })
        .capture("optional", |b| b.literal("x"))
        .optional()
        .build()
        .unwrap();
    let captures = regex.full_match("off").unwrap().unwrap();

    assert!(captures.at_index(20).is_none());
    assert!(captures.with_name("missing").is_none());
    assert_eq!(captures.with_name("optional").unwrap().len(), 0);
    assert_eq!(
        captures.with_name("choice").unwrap()[0].taken_branch(),
        Some(1)
    );
}

#[test]
fn empty_matches_are_distinct_from_untaken_captures() {
    let regex = RegexBuilder::new()
        .capture("empty", |b| b.literal(""))
        .capture("optional", |b| b.literal("x"))
        .optional()
        .build()
        .unwrap();
    let captures = regex.full_match("").unwrap().unwrap();

    assert_eq!(captures.with_name("empty").unwrap().len(), 1);
    assert_eq!(captures.with_name("empty").unwrap()[0].matched_text(), "");
    assert!(captures.with_name("optional").unwrap().is_empty());
}

#[test]
fn occurrence_accessors_report_their_variant_without_panicking() {
    let regex = RegexBuilder::new()
        .group(|b| b.capture("inner", |inner| inner.literal("x")))
        .capture("plain", |b| b.literal("y"))
        .or(|branches| branches.branch(|b| b.literal("z")))
        .build()
        .unwrap();
    let captures = regex.full_match("xyz").unwrap().unwrap();
    let group = &captures.at_index(0).unwrap()[0];
    let plain = &captures.with_name("plain").unwrap()[0];
    let alternative = &captures.at_index(1).unwrap()[0];

    assert!(group.try_group().is_some());
    assert_eq!(
        group.expect_group().with_name("inner").unwrap()[0].matched_text(),
        "x"
    );
    assert_eq!(plain.try_literal(), Some("y"));
    assert_eq!(plain.expect_literal(), "y");
    assert!(plain.try_group().is_none());
    assert_eq!(alternative.try_alternative().unwrap().0, 0);
    assert_eq!(alternative.expect_alternative().0, 0);
    assert_eq!(alternative.taken_branch(), Some(0));
    assert_eq!(alternative.matched_text(), "z");
}

#[test]
fn capture_display_is_a_readable_tree_and_alternate_format_keeps_full_text() {
    let regex = RegexBuilder::new()
        .capture("long", |b| b.literal("x".repeat(50)))
        .build()
        .unwrap();
    let captures = regex.full_match("x".repeat(50)).unwrap().unwrap();
    let shortened = format!("{captures}");
    let full = format!("{captures:#}");

    assert!(shortened.contains("..."));
    assert!(full.contains(&"x".repeat(50)));
    assert!(format!("{:#}", &captures.with_name("long").unwrap()[0]).contains(&"x".repeat(50)));

    let short_regex = RegexBuilder::new()
        .capture("digit", |b| b.any_digit())
        .build()
        .unwrap();
    let short_captures = short_regex.full_match("7").unwrap().unwrap();
    assert_eq!(format!("{short_captures}"), "\"digit\":\n  - literal \"7\"");
}

#[test]
fn capture_occurrence_variants_are_publicly_matchable() {
    let regex = RegexBuilder::new()
        .capture("text", |b| b.literal("word"))
        .build()
        .unwrap();
    let captures = regex.full_match("word").unwrap().unwrap();

    assert!(matches!(
        &captures.with_name("text").unwrap()[0],
        CaptureOccurrence::Literal(_)
    ));
}
