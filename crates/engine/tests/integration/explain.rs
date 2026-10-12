use kanaemi_core::Converter;
use kanaemi_engine::{
    DropReason, Dropped, Engine, Explanation, Lookup, LookupKey, OkuriHead, Origin, OriginItem,
    RuleGroup, Score, Selections, Slot, TextDictionary, show_placeholders,
};
use proptest::prelude::*;

use crate::common::{Discard, Learn, Notations, dictionary, model};

fn text(text: &str) -> Slot {
    Slot::Dictionary(dictionary(text))
}

fn engine(slots: Vec<Slot>, user: &str) -> Engine {
    let user = TextDictionary::parse_user_custom(user).0;
    Engine::new(slots, user, Discard)
}

fn surfaces(explanation: &Explanation) -> Vec<&str> {
    explanation
        .candidates
        .iter()
        .map(|c| c.facts.surface.as_str())
        .collect()
}

fn explained<'a>(explanation: &'a Explanation, surface: &str) -> &'a kanaemi_engine::Explained {
    explanation
        .candidates
        .iter()
        .find(|c| c.facts.surface == surface)
        .unwrap_or_else(|| panic!("{surface} is not a candidate: {explanation:?}"))
}

fn origin(dictionary: usize, cost: u32, item: OriginItem) -> Origin {
    Origin {
        dictionary,
        cost,
        item,
    }
}

fn word(reading: &str, surface: &str) -> OriginItem {
    OriginItem::Word {
        reading: reading.into(),
        surface: surface.into(),
    }
}

fn okuri(stem: &str, head: OkuriHead, surface: &str) -> OriginItem {
    OriginItem::Okuri {
        stem: stem.into(),
        head,
        surface: surface.into(),
    }
}

fn conjugated(stem: &str, surface: &str, conjugation: &str, rest: &str) -> OriginItem {
    OriginItem::Conjugated {
        stem: stem.into(),
        surface: surface.into(),
        conjugation: conjugation.into(),
        rest: rest.into(),
    }
}

fn reasons(explanation: &Explanation) -> Vec<(Option<&str>, &OriginItem, &DropReason)> {
    explanation
        .dropped
        .iter()
        .map(|d| (d.surface.as_deref(), &d.origin.item, &d.reason))
        .collect()
}

#[test]
fn the_reading_and_okurigana_are_given_as_converted() {
    let e = engine(vec![text("か\t書\t五段-カ行")], "");
    let explanation = e.explain("か\u{3099}く", Some("く"));
    assert_eq!(explanation.reading, "がく");
    assert_eq!(explanation.okurigana.as_deref(), Some("く"));
}

#[test]
fn a_word_of_the_whole_reading_comes_from_its_item() {
    let e = engine(vec![Slot::UserCustom, text("かく\t角")], "");
    let explanation = e.explain("かく", None);
    assert_eq!(
        explained(&explanation, "角").origins,
        [origin(1, 0, word("かく", "角"))]
    );
}

#[test]
fn an_okurigana_word_comes_from_its_stem_its_head_and_its_surface_before_the_okurigana() {
    let e = engine(vec![Slot::UserCustom, text("く*ら\t暮ら\nく*r\t繰")], "");
    let explanation = e.explain("くら", Some("ら"));
    assert_eq!(
        explained(&explanation, "暮ら").origins,
        [origin(1, 0, okuri("く", OkuriHead::Kana('ら'), "暮"))]
    );
    assert_eq!(
        explained(&explanation, "繰ら").origins,
        [origin(1, 0, okuri("く", OkuriHead::Row('r'), "繰"))]
    );
}

#[test]
fn a_built_form_comes_from_its_stem_its_type_and_what_follows_it() {
    let e = engine(vec![Slot::UserCustom, text("か\t書\t五段-カ行")], "");
    let explanation = e.explain("かく", None);
    let built = explained(&explanation, "書く");
    assert!(built.facts.built);
    assert_eq!(
        built.origins.iter().map(|o| &o.item).collect::<Vec<_>>(),
        [&conjugated("か", "書", "五段-カ行", "く")]
    );
    assert_eq!(
        built.origins[0].cost, built.facts.cost,
        "with what building adds"
    );
}

