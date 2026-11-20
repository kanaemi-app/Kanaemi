//! Numbers in readings, which numeric items put in their placeholders.

use crate::placeholder::{CLOSE, OPEN};

/// The numbers of a reading, and the reading a numeric item has for it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Numbers {
    /// The reading with a placeholder in place of each number.
    pub(crate) reading: String,
    /// The digits of each number as typed, first to last.
    pub(crate) values: Vec<String>,
}

impl Numbers {
    /// `None` for a reading without digits.
    pub(crate) fn find(reading: &str) -> Option<Self> {
        let mut found = Self {
            reading: String::new(),
            values: Vec::new(),
        };
        let mut in_number = false;
        for c in reading.chars() {
            match (is_digit(c), in_number) {
                (true, true) => found.values.last_mut()?.push(c),
                (true, false) => {
                    found.reading.extend([OPEN, CLOSE]);
                    found.values.push(c.to_string());
                }
                (false, _) => found.reading.push(c),
            }
            in_number = is_digit(c);
        }
        (!found.values.is_empty()).then_some(found)
    }
}

fn is_digit(c: char) -> bool {
    matches!(c, '0'..='9' | '０'..='９')
}

/// The name of a reading's placeholder that takes a number typed in kana.
pub(crate) const KANA: &str = "kana";

/// Past each way with one kana number, at most this many readings in all are
/// looked up for a reading's kana numbers: a reading full of kana that read
/// as numbers (し, く, に) would otherwise be looked up in every dictionary
/// for each way to split it. A long reading may not reach the ways with
/// three or more numbers.
const MAX_KANA_READINGS: usize = 32;

impl Numbers {
    /// Each way to put one kana number of the reading, then each way to put
    /// more within [`MAX_KANA_READINGS`], with all its digits, as
    /// placeholders. Fewer kana numbers come first, and earlier ones first.
    pub(crate) fn kana(reading: &str) -> Vec<Self> {
        let digits = digit_runs(reading);
        let in_digits = |at: usize| digits.iter().any(|&(start, end)| start <= at && at < end);
        let numerals: Vec<(usize, usize, String)> = reading
            .char_indices()
            .filter(|&(at, _)| !in_digits(at))
            .flat_map(|(at, _)| {
                kana_numbers(&reading[at..])
                    .into_iter()
                    .map(move |(len, value)| (at, at + len, value))
            })
            .filter(|&(start, end, _)| !digits.iter().any(|&(s, e)| s < end && start < e))
            .collect();
        let more = MAX_KANA_READINGS.saturating_sub(numerals.len());
        apart(&numerals, 1, 0)
            .chain(
                (2..=numerals.len())
                    .flat_map(|count| apart(&numerals, count, 0))
                    .take(more),
            )
            .map(|kana| {
                let mut spans: Vec<(usize, usize, &str, &str)> = digits
                    .iter()
                    .map(|&(start, end)| (start, end, "", &reading[start..end]))
                    .chain(
                        kana.iter()
                            .map(|(start, end, value)| (*start, *end, KANA, value.as_str())),
                    )
                    .collect();
                spans.sort_by_key(|&(start, ..)| start);
                let mut found = Self {
                    reading: String::new(),
                    values: Vec::new(),
                };
                let mut at = 0;
                for (start, end, name, value) in spans {
                    found.reading.push_str(&reading[at..start]);
                    found.reading.push(OPEN);
                    found.reading.push_str(name);
                    found.reading.push(CLOSE);
                    found.values.push(value.to_owned());
                    at = end;
                }
                found.reading.push_str(&reading[at..]);
                found
            })
            .collect()
    }
}

/// Each choice of `count` of `numerals`, in order and not overlapping, that
/// start at or after `from`, made only as it is taken: a reading full of kana
/// that read as numbers has far more choices than are looked up.
fn apart<'a>(
    numerals: &'a [(usize, usize, String)],
    count: usize,
    from: usize,
) -> Box<dyn Iterator<Item = Vec<&'a (usize, usize, String)>> + 'a> {
    if count == 0 {
        return Box::new(std::iter::once(Vec::new()));
    }
    Box::new(
        numerals
            .iter()
            .enumerate()
            .filter(move |(_, n)| n.0 >= from)
            .flat_map(move |(i, n)| {
                apart(&numerals[i + 1..], count - 1, n.1).map(move |mut rest| {
                    rest.insert(0, n);
                    rest
                })
            }),
    )
}

