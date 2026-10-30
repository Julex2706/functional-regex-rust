use regex_builder::{BuildError, RegexBuilder};

#[test]
fn custom_sets_match_literals_and_ranges() {
    let set = RegexBuilder::new()
        .any_char_of(|set| set.literal("ae").range('0', '2'))
        .build()
        .unwrap();

    for accepted in ["a", "e", "0", "1", "2"] {
        assert!(set.full_match(accepted).unwrap().is_some(), "{accepted}");
    }
    for rejected in ["b", "3", ""] {
        assert!(set.full_match(rejected).unwrap().is_none(), "{rejected}");
    }
}

#[test]
fn negated_sets_and_any_character_handle_empty_sets() {
    let none = RegexBuilder::new().any_char_of(|set| set).build().unwrap();
    let everything = RegexBuilder::new().any_except(|set| set).build().unwrap();
    let except_vowels = RegexBuilder::new()
        .any_except(|set| set.literal("aeiou"))
        .build()
        .unwrap();
    let any = RegexBuilder::new().any_char().build().unwrap();

    assert!(none.full_match("a").unwrap().is_none());
    assert!(everything.full_match("🦀").unwrap().is_some());
    assert!(except_vowels.full_match("b").unwrap().is_some());
    assert!(except_vowels.full_match("a").unwrap().is_none());
    assert!(any.full_match("🦀").unwrap().is_some());
}

#[test]
fn unicode_ranges_operate_on_scalar_values() {
    let regex = RegexBuilder::new().range('α', 'ω').build().unwrap();

    assert!(regex.full_match("β").unwrap().is_some());
    assert!(regex.full_match("A").unwrap().is_none());
}

#[test]
fn backwards_ranges_return_an_error() {
    let regex = RegexBuilder::new().range('z', 'a').build();

    assert_eq!(
        regex.unwrap_err(),
        BuildError::InvalidRange { from: 'z', to: 'a' }
    );
}

#[test]
fn shortcut_classes_and_boundaries_have_documented_ascii_semantics() {
    let word = RegexBuilder::new().any_word().build().unwrap();
    let non_word = RegexBuilder::new().any_non_word().build().unwrap();
    let digit = RegexBuilder::new().any_digit().build().unwrap();
    let non_digit = RegexBuilder::new().any_non_digit().build().unwrap();
    let whitespace = RegexBuilder::new().any_whitespace().build().unwrap();
    let non_whitespace = RegexBuilder::new().any_non_whitespace().build().unwrap();
    let hex = RegexBuilder::new().any_hex_digit().build().unwrap();
    let non_hex = RegexBuilder::new().any_non_hex_digit().build().unwrap();
    let letter = RegexBuilder::new().any_letter().build().unwrap();
    let lowercase = RegexBuilder::new().any_lowercase().build().unwrap();
    let uppercase = RegexBuilder::new().any_uppercase().build().unwrap();
    let border = RegexBuilder::new()
        .word_border()
        .literal("rust")
        .word_border()
        .build()
        .unwrap();
    let non_border = RegexBuilder::new()
        .any_word()
        .non_word_border()
        .literal("_")
        .non_word_border()
        .any_word()
        .build()
        .unwrap();

    assert!(word.full_match("_").unwrap().is_some());
    assert!(word.full_match("é").unwrap().is_none());
    assert!(non_word.full_match("é").unwrap().is_some());
    assert!(digit.full_match("7").unwrap().is_some());
    assert!(non_digit.full_match("x").unwrap().is_some());
    assert!(whitespace.full_match("\t").unwrap().is_some());
    assert!(non_whitespace.full_match("x").unwrap().is_some());
    assert!(hex.full_match("F").unwrap().is_some());
    assert!(non_hex.full_match("g").unwrap().is_some());
    assert!(letter.full_match("z").unwrap().is_some());
    assert!(lowercase.full_match("z").unwrap().is_some());
    assert!(uppercase.full_match("Z").unwrap().is_some());
    assert!(border.full_match("rust").unwrap().is_some());
    assert!(border.full_match("rusty").unwrap().is_none());
    assert!(non_border.full_match("a_b").unwrap().is_some());
    assert!(non_border.full_match("a_").unwrap().is_none());
}

#[test]
fn custom_border_and_non_border_are_zero_width_checks() {
    let vowel_to_consonant = RegexBuilder::new()
        .literal("a")
        .border_of(|set| set.literal("aeiou"))
        .any_except(|set| set.literal("aeiou"))
        .build()
        .unwrap();
    assert!(vowel_to_consonant.full_match("ab").unwrap().is_some());
    assert!(vowel_to_consonant.full_match("aa").unwrap().is_none());

    let border = RegexBuilder::new()
        .literal("x")
        .border_of(|set| set.literal("x"))
        .literal("a")
        .build()
        .unwrap();
    let non_border = RegexBuilder::new()
        .literal("x")
        .non_border_of(|set| set.literal("x"))
        .literal("x")
        .build()
        .unwrap();

    assert!(border.full_match("xa").unwrap().is_some());
    assert!(border.full_match("xx").unwrap().is_none());
    assert!(non_border.full_match("xx").unwrap().is_some());
    assert!(non_border.full_match("xa").unwrap().is_none());
}

#[test]
fn any_char_matches_one_unicode_scalar_not_one_byte() {
    let regex = RegexBuilder::new().any_char().build().unwrap();

    assert!(regex.full_match("🦀").unwrap().is_some());
    assert!(regex.full_match("ab").unwrap().is_none());
}