#[test]
fn a_filled_candidate_comes_from_its_item_with_placeholders() {
    let mut e = engine(vec![Slot::UserCustom, text("{}こ\t{wide-num}個")], "");
    e.set_functions(Notations::shared());
    let explanation = e.explain("1こ", None);
    let [origin] = &explained(&explanation, "１個").origins[..] else {
        panic!("{explanation:?}");
    };
    let OriginItem::Placeholders { reading, surface } = &origin.item else {
        panic!("{origin:?}");
    };
    assert_eq!(
        (show_placeholders(reading), show_placeholders(surface)),
        ("{}こ".into(), "{wide-num}個".into())
    );
}

#[test]
fn every_way_a_dictionary_gives_a_surface_is_listed_the_one_taken_first() {
    let e = engine(
        vec![
            Slot::UserCustom,
            text("くら\t暮ら\t\t5\nく*ら\t暮ら\t\t3\nく\t暮\t五段-ラ行\t1"),
        ],
        "",
    );
    let explanation = e.explain("くら", Some("ら"));
    let candidate = explained(&explanation, "暮ら");
    assert_eq!(
        candidate.origins,
        [
            origin(1, 3, okuri("く", OkuriHead::Kana('ら'), "暮")),
            origin(1, 5, word("くら", "暮ら")),
            origin(
                1,
                candidate.origins[2].cost,
                conjugated("く", "暮", "五段-ラ行", "ら")
            ),
        ]
    );
    assert_eq!(candidate.facts.cost, 3);
}

#[test]
fn a_pair_the_user_hid_is_dropped_as_hidden() {
    let e = engine(
        vec![Slot::UserCustom, text("かく\t角\nかく\t画")],
        "!かく\t角",
    );
    let explanation = e.explain("かく", None);
    assert_eq!(surfaces(&explanation), ["画"]);
    assert_eq!(
        reasons(&explanation),
        [(Some("角"), &word("かく", "角"), &DropReason::Hidden)]
    );
}

#[test]
fn a_stem_whose_type_does_not_take_what_follows_it_is_dropped() {
    let e = engine(vec![Slot::UserCustom, text("か\t書\t五段-カ行")], "");
    let explanation = e.explain("かた", Some("た"));
    assert_eq!(surfaces(&explanation), Vec::<&str>::new());
    assert_eq!(
        reasons(&explanation),
        [(
            Some("書た"),
            &conjugated("か", "書", "五段-カ行", "た"),
            &DropReason::NotConjugated
        )]
    );
}

#[test]
fn a_candidate_that_does_not_end_with_the_okurigana_is_dropped() {
    let e = engine(vec![Slot::UserCustom, text("かく\t角")], "");
    let explanation = e.explain("かく", Some("く"));
    assert_eq!(
        reasons(&explanation),
        [(
            Some("角"),
            &word("かく", "角"),
            &DropReason::OkuriganaMismatch
        )]
    );
}

#[test]
fn a_surface_an_earlier_dictionary_gave_is_dropped_from_the_later_one() {
    let e = engine(
        vec![
            Slot::UserCustom,
            text("かく\t角"),
            text("かく\t角\nかく\t画"),
        ],
        "",
    );
    let explanation = e.explain("かく", None);
    assert_eq!(surfaces(&explanation), ["角", "画"]);
    assert_eq!(
        explanation.dropped,
        [Dropped {
            surface: Some("角".into()),
            origin: origin(2, 1, word("かく", "角")),
            reason: DropReason::Shadowed { by: 1 },
        }]
    );
}

#[test]
fn an_item_whose_placeholders_are_not_filled_is_dropped() {
    let mut e = engine(vec![Slot::UserCustom, text("{}こ\t{kanji}個")], "");
    e.set_functions(Notations::shared());
    let explanation = e.explain("100こ", None);
    assert_eq!(surfaces(&explanation), Vec::<&str>::new());
    let [dropped] = &explanation.dropped[..] else {
        panic!("{explanation:?}");
    };
    assert_eq!(dropped.surface, None);
    assert_eq!(dropped.reason, DropReason::NotFilled);
    assert!(matches!(
        dropped.origin.item,
        OriginItem::Placeholders { .. }
    ));
}

#[test]
fn every_key_looked_up_is_listed_with_how_many_items_it_found() {
    let e = engine(
        vec![Slot::UserCustom, text("く\t繰\t五段-ラ行\nく*ら\t暮ら")],
        "",
    );
    let explanation = e.explain("くら", Some("ら"));
    let looked_up: Vec<&Lookup> = explanation
        .lookups
        .iter()
        .filter(|l| l.dictionary == 1)
        .collect();
    let lookup = |key, found| Lookup {
        dictionary: 1,
        key,
        found,
    };
    assert_eq!(
        looked_up,
        [
            &lookup(LookupKey::Reading("く".into()), 1),
            &lookup(LookupKey::Reading("くら".into()), 0),
            &lookup(
                LookupKey::Okuri {
                    stem: "く".into(),
                    head: OkuriHead::Kana('ら'),
                },
                1
            ),
            &lookup(
                LookupKey::Okuri {
                    stem: "く".into(),
                    head: OkuriHead::Row('r'),
                },
                0
            ),
        ]
    );
    assert!(
        explanation.lookups.iter().any(|l| l.dictionary == 0),
        "the user custom dictionary is looked up too"
    );
}

