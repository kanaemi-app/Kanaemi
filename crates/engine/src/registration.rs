//! The item a word registered with placeholders is written as, and the one a
//! word writing the numbers of its reading would be with them.

use crate::numeric::Numbers;
use crate::placeholder::{CLOSE, Call, Functions, OPEN, fits};
use crate::text_dictionary::mark_placeholders;

/// A word registered with placeholders, written as an item with them.
pub(crate) struct PlaceholderRegistration {
    /// With a placeholder in place of each number.
    pub(crate) reading: String,
    pub(crate) surface: String,
    /// As typed, kana numbers as digits.
    pub(crate) numbers: Vec<String>,
}

/// The item a word is registered as when its surface has placeholders.
/// `None` for a word registered as typed: with okurigana, without
/// placeholders, or with placeholders that would make an invalid line.
///
/// A surface without placeholders is never made one here, even when it
/// writes the number of its reading: 一緒 for いっしょ would become an item
/// that gives 二緒 for にしょ. Only a person who sees the item it would be
/// picks that.
pub(crate) fn placeholder_registration(
    reading: &str,
    okurigana: Option<&str>,
    surface: &str,
) -> Option<PlaceholderRegistration> {
    if okurigana.is_some() {
        return None;
    }
    let surface = mark_placeholders(surface)?;
    // Kana read as numbers only when the surface takes a number: きょう
    // with {-:date} stays the word of its reading.
    let as_typed = Numbers {
        reading: reading.to_owned(),
        values: Vec::new(),
    };
    let numbers = match Numbers::find(reading) {
        Some(digits) if fits(&surface, digits.values.len()) => digits,
        None if fits(&surface, 0) => as_typed,
        _ => {
            // じゅうにち for {kanji}日 is 十 and にち, not 十, に and ち with
            // the last number left out of the surface.
            let fitting = |n: &Numbers| fits(&surface, n.values.len());
            let uses_all = |n: &Numbers| !fits(&surface, n.values.len() - 1);
            let mut ways: Vec<Numbers> = numbers_of(reading).into_iter().filter(fitting).collect();
            let at = ways.iter().position(uses_all).unwrap_or(0);
            (at < ways.len()).then(|| ways.swap_remove(at))?
        }
    };
    Some(PlaceholderRegistration {
        reading: numbers.reading,
        surface,
        numbers: numbers.values,
    })
}

/// The functions that write a number one way, which a word's numbers are
/// looked for in, the one to pick first where two write it the same (三 is
/// both `kanji` and `kanji-num`).
const NUMBER_WAYS: [&str; 6] = [
    "half-num",
    "wide-num",
    "kanji",
    "kanji-num",
    "daiji",
    "grouped-num",
];

/// The numeric item a word of `reading` and `surface`, which has no
/// placeholders, would be with each written number made the placeholder of
/// its way; `None` unless the surface writes every number of the reading.
pub(crate) fn numbered(
    reading: &str,
    surface: &str,
    functions: &dyn Functions,
) -> Option<PlaceholderRegistration> {
    numbers_of(reading).into_iter().find_map(|numbers| {
        let surface = written_numbers(surface, &numbers.values, functions)?;
        Some(PlaceholderRegistration {
            reading: numbers.reading,
            surface,
            numbers: numbers.values,
        })
    })
}

/// `surface` with each of `numbers`, in turn, made the placeholder of the
/// first way of [`NUMBER_WAYS`] it is written in; `None` unless every number
/// is found.
fn written_numbers(surface: &str, numbers: &[String], functions: &dyn Functions) -> Option<String> {
    let mut out = String::new();
    let mut rest = surface;
    for number in numbers {
        let (at, way, text) = NUMBER_WAYS
            .iter()
            .filter(|way| functions.has(way))
            .filter_map(|&way| {
                let call = Call {
                    name: way,
                    source: number,
                    argument: None,
                };
                let text = functions.call(&call)?.into_iter().next()?.text;
                Some((rest.find(&text).filter(|_| !text.is_empty())?, way, text))
            })
            .min_by_key(|&(at, ..)| at)?;
        out.push_str(&rest[..at]);
        out.push(OPEN);
        out.push_str(way);
        out.push(CLOSE);
        rest = &rest[at + text.len()..];
    }
    (!numbers.is_empty()).then(|| out + rest)
}

/// Each way a reading's numbers may be put as placeholders, the one that
/// puts the most of the reading in numbers first, then the one with fewer
/// numbers: digits as typed, and kana numbers read in full (十二 for
/// じゅうに rather than 十 and 二).
fn numbers_of(reading: &str) -> Vec<Numbers> {
    let mut ways: Vec<Numbers> = Numbers::find(reading)
        .into_iter()
        .chain(Numbers::kana(reading))
        .collect();
    let left = |n: &Numbers| {
        let mut inside = false;
        n.reading
            .chars()
            .filter(|&c| {
                inside = match c {
                    OPEN => true,
                    CLOSE => false,
                    _ => inside,
                };
                !inside && c != CLOSE
            })
            .count()
    };
    ways.sort_by_key(|n| (left(n), n.values.len()));
    ways
}
