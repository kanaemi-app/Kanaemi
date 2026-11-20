//! The item a word registered with placeholders is written as.

use crate::numeric::Numbers;
use crate::placeholder::{CLOSE, OPEN, fits};
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

/// Each way a reading's numbers may be put as placeholders, the one that
/// puts the most of the reading in numbers first, then the one with fewer
/// numbers: digits as typed, and kana numbers read in full (十二 for
/// じゅうに rather than 十 and 二).
pub(crate) fn numbers_of(reading: &str) -> Vec<Numbers> {
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