#[test]
fn a_model_score_is_the_sum_of_the_weights_of_its_features() {
    let mut e = engine(vec![Slot::UserCustom, text("かく\t角\nかく\t画")], "");
    e.set_model(Some(model(16, &[("s\u{1f}画", 2.0)])));
    let explanation = e.explain("かく", None);
    assert!(explanation.ranked_by_model);
    assert_eq!(surfaces(&explanation), ["画", "角"]);
    let Score::Model { total, features } = &explained(&explanation, "画").score else {
        panic!("{explanation:?}");
    };
    assert_eq!(*total, 2.0);
    assert!(
        features.contains(&("s\u{1f}画".into(), 2.0)),
        "{features:?}"
    );
    assert_eq!(features.iter().map(|(_, w)| w).sum::<f32>(), *total);
}

#[test]
fn without_a_model_the_rule_that_placed_a_candidate_is_given() {
    let mut e = engine(
        vec![Slot::UserCustom, text("かく\t角\nかく\t画\nかく\t格")],
        "",
    );
    e.commit("かく", "画");
    e.commit("かく", "格");
    let explanation = e.explain("かく", None);
    assert!(!explanation.ranked_by_model);
    let rule = |surface| explained(&explanation, surface).score.clone();
    assert_eq!(rule("格"), Score::Rules(RuleGroup::LastCommit));
    assert_eq!(rule("画"), Score::Rules(RuleGroup::Committed));
    assert_eq!(rule("角"), Score::Rules(RuleGroup::Other));
}

#[test]
fn a_candidate_split_at_the_mark_shows_how_it_was_brought_forward() {
    let e = engine(
        vec![
            Slot::UserCustom,
            text("たちゃ\t立ちゃ\t\t1\nたち*ゃ\t達ゃ\t\t3"),
        ],
        "",
    );
    let explanation = e.explain("たちゃ", Some("ゃ"));
    assert!(explanation.okuri_first_applied);
    assert_eq!(surfaces(&explanation), ["達ゃ", "立ちゃ"]);
    let split = explained(&explanation, "達ゃ");
    assert!(split.okuri);
    assert_eq!((split.ranked, split.after_okuri_first), (1, 0));
}

#[test]
fn a_favorite_shows_its_weight_and_how_it_was_brought_forward() {
    let mut e = engine(
        vec![
            Slot::UserCustom,
            text("きしゃ\t記者\t\t10\nきしゃ\t汽車\t\t20\nきしゃ\t貴社\t\t30"),
        ],
        "",
    );
    e.replace_selections(Selections::parse("きしゃ\t貴社\t3\n"));
    let explanation = e.explain("きしゃ", None);
    assert!(explanation.favor_applied);
    assert_eq!(surfaces(&explanation), ["貴社", "記者", "汽車"]);
    let favorite = explained(&explanation, "貴社");
    assert!(favorite.favorite.is_some_and(|w| w >= 2.5), "{favorite:?}");
    assert_eq!((favorite.ranked, favorite.after_okuri_first), (2, 2));
    assert_eq!(explained(&explanation, "記者").favorite, None);
}

#[test]
fn a_commit_of_the_reading_in_the_field_leaves_the_order_to_its_history() {
    let mut e = engine(vec![Slot::UserCustom, text("かく\t角\nか*く\t書く")], "");
    e.replace_selections(Selections::parse("かく\t角\t3\n"));
    e.commit("かく", "書く");
    let explanation = e.explain("かく", Some("く"));
    assert!(!explanation.okuri_first_applied);
    assert!(!explanation.favor_applied);
}

#[test]
fn the_context_and_the_history_ranked_against_are_given() {
    let mut e = engine(vec![Slot::UserCustom, text("かく\t角")], "");
    e.type_text("きょうは");
    e.commit("かく", "角");
    let explanation = e.explain("かく", None);
    assert_eq!(explanation.context, "きょうは");
    assert_eq!(explanation.history_len, 1);
}

