//! The words of a user custom dictionary as the settings app lists, adds,
//! edits and removes them: the words registering one writes, checked before
//! they are written.

use std::fs::OpenOptions;
use std::io;
use std::path::Path;

use crate::OkuriHead;
use crate::TextDictionary;
use crate::numeric::KANA;
use crate::okuri::okuri_row;
use crate::placeholder::{CLOSE, Functions, OPEN, fill_all, fits, placeholders};
use crate::registration::numbered;
use crate::text_dictionary::{ItemLine, mark_placeholders, nfc, show_placeholders};
use crate::user_custom::{
    FileLock, append_line, lines, open_private, read_if_exists, remove_lines, rewrite,
};

/// The numbers a word with numbers is shown with before it is added.
const SAMPLE_NUMBERS: [&str; 3] = ["1", "3", "12"];

/// A word of a user custom dictionary without a conjugation type or a cost,
/// its okurigana marked by a kana: what registering a word writes.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct UserWord {
    /// The reading before any okurigana, with numbers' placeholders as an
    /// item holds them.
    stem: String,
    /// The first kana of the okurigana.
    okurigana: Option<char>,
    /// The whole surface, okurigana included, with placeholders as an item
    /// holds them.
    surface: String,
}

/// Why a word cannot be written. The reading and the surface a person typed
/// stay out of the message; only a character or a function's name is told.
#[derive(Debug, thiserror::Error)]
pub enum WordError {
    #[error("the reading is empty")]
    EmptyReading,
    /// A character other than hiragana, `ー`, `*` and a number's `{}`.
    #[error("the reading has {0:?}, which is not hiragana")]
    NotKana(char),
    /// A `{` or `}` that is not a number's `{}`.
    #[error("a brace of the reading is not a number's {{}}")]
    Brace,
    #[error("the reading marks okurigana more than once")]
    ManyMarks,
    #[error("the reading has nothing before its okurigana")]
    EmptyStem,
    /// What follows the `*` is not one kana okurigana can start with.
    #[error("the okurigana is not one kana it can start with")]
    Okurigana,
    #[error("a word with okurigana has a placeholder")]
    OkuriganaWithPlaceholder,
    #[error("the surface is empty")]
    EmptySurface,
    /// A control character, or one the dictionary keeps for placeholders.
    #[error("the surface has {0:?}, which a word cannot have")]
    Unusable(char),
    /// The surface of a word with okurigana does not end in its kana.
    #[error("the surface does not end in the okurigana {0}")]
    SurfaceOkurigana(char),
    /// A placeholder takes a number the reading does not have.
    #[error("a placeholder takes a number the reading does not have")]
    Placeholders,
    #[error("there is no function {0:?}")]
    UnknownFunction(String),
    /// The line would not read back as the word, which should never be.
    #[error("the line does not read back as the word")]
    Invalid,
    #[error("the word is registered already")]
    Exists,
    #[error("the word is no longer in the dictionary")]
    Gone,
    #[error(transparent)]
    Io(#[from] io::Error),
}

impl UserWord {
    /// The word of `reading` and `surface` as a person writes them, as
    /// [`UserWord::reading`] and [`UserWord::surface`] give them back: the
    /// reading in hiragana with `*` before the first kana of okurigana
    /// (`か*く`) and `{}` for each number (`{}こ`); the surface with each
    /// `{…}` that may be a placeholder made one, as registering does. Both
    /// are trimmed and normalized to NFC. A placeholder's function must be
    /// one `has_function` knows, unless it names none.
    pub fn new(
        reading: &str,
        surface: &str,
        has_function: impl Fn(&str) -> bool,
    ) -> Result<Self, WordError> {
        let reading = nfc(reading.trim());
        if reading.is_empty() {
            return Err(WordError::EmptyReading);
        }
        let (stem, okurigana) = match reading.split_once('*') {
            None => (reading.as_str(), None),
            Some((_, rest)) if rest.contains('*') => return Err(WordError::ManyMarks),
            Some((stem, kana)) => (stem, Some(kana)),
        };
        let (stem, numbers) = stem_of(stem)?;
        if stem.is_empty() {
            return Err(WordError::EmptyStem);
        }
        let okurigana = okurigana.map(okurigana_of).transpose()?;

        let surface = nfc(surface.trim());
        if surface.is_empty() {
            return Err(WordError::EmptySurface);
        }
        if let Some(c) = surface
            .chars()
            .find(|&c| c.is_control() || c == OPEN || c == CLOSE)
        {
            return Err(WordError::Unusable(c));
        }
        let surface = mark_placeholders(&surface).unwrap_or(surface);
        let marked = placeholders(&surface).ok_or(WordError::Invalid)?;
        if okurigana.is_some() && (numbers > 0 || !marked.is_empty()) {
            return Err(WordError::OkuriganaWithPlaceholder);
        }
        if let Some(missing) = marked
            .iter()
            .find(|p| !p.name.is_empty() && !has_function(p.name))
        {
            return Err(WordError::UnknownFunction(missing.name.to_owned()));
        }
        if !fits(&surface, numbers) {
            return Err(WordError::Placeholders);
        }
        if let Some(kana) = okurigana
            && !surface.ends_with(kana)
        {
            return Err(WordError::SurfaceOkurigana(kana));
        }

        let word = Self {
            stem,
            okurigana,
            surface,
        };
        // Written as checked here, the line should always read back.
        if !word.is(&word.line()) {
            return Err(WordError::Invalid);
        }
        Ok(word)
    }