/// The byte ranges of the runs of digits in `reading`.
fn digit_runs(reading: &str) -> Vec<(usize, usize)> {
    let mut runs: Vec<(usize, usize)> = Vec::new();
    for (at, c) in reading.char_indices() {
        if !is_digit(c) {
            continue;
        }
        match runs.last_mut() {
            Some((_, end)) if *end == at => *end = at + c.len_utf8(),
            _ => runs.push((at, at + c.len_utf8())),
        }
    }
    runs
}

const DIGIT_KANA: [(&str, u128); 9] = [
    ("いち", 1),
    ("に", 2),
    ("さん", 3),
    ("よん", 4),
    ("ご", 5),
    ("ろく", 6),
    ("なな", 7),
    ("はち", 8),
    ("きゅう", 9),
];

/// Read only as the ones of a section: しひゃく and くせん are no numerals.
const ONES_KANA: [(&str, u128); 3] = [("し", 4), ("しち", 7), ("く", 9)];

/// A digit whose last sound doubles the consonant after it (いっこ).
const DOUBLED_KANA: [(&str, u128); 3] = [("いっ", 1), ("ろっ", 6), ("はっ", 8)];

/// The units that group four digits, with their powers of ten, largest
/// first.
const GROUPS: [(&str, u32); 4] = [("けい", 16), ("ちょう", 12), ("おく", 8), ("まん", 4)];

/// One way a part of a section is read: its length in bytes, its value, and
/// whether it ends on a doubled consonant.
type Part = (usize, u128, bool);

/// Each way `kana` begins with a kana numeral: the length in bytes of the
/// kana read, and the number as ASCII digits. Shortest first.
pub(crate) fn kana_numbers(kana: &str) -> Vec<(usize, String)> {
    let mut found: Vec<(usize, u128)> = Vec::new();
    numeral(kana, 0, 0, u32::MAX, &mut found);
    found.sort();
    found.dedup();
    found
        .into_iter()
        .map(|(len, value)| (len, value.to_string()))
        .collect()
}

/// Reads on from `at`, where `value` was read so far and the next group is
/// below 10^`below`.
fn numeral(kana: &str, at: usize, value: u128, below: u32, found: &mut Vec<(usize, u128)>) {
    for (len, section, doubled) in sections(&kana[at..]) {
        let end = at + len;
        if section == 0 {
            continue;
        }
        found.push((end, value + section));
        for (unit, power) in GROUPS.into_iter().filter(|&(_, p)| p < below) {
            // A doubled consonant reads on only into a unit beginning with
            // one that doubles: いっちょう, じゅっけい.
            if doubled && !matches!(unit, "ちょう" | "けい") {
                continue;
            }
            if kana[end..].starts_with(unit) {
                let value = value + section * 10u128.pow(power);
                found.push((end + unit.len(), value));
                numeral(kana, end + unit.len(), value, power, found);
            }
        }
    }
}

/// Each way `kana` begins with a section, a number below 10000. A section
/// ending on a doubled consonant must end the numeral, unless a unit follows.
fn sections(kana: &str) -> Vec<Part> {
    let mut out = Vec::new();
    for (t_len, t, t_doubled) in part(kana, &["せん", "ぜん"], 1000, &["せん"]) {
        if t_doubled {
            continue;
        }
        let rest = &kana[t_len..];
        for (h_len, h, h_doubled) in part(rest, &["ひゃく", "びゃく", "ぴゃく"], 100, &["ぴゃく"])
        {
            if h_doubled {
                continue;
            }
            let rest = &rest[h_len..];
            for (u_len, u, u_doubled) in tens(rest) {
                let ones = if u_doubled {
                    vec![(0, 0, false)]
                } else {
                    ones(&rest[u_len..])
                };
                for (o_len, o, o_doubled) in ones {
                    out.push((
                        t_len + h_len + u_len + o_len,
                        t + h + u + o,
                        u_doubled || o_doubled,
                    ));
                }
            }
        }
    }
    out
}

