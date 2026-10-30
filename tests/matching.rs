use regex_builder::RegexBuilder;

#[test]
fn long_greedy_and_lazy_repetitions_match_without_per_iteration_recursion() {
    let greedy = RegexBuilder::new()
        .any_digit()
        .one_or_more()
        .build()
        .unwrap();
    let lazy = RegexBuilder::new()
        .any_digit()
        .one_or_more()
        .lazy()
        .build()
        .unwrap();
    let input = "7".repeat(20_000);

    assert!(greedy.full_match(input.clone()).unwrap().is_some());
    assert!(lazy.full_match(input).unwrap().is_some());
}

#[test]
fn lazy_top_level_repeat_yields_before_trying_longer_matches() {
    let regex = RegexBuilder::new()
        .literal("a")
        .zero_or_more()
        .lazy()
        .build()
        .unwrap();

    assert!(regex.full_match("a".repeat(50_000)).unwrap().is_some());
}

#[test]
fn lazy_repeat_can_finish_without_exploring_an_extension() {
    let regex = RegexBuilder::new()
        .literal("a")
        .zero_or_more()
        .lazy()
        .build()
        .unwrap();

    assert!(regex.full_match("").unwrap().is_some());
}

#[test]
fn prefixed_repetition_is_matched_as_a_stream() {
    let regex = RegexBuilder::new()
        .literal("digits:")
        .any_digit()
        .one_or_more()
        .build()
        .unwrap();

    assert!(regex
        .full_match(format!("digits:{}", "8".repeat(20_000)))
        .unwrap()
        .is_some());
}

#[test]
fn repetition_keeps_greedy_and_lazy_capture_priority() {
    let greedy = RegexBuilder::new()
        .capture("prefix", |part| part.literal("a").zero_or_more())
        .literal("a")
        .zero_or_more()
        .build()
        .unwrap();
    let lazy = RegexBuilder::new()
        .capture("prefix", |part| part.literal("a").zero_or_more().lazy())
        .literal("a")
        .zero_or_more()
        .build()
        .unwrap();

    let greedy_captures = greedy.full_match("aa").unwrap().unwrap();
    let lazy_captures = lazy.full_match("aa").unwrap().unwrap();

    assert_eq!(
        greedy_captures.with_name("prefix").unwrap()[0].matched_text(),
        "aa"
    );
    assert_eq!(
        lazy_captures.with_name("prefix").unwrap()[0].matched_text(),
        ""
    );
}

#[test]
fn lazy_changes_which_adjacent_capture_consumes_the_input() {
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
    let greedy = greedy.full_match("aa").unwrap().unwrap();
    let lazy = lazy.full_match("aa").unwrap().unwrap();

    assert_eq!(greedy.with_name("first").unwrap()[0].matched_text(), "aa");
    assert_eq!(lazy.with_name("first").unwrap()[0].matched_text(), "");
    assert_eq!(lazy.with_name("second").unwrap()[0].matched_text(), "aa");
}

#[test]
fn alternatives_are_tried_in_order_and_report_the_branch() {
    let regex = RegexBuilder::new()
        .or_named("word", |branches| {
            branches
                .branch(|b| b.literal("cat"))
                .branch(|b| b.literal("car"))
        })
        .build()
        .unwrap();

    let captures = regex.full_match("car").unwrap().unwrap();
    let word = &captures.with_name("word").unwrap()[0];

    assert_eq!(word.matched_text(), "car");
    assert_eq!(word.taken_branch(), Some(1));
}

#[test]
fn captures_preserve_each_repeated_occurrence() {
    let regex = RegexBuilder::new()
        .capture("digit", |b| b.any_digit())
        .one_or_more()
        .build()
        .unwrap();

    let captures = regex.full_match("123").unwrap().unwrap();
    let digits = captures.with_name("digit").unwrap();

    assert_eq!(
        digits
            .iter()
            .map(|occurrence| occurrence.matched_text())
            .collect::<Vec<_>>(),
        ["1", "2", "3"]
    );
}