    /// The reading as a person writes it: `か*く`, `{}こ`.
    pub fn reading(&self) -> String {
        let mut reading = show_placeholders(&self.stem);
        if let Some(kana) = self.okurigana {
            reading.push('*');
            reading.push(kana);
        }
        reading
    }

    /// The surface as a person writes it, placeholders in braces.
    pub fn surface(&self) -> String {
        show_placeholders(&self.surface)
    }

    /// This word, which has no placeholders, with each number of its reading
    /// made a placeholder, when its surface writes every one (さんだんかい
    /// 三段階 → {kana}だんかい {kanji}段階), for a person to add in its
    /// place. `None` when there is no such word.
    pub fn numbered(&self, functions: &dyn Functions) -> Option<Self> {
        if self.okurigana.is_some() || self.stem.contains(OPEN) || self.surface.contains(OPEN) {
            return None;
        }
        let registration = numbered(&self.stem, &self.surface, functions)?;
        let word = Self {
            stem: registration.reading,
            okurigana: None,
            surface: registration.surface,
        };
        word.is(&word.line()).then_some(word)
    }

    /// What a word with numbers gives for a few numbers, each value of each
    /// in turn; none for a word without numbers.
    pub fn samples(&self, functions: &dyn Functions) -> Vec<String> {
        let numbers = self.stem.matches(OPEN).count();
        if numbers == 0 {
            return Vec::new();
        }
        SAMPLE_NUMBERS
            .iter()
            .flat_map(|sample| {
                let values = vec![(*sample).to_owned(); numbers];
                fill_all(&self.surface, &values, &self.stem, Some(functions))
            })
            .map(|filled| filled.text)
            .collect()
    }

    /// The line of a user custom dictionary that registers the word.
    fn line(&self) -> String {
        let okurigana = self.okurigana.map(String::from);
        ItemLine {
            reading: &self.stem,
            okurigana: okurigana.as_deref(),
            surface: &self.surface,
            ..ItemLine::default()
        }
        .to_string()
    }

    /// Whether `line` of a user custom dictionary registers this word.
    fn is(&self, line: &str) -> bool {
        Self::of_line(line).as_ref() == Some(self)
    }

    /// The word `line` registers, if it is one registering writes.
    fn of_line(line: &str) -> Option<Self> {
        let registration = TextDictionary::registration(line)?;
        let (stem, head, surface) = registration.plain()?;
        let okurigana = match head {
            None => None,
            Some(OkuriHead::Kana(kana)) => Some(kana),
            Some(OkuriHead::Row(_)) => return None,
        };
        Some(Self {
            stem: stem.to_owned(),
            okurigana,
            surface: surface.to_owned(),
        })
    }
}

/// The reading before the okurigana, with each `{}` and `{kana}` made a
/// number's placeholder, and how many numbers it has.
fn stem_of(written: &str) -> Result<(String, usize), WordError> {
    let mut stem = String::new();
    let mut numbers = 0;
    let mut rest = written;
    while let Some(c) = rest.chars().next() {
        rest = &rest[c.len_utf8()..];
        match c {
            '{' => {
                let (name, after) = rest.split_once('}').ok_or(WordError::Brace)?;
                if !matches!(name, "" | KANA) {
                    return Err(WordError::Brace);
                }
                stem.push(OPEN);
                stem.push_str(name);
                stem.push(CLOSE);
                numbers += 1;
                rest = after;
            }
            '}' => return Err(WordError::Brace),
            c if is_reading_kana(c) => stem.push(c),
            c => return Err(WordError::NotKana(c)),
        }
    }
    Ok((stem, numbers))
}

/// The kana written after the `*`: one that starts okurigana.
fn okurigana_of(written: &str) -> Result<char, WordError> {
    let mut chars = written.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) if okuri_row(c).is_some() => Ok(c),
        (Some(c), _) if !is_reading_kana(c) && c != '{' && c != '}' => Err(WordError::NotKana(c)),
        _ => Err(WordError::Okurigana),
    }
}