/// The thousands or hundreds of a section: none, the unit alone, or a digit
/// and the unit. `doubled_units` are the units a doubled digit goes before.
fn part(kana: &str, units: &[&str], unit: u128, doubled_units: &[&str]) -> Vec<Part> {
    let mut out = vec![(0, 0, false)];
    if kana.starts_with(units[0]) {
        out.push((units[0].len(), unit, false));
    }
    for (digit, value) in DIGIT_KANA.into_iter().filter(|&(_, v)| v > 1) {
        let Some(rest) = kana.strip_prefix(digit) else {
            continue;
        };
        for u in units.iter().filter(|u| rest.starts_with(**u)) {
            out.push((digit.len() + u.len(), value * unit, false));
        }
    }
    for (digit, value) in DOUBLED_KANA {
        let Some(rest) = kana.strip_prefix(digit) else {
            continue;
        };
        for u in doubled_units.iter().filter(|u| rest.starts_with(**u)) {
            out.push((digit.len() + u.len(), value * unit, false));
        }
    }
    out
}

fn tens(kana: &str) -> Vec<Part> {
    let mut out = vec![(0, 0, false)];
    let mut with = |prefix_len: usize, rest: &str, times: u128| {
        if rest.starts_with("じゅう") {
            out.push((prefix_len + "じゅう".len(), times * 10, false));
        }
        for doubled in ["じゅっ", "じっ"] {
            if rest.starts_with(doubled) {
                out.push((prefix_len + doubled.len(), times * 10, true));
            }
        }
    };
    with(0, kana, 1);
    for (digit, value) in DIGIT_KANA.into_iter().filter(|&(_, v)| v > 1) {
        if let Some(rest) = kana.strip_prefix(digit) {
            with(digit.len(), rest, value);
        }
    }
    out
}