#[test]
fn explaining_leaves_the_conversion_a_commit_is_recorded_by() {
    let mut e = engine(vec![Slot::UserCustom, text("{}こ\t{wide-num}個")], "");
    e.set_functions(Notations::shared());
    e.convert("1こ", None);
    e.explain("2こ", None);
    e.commit("1こ", "１個");
    let explanation = e.explain("3こ", None);
    assert_eq!(
        explained(&explanation, "３個").score,
        Score::Rules(RuleGroup::LastCommit),
        "the commit was recorded by its item, as converted"
    );
}

#[test]
fn the_candidates_are_those_converting_gives_with_a_model_and_a_history() {
    let mut e = engine(
        vec![
            Slot::UserCustom,
            text("かく\t角\nかく\t画\nか\t書\t五段-カ行\nか*く\t欠く"),
        ],
        "",
    );
    e.set_model(Some(model(16, &[("s\u{1f}画", 1.0), ("x\u{1f}1", 0.5)])));
    e.type_text("えを");
    e.commit("かく", "角");
    for okurigana in [None, Some("く")] {
        let converted: Vec<String> = e
            .convert("かく", okurigana)
            .into_iter()
            .map(|c| c.surface)
            .collect();
        assert_eq!(surfaces(&e.explain("かく", okurigana)), converted);
    }
}

const STEMS: [&str; 4] = ["か", "く", "くら", "かっ"];
const SURFACES: [&str; 5] = ["書", "来", "暮", "勝", "角"];
const TYPES: [&str; 4] = ["五段-カ行", "五段-ラ行", "五段-サ行", "五段-タ行"];
const HEADS: [&str; 6] = ["く", "ら", "っ", "k", "r", "t"];
const READINGS: [&str; 6] = ["かく", "くら", "かった", "かっ", "くる", "か"];

/// One line of a small text dictionary over a few readings, so lines meet.
fn line() -> impl Strategy<Value = String> {
    let cost = proptest::option::of(0u32..50)
        .prop_map(|c| c.map(|c| format!("\t\t{c}")).unwrap_or_default());
    prop_oneof![
        (0..READINGS.len(), 0..SURFACES.len(), cost.clone())
            .prop_map(|(r, s, c)| format!("{}\t{}{c}", READINGS[r], SURFACES[s])),
        (0..STEMS.len(), 0..SURFACES.len(), 0..TYPES.len())
            .prop_map(|(r, s, t)| format!("{}\t{}\t{}", STEMS[r], SURFACES[s], TYPES[t])),
        (0..2usize, 0..HEADS.len(), 0..SURFACES.len(), cost).prop_map(|(r, h, s, c)| {
            let head = HEADS[h];
            let surface = if head.is_ascii() {
                SURFACES[s].to_owned()
            } else {
                format!("{}{head}", SURFACES[s])
            };
            format!("{}*{head}\t{surface}{c}", STEMS[r])
        }),
    ]
}

proptest! {
    #[test]
    fn the_candidates_are_those_converting_gives(
        first in proptest::collection::vec(line(), 0..8),
        second in proptest::collection::vec(line(), 0..8),
        hidden in proptest::option::of((0..READINGS.len(), 0..SURFACES.len())),
        commit in proptest::option::of((0..READINGS.len(), 0..SURFACES.len())),
        reading in 0..READINGS.len(),
        okurigana in 0..3usize,
    ) {
        let user = hidden
            .map(|(r, s)| format!("!{}\t{}", READINGS[r], SURFACES[s]))
            .unwrap_or_default();
        let mut e = engine(
            vec![Slot::UserCustom, text(&first.join("\n")), text(&second.join("\n"))],
            &user,
        );
        if let Some((r, s)) = commit {
            e.commit(READINGS[r], SURFACES[s]);
        }
        let reading = READINGS[reading];
        let chars: Vec<char> = reading.chars().collect();
        let okurigana: Option<String> = (okurigana > 0 && okurigana < chars.len())
            .then(|| chars[chars.len() - okurigana..].iter().collect());
        let converted: Vec<(String, Option<String>)> = e
            .convert(reading, okurigana.as_deref())
            .into_iter()
            .map(|c| (c.surface, c.source))
            .collect();
        let explained: Vec<(String, Option<String>)> = e
            .explain(reading, okurigana.as_deref())
            .candidates
            .into_iter()
            .map(|c| (c.facts.surface, c.source))
            .collect();
        prop_assert_eq!(explained, converted);
    }
}