/// Hiragana, its iteration marks, and the long vowel mark.
fn is_reading_kana(c: char) -> bool {
    matches!(c, '\u{3041}'..='\u{3096}' | 'ゝ' | 'ゞ' | 'ー')
}

/// The words of the user custom dictionary at `path` that [`UserWord`] can
/// write, each once, by reading, then surface. A missing file has none.
pub fn user_words(path: impl AsRef<Path>) -> io::Result<Vec<UserWord>> {
    let bytes = read_if_exists(path.as_ref())?.unwrap_or_default();
    let mut words: Vec<UserWord> = Vec::new();
    for word in lines(&bytes).filter_map(|(_, text)| UserWord::of_line(text?)) {
        // One a person wrote another way, such as a literal `{` that would
        // be read as a placeholder, could not be written back as it is.
        let again = UserWord::new(&word.reading(), &word.surface(), |_| true);
        if again.is_ok_and(|again| again == word) && !words.contains(&word) {
            words.push(word);
        }
    }
    words.sort_by_cached_key(|w| (w.reading(), w.surface()));
    Ok(words)
}

/// Appends the line of `word` to the user custom dictionary at `path`, as
/// registering does, unless a line registers it already. The file is
/// flushed to the disk after its lock is let go, so the IME waits only
/// while the line is read and appended.
pub fn add_user_word(path: impl AsRef<Path>, word: &UserWord) -> Result<(), WordError> {
    let path = path.as_ref();
    let file = {
        let _lock = FileLock::hold(path)?;
        let bytes = read_if_exists(path)?.unwrap_or_default();
        if lines(&bytes).any(|(_, text)| text.is_some_and(|text| word.is(text))) {
            return Err(WordError::Exists);
        }
        let mut file = open_private(
            OpenOptions::new().read(true).append(true).create(true),
            path,
        )?;
        append_line(&mut file, &word.line())?;
        file
    };
    file.sync_all()?;
    Ok(())
}

/// Writes `new` in place of `old` in the user custom dictionary at `path`:
/// the last line of `old` becomes the line of `new` where it is, so the word
/// keeps its place among the words of its reading, and any other line of
/// `old` goes. Every other line stays byte for byte. Nothing changes when
/// `old` has no line any more, or another line registers `new` already.
pub fn edit_user_word(
    path: impl AsRef<Path>,
    old: &UserWord,
    new: &UserWord,
) -> Result<(), WordError> {
    if old == new {
        return Ok(());
    }
    let line = new.line();
    rewrite(path.as_ref(), |bytes| {
        let mut last = None;
        for (i, (_, text)) in lines(bytes).enumerate() {
            match text {
                Some(text) if new.is(text) => return Err(WordError::Exists),
                Some(text) if old.is(text) => last = Some(i),
                _ => {}
            }
        }
        let last = last.ok_or(WordError::Gone)?;
        let mut edited = Vec::with_capacity(bytes.len() + line.len());
        for (i, (raw, text)) in lines(bytes).enumerate() {
            match text {
                Some(text) if i == last => edited.extend(with_text(raw, text, &line)),
                Some(text) if old.is(text) => {}
                _ => edited.extend_from_slice(raw),
            }
        }
        Ok(Some(edited))
    })
}

/// Takes every line of `word` out of the user custom dictionary at `path`;
/// the other lines, hide lines of its pair among them, stay byte for byte.
pub fn remove_user_word(path: impl AsRef<Path>, word: &UserWord) -> io::Result<()> {
    remove_lines(path.as_ref(), |text| word.is(text))
}