fn ones(kana: &str) -> Vec<Part> {
    let mut out = vec![(0, 0, false)];
    for (digit, value) in DIGIT_KANA.into_iter().chain(ONES_KANA) {
        if kana.starts_with(digit) {
            out.push((digit.len(), value, false));
        }
    }
    for (digit, value) in DOUBLED_KANA {
        if kana.starts_with(digit) {
            out.push((digit.len(), value, true));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn placeholder(name: &str) -> String {
        format!("{OPEN}{name}{CLOSE}")
    }

    #[test]
    fn each_run_of_digits_in_a_reading_is_one_number() {
        let found = Numbers::find("1がつ２０にち").unwrap();
        assert_eq!(
            found.reading,
            format!("{}がつ{}にち", placeholder(""), placeholder(""))
        );
        assert_eq!(found.values, ["1", "２０"]);
    }

    #[test]
    fn ascii_and_full_width_digits_mix_in_one_number() {
        let found = Numbers::find("1２こ").unwrap();
        assert_eq!(found.reading, format!("{}こ", placeholder("")));
        assert_eq!(found.values, ["1２"]);
    }

    #[test]
    fn a_point_or_a_comma_splits_numbers() {
        assert_eq!(Numbers::find("1.5").unwrap().values, ["1", "5"]);
        assert_eq!(Numbers::find("1,000").unwrap().values, ["1", "000"]);
    }

    #[test]
    fn a_reading_without_digits_has_no_numbers() {
        assert!(Numbers::find("こ").is_none());
    }

    fn values(kana: &str) -> Vec<(&str, String)> {
        kana_numbers(kana)
            .into_iter()
            .map(|(len, value)| (&kana[..len], value))
            .collect()
    }

    fn value_of(kana: &str) -> Option<String> {
        values(kana)
            .into_iter()
            .find(|(read, _)| *read == kana)
            .map(|(_, value)| value)
    }

    #[test]
    fn a_kana_numeral_reads_as_its_digits() {
        let cases = [
            ("いち", "1"),
            ("さん", "3"),
            ("よん", "4"),
            ("し", "4"),
            ("なな", "7"),
            ("しち", "7"),
            ("きゅう", "9"),
            ("く", "9"),
            ("じゅう", "10"),
            ("じゅうに", "12"),
            ("にじゅう", "20"),
            ("ひゃく", "100"),
            ("ひゃくにじゅうさん", "123"),
            ("せん", "1000"),
            ("いちまん", "10000"),
            ("にまんさんぜん", "23000"),
            ("じゅうおく", "1000000000"),
            ("いっちょう", "1000000000000"),
        ];
        for (kana, digits) in cases {
            assert_eq!(value_of(kana).as_deref(), Some(digits), "{kana}");
        }
    }

    #[test]
    fn a_kana_numeral_reads_with_its_sounds_changed() {
        let cases = [
            ("さんびゃく", "300"),
            ("ろっぴゃく", "600"),
            ("はっぴゃく", "800"),
            ("さんぜん", "3000"),
            ("はっせん", "8000"),
            ("いっせん", "1000"),
            ("いっ", "1"),
            ("ろっ", "6"),
            ("はっ", "8"),
            ("じゅっ", "10"),
            ("じっ", "10"),
            ("にじゅっ", "20"),
        ];
        for (kana, digits) in cases {
            assert_eq!(value_of(kana).as_deref(), Some(digits), "{kana}");
        }
    }

    #[test]
    fn every_way_a_reading_begins_with_a_kana_numeral_is_found() {
        assert_eq!(
            values("じゅうにがつ"),
            [("じゅう", "10".to_owned()), ("じゅうに", "12".to_owned())]
        );
        assert_eq!(
            values("しちにん"),
            [("し", "4".to_owned()), ("しち", "7".to_owned())]
        );
    }

    #[test]
    fn what_no_numeral_is_written_as_reads_as_none() {
        for kana in [
            "いちに",
            "いっさん",
            "ひゃくひゃく",
            "じゅうじゅう",
            "しひゃく",
            "くせん",
        ] {
            assert_eq!(value_of(kana), None, "{kana}");
        }
        for kana in [
            "まん",
            "おく",
            "ちょう",
            "ひと",
            "ふた",
            "みっ",
            "れい",
            "ぜろ",
            "だんかい",
        ] {
            assert_eq!(value_of(kana), None, "{kana}");
        }
    }

    #[test]
    fn a_doubled_consonant_ends_a_numeral_unless_a_unit_follows() {
        assert_eq!(value_of("いっに"), None);
        assert_eq!(value_of("じゅっに"), None);
        assert_eq!(value_of("ろっぴゃくいっ").as_deref(), Some("601"));
    }

    #[test]
    fn a_kana_number_is_put_as_a_kana_placeholder() {
        let kana = placeholder(KANA);
        let found = Numbers::kana("さんだんかい");
        assert_eq!(
            found,
            [Numbers {
                reading: format!("{kana}だんかい"),
                values: vec!["3".to_owned()],
            }]
        );
    }

    #[test]
    fn digits_stay_as_a_digit_placeholder_beside_a_kana_number() {
        let found = Numbers::kana("1がつさんにち");
        assert!(found.contains(&Numbers {
            reading: format!("{}がつ{}にち", placeholder(""), placeholder(KANA)),
            values: vec!["1".to_owned(), "3".to_owned()],
        }));
    }

    #[test]
    fn a_reading_is_put_every_way_its_kana_numbers_split() {
        let kana = placeholder(KANA);
        let readings: Vec<_> = Numbers::kana("じゅうにがつ")
            .into_iter()
            .map(|n| (n.reading, n.values))
            .collect();
        for expected in [
            (format!("{kana}にがつ"), vec!["10"]),
            (format!("{kana}がつ"), vec!["12"]),
            (format!("じゅう{kana}がつ"), vec!["2"]),
        ] {
            let expected = (
                expected.0,
                expected.1.iter().map(|v| v.to_string()).collect(),
            );
            assert!(readings.contains(&expected), "{expected:?} in {readings:?}");
        }
    }

    #[test]
    fn every_way_with_one_kana_number_is_kept_past_the_limit() {
        let reading = format!("{}だんかい", "さんが".repeat(MAX_KANA_READINGS + 8));
        let found = Numbers::kana(&reading);
        let ones = found.iter().filter(|n| n.values.len() == 1).count();
        assert_eq!(ones, MAX_KANA_READINGS + 8);
        assert_eq!(found.len(), ones);
    }

    #[test]
    fn ways_with_more_kana_numbers_fill_what_is_left_of_the_limit() {
        let found = Numbers::kana("さんがさんがさん");
        let counts: Vec<usize> = found.iter().map(|n| n.values.len()).collect();
        assert_eq!(counts, [1, 1, 1, 2, 2, 2, 3]);
        let kana = placeholder(KANA);
        let all = Numbers::kana("にねんさんがつよんにち")
            .into_iter()
            .find(|n| n.values == ["2", "3", "4"])
            .unwrap();
        assert_eq!(all.reading, format!("{kana}ねん{kana}がつ{kana}にち"));
    }

    #[test]
    fn a_reading_without_kana_numbers_has_no_kana_placeholders() {
        assert!(Numbers::kana("だんかい").is_empty());
        assert!(Numbers::kana("12こ").is_empty());
    }
}