/// `raw`, a line of a file whose text is `text`, with `new` for the text: a
/// byte order mark before and the line ending after stay.
fn with_text(raw: &[u8], text: &str, new: &str) -> Vec<u8> {
    // `text` is a part of `raw`.
    let start = text.as_ptr() as usize - raw.as_ptr() as usize;
    let end = start + text.len();
    [&raw[..start], new.as_bytes(), &raw[end..]].concat()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::placeholder::{Call, Value};

    fn word(reading: &str, surface: &str) -> Result<UserWord, WordError> {
        UserWord::new(reading, surface, |name| name == "kanji")
    }

    fn line(reading: &str, surface: &str) -> String {
        word(reading, surface).unwrap().line()
    }

    #[test]
    fn a_plain_word_is_written_as_a_word_line() {
        assert_eq!(line("きしゃ", "記者"), "きしゃ\t記者");
        assert_eq!(line(" らーめん ", " ラーメン "), "らーめん\tラーメン");
    }

    #[test]
    fn a_word_with_okurigana_is_written_with_its_first_kana() {
        assert_eq!(line("か*く", "書く"), "か*く\t書く");
        assert_eq!(line("か*っ", "勝っ"), "か*っ\t勝っ");
        let written = word("か*く", "書く").unwrap();
        assert_eq!(written.reading(), "か*く");
        assert_eq!(written.surface(), "書く");
    }

    #[test]
    fn numbers_and_placeholders_are_written_as_an_item_with_them() {
        assert_eq!(line("{}こ", "{kanji}個"), "{}こ\t{kanji}個");
        assert_eq!(
            line("{}がつ{}にち", "{}月{1:}日"),
            "{}がつ{}にち\t{}月{1:}日"
        );
        assert_eq!(line("きょう", "{-:}"), "きょう\t{-:}");
        let written = word("{}こ", "{kanji}個").unwrap();
        assert_eq!(written.reading(), "{}こ");
        assert_eq!(written.surface(), "{kanji}個");
    }

    #[test]
    fn a_reading_may_take_a_number_typed_in_kana() {
        assert_eq!(
            line("{kana}だんかい", "{kanji}段階"),
            "{kana}だんかい\t{kanji}段階"
        );
        assert!(matches!(word("{x}こ", "{kanji}個"), Err(WordError::Brace)));
    }

    /// `kanji` writes 1 to 19 in kanji; `half-num` gives its source.
    struct Kanji;

    impl Functions for Kanji {
        fn has(&self, name: &str) -> bool {
            matches!(name, "kanji" | "half-num")
        }

        fn call(&self, call: &Call) -> Option<Vec<Value>> {
            const ONES: [&str; 10] = ["", "一", "二", "三", "四", "五", "六", "七", "八", "九"];
            let text = match call.name {
                "half-num" => call.source.to_owned(),
                _ => match call.source.parse::<usize>().ok()? {
                    n @ 1..=9 => ONES[n].to_owned(),
                    n @ 10..=19 => format!("十{}", ONES[n - 10]),
                    _ => return None,
                },
            };
            Some(vec![text.into()])
        }
    }

    #[test]
    fn a_word_writing_the_number_of_its_reading_is_numbered() {
        let numbered = word("さんだんかい", "三段階")
            .unwrap()
            .numbered(&Kanji)
            .unwrap();
        assert_eq!(numbered.reading(), "{kana}だんかい");
        assert_eq!(numbered.surface(), "{kanji}段階");
        assert_eq!(numbered.line(), "{kana}だんかい\t{kanji}段階");
    }

    #[test]
    fn a_word_not_writing_its_numbers_or_with_placeholders_is_not_numbered() {
        for (reading, surface) in [
            ("さんま", "秋刀魚"),
            ("{}こ", "{kanji}個"),
            ("か*く", "書く"),
        ] {
            assert_eq!(
                word(reading, surface).unwrap().numbered(&Kanji),
                None,
                "{reading}"
            );
        }
    }

    #[test]
    fn a_numeric_word_shows_what_it_gives_for_a_few_numbers() {
        let numbered = word("{kana}だんかい", "{kanji}段階").unwrap();
        assert_eq!(numbered.samples(&Kanji), ["一段階", "三段階", "十二段階"]);
        assert_eq!(
            word("さんま", "秋刀魚").unwrap().samples(&Kanji),
            Vec::<String>::new()
        );
    }

    #[test]
    fn a_brace_that_is_no_placeholder_stays_a_brace() {
        assert_eq!(line("かっこ", "{a:b}"), "かっこ\t\\{a:b}");
        assert_eq!(line("かっこ", "}"), "かっこ\t}");
    }

    #[test]
    fn a_reading_is_normalized_before_it_is_checked() {
        assert_eq!(line("か\u{3099}く", "学"), "がく\t学");
    }

    #[test]
    fn a_surface_is_trimmed_and_normalized_as_a_reading_is() {
        assert_eq!(line("がっこう", " カ\u{3099}ッコウ "), "がっこう\tガッコウ");
    }

    #[test]
    fn a_reading_must_be_hiragana() {
        assert!(matches!(word("", "記者"), Err(WordError::EmptyReading)));
        assert!(matches!(word("  ", "記者"), Err(WordError::EmptyReading)));
        assert!(matches!(
            word("キシャ", "記者"),
            Err(WordError::NotKana('キ'))
        ));
        assert!(matches!(
            word("kisha", "記者"),
            Err(WordError::NotKana('k'))
        ));
        assert!(matches!(
            word("き しゃ", "記者"),
            Err(WordError::NotKana(' '))
        ));
        assert!(matches!(word("3こ", "三個"), Err(WordError::NotKana('3'))));
        assert!(matches!(word("{こ", "個"), Err(WordError::Brace)));
        assert!(matches!(word("こ}", "個"), Err(WordError::Brace)));
    }

    #[test]
    fn okurigana_is_one_kana_after_a_stem() {
        assert!(matches!(word("か*く*", "書く"), Err(WordError::ManyMarks)));
        assert!(matches!(word("*く", "く"), Err(WordError::EmptyStem)));
        assert!(matches!(word("か*", "書"), Err(WordError::Okurigana)));
        assert!(matches!(
            word("か*くよ", "書くよ"),
            Err(WordError::Okurigana)
        ));
        assert!(matches!(word("か*ー", "書ー"), Err(WordError::Okurigana)));
        assert!(matches!(word("か*k", "書"), Err(WordError::NotKana('k'))));
        assert!(matches!(
            word("か*く", "書け"),
            Err(WordError::SurfaceOkurigana('く'))
        ));
    }

    #[test]
    fn a_word_with_okurigana_has_no_placeholders() {
        assert!(matches!(
            word("{}か*く", "{}書く"),
            Err(WordError::OkuriganaWithPlaceholder)
        ));
        assert!(matches!(
            word("か*く", "{kanji}く"),
            Err(WordError::OkuriganaWithPlaceholder)
        ));
    }

    #[test]
    fn a_surface_must_be_a_word_on_one_line() {
        assert!(matches!(word("きしゃ", ""), Err(WordError::EmptySurface)));
        assert!(matches!(word("きしゃ", " "), Err(WordError::EmptySurface)));
        assert!(matches!(
            word("きしゃ", "記\t者"),
            Err(WordError::Unusable('\t'))
        ));
        assert!(matches!(
            word("きしゃ", "記\n者"),
            Err(WordError::Unusable('\n'))
        ));
        assert!(matches!(
            word("きしゃ", "記\u{FDD0}者"),
            Err(WordError::Unusable('\u{FDD0}'))
        ));
    }

    #[test]
    fn placeholders_take_only_the_numbers_the_reading_has() {
        assert!(matches!(
            word("こ", "{kanji}個"),
            Err(WordError::Placeholders)
        ));
        assert!(matches!(
            word("{}こ", "{1:}個"),
            Err(WordError::Placeholders)
        ));
        assert!(word("{}こ", "個").is_ok(), "a number may go unused");
    }

    #[test]
    fn a_placeholder_names_a_function_there_is() {
        assert!(matches!(
            word("わら", "{笑}"),
            Err(WordError::UnknownFunction(name)) if name == "笑"
        ));
        assert!(
            word("{}こ", "{}個").is_ok(),
            "no name gives the number back"
        );
    }

    #[test]
    fn a_line_with_a_row_a_conjugation_or_a_cost_is_no_word_to_write() {
        for line in [
            "か*k\t書",
            "か\t書\t五段-カ行",
            "きしゃ\t記者\t\t3",
            "!きしゃ\t汽車",
            "# き",
        ] {
            assert_eq!(UserWord::of_line(line), None, "{line}");
        }
    }

    #[test]
    fn a_line_keeps_its_ending_and_byte_order_mark_when_its_text_changes() {
        let bytes = "\u{feff}かく\t書く\r\nきしゃ\t記者".as_bytes();
        let changed: Vec<u8> = lines(bytes)
            .flat_map(|(raw, text)| with_text(raw, text.unwrap(), "あ\tい"))
            .collect();
        assert_eq!(changed, "\u{feff}あ\tい\r\nあ\tい".as_bytes());
    }
}
