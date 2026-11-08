//! Whether another dictionary already offers a word of the user custom
//! dictionary, found by converting as the IME does with every word the
//! settings app lists taken out, so a word shown as offered still converts
//! once it is removed.

use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use kanaemi_config::{DictionarySource, MODEL_FILE, builtin_dictionary, dictionary_sources};
use kanaemi_core::Converter;
use kanaemi_engine::{
    Dictionary, Engine, LineSink, RankingModel, Selections, Slot, TextDictionary, UserWord,
    mark_placeholders, open_dictionary, user_words,
};

/// Where a word is offered without the user custom dictionary's words.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Offer {
    /// The name of the dictionary whose candidate it is.
    pub dictionary: String,
    /// Its place among the candidates, from 1.
    pub place: usize,
}

/// What the check reads, as the IME finds it.
#[derive(Clone, Debug, PartialEq)]
pub struct Sources {
    pub dir: PathBuf,
    /// The dictionary list as the settings write it, if they do.
    pub listed: Option<Vec<DictionarySource>>,
    pub custom: PathBuf,
    pub selections: PathBuf,
    pub ranking_model: bool,
}

/// The engine that converts with `sources` as the IME would, but with only
/// the hide lines of the user custom dictionary: what is left once every word
/// the settings app lists is removed. Each dictionary is named by `name`.
pub fn engine_without_words(
    sources: &Sources,
    name: impl Fn(&DictionarySource) -> String,
) -> Engine {
    let slots = dictionary_sources(&sources.dir, sources.listed.as_deref())
        .into_iter()
        .filter_map(|source| {
            let dictionary = match &source {
                DictionarySource::UserCustom => return Some(Slot::UserCustom),
                DictionarySource::File(path) => open(path)?,
                DictionarySource::Converted { binary, text } => {
                    open(binary).or_else(|| open(text))?
                }
                DictionarySource::Builtin(builtin) => {
                    Box::new(TextDictionary::parse(builtin_dictionary(builtin)?.text).0)
                }
            };
            Some(Slot::Named(name(&source), dictionary))
        })
        .collect::<Vec<_>>();
    let user = fs::read(&sources.custom).unwrap_or_default();
    let mut engine = Engine::new(slots, hide_lines(&user), Discard);
    if sources.ranking_model {
        engine.set_model(
            RankingModel::open(sources.dir.join(MODEL_FILE))
                .ok()
                .map(Arc::new),
        );
    }
    if let Ok(text) = fs::read_to_string(&sources.selections) {
        engine.replace_selections(Selections::parse(text));
    }
    engine
}

/// A dictionary the IME would skip, as it cannot be read, is skipped.
fn open(path: &Path) -> Option<Box<dyn Dictionary>> {
    open_dictionary(path).ok().map(|(dictionary, _)| dictionary)
}

/// The user custom dictionary of `bytes` with only its hide lines, which
/// removing a word leaves.
fn hide_lines(bytes: &[u8]) -> TextDictionary {
    let text = String::from_utf8_lossy(bytes);
    let hides: String = text
        .trim_start_matches('\u{feff}')
        .split_inclusive('\n')
        .filter(|line| line.starts_with('!'))
        .collect();
    TextDictionary::parse_user_custom(hides).0
}

/// Takes no line in: the engine registers nothing while it checks.
struct Discard;

impl LineSink for Discard {
    fn append(&mut self, _: &str) -> io::Result<()> {
        Ok(())
    }
}

/// Where `engine` offers `word`: the candidate of its surface, converting its
/// reading, with okurigana marked as the word marks it. A word with
/// placeholders is not looked at: what it converts to depends on the number
/// typed and on the functions.
pub fn offer(engine: &Engine, word: &UserWord) -> Option<Offer> {
    let reading = word.reading();
    let surface = word.surface();
    if mark_placeholders(&reading).is_some() || mark_placeholders(&surface).is_some() {
        return None;
    }
    let (reading, okurigana) = match reading.split_once('*') {
        Some((stem, okurigana)) => (format!("{stem}{okurigana}"), Some(okurigana)),
        None => (reading.clone(), None),
    };
    engine
        .convert(&reading, okurigana)
        .into_iter()
        .enumerate()
        .find(|(_, candidate)| candidate.surface == surface)
        .and_then(|(at, candidate)| {
            Some(Offer {
                dictionary: candidate.source?,
                place: at + 1,
            })
        })
}

/// Every word the settings app lists that another dictionary offers.
pub fn offers(
    sources: &Sources,
    name: impl Fn(&DictionarySource) -> String,
) -> io::Result<HashMap<UserWord, Offer>> {
    let words = user_words(&sources.custom)?;
    let engine = engine_without_words(sources, name);
    Ok(words
        .into_iter()
        .filter_map(|word| Some((word.clone(), offer(&engine, &word)?)))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn word(reading: &str, surface: &str) -> UserWord {
        UserWord::new(reading, surface, |_| true).unwrap()
    }

    /// A settings folder with `custom` and the text dictionaries `files`,
    /// listed after the user custom dictionary in their order.
    fn folder(name: &str, custom: &str, files: &[(&str, &str)]) -> Sources {
        let dir = std::env::temp_dir().join(format!(
            "kanaemi-settings-offered-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("dictionaries")).unwrap();
        fs::write(dir.join("custom.tsv"), custom).unwrap();
        let mut listed = vec![DictionarySource::UserCustom];
        for (file, text) in files {
            let path = dir.join("dictionaries").join(file);
            fs::write(&path, text).unwrap();
            listed.push(DictionarySource::File(path));
        }
        Sources {
            custom: dir.join("custom.tsv"),
            selections: dir.join("selections.tsv"),
            dir,
            listed: Some(listed),
            ranking_model: false,
        }
    }

    fn file_name(source: &DictionarySource) -> String {
        match source {
            DictionarySource::File(path) => {
                path.file_name().unwrap().to_string_lossy().into_owned()
            }
            _ => String::new(),
        }
    }

    fn offered(sources: &Sources, reading: &str, surface: &str) -> Option<Offer> {
        offer(
            &engine_without_words(sources, file_name),
            &word(reading, surface),
        )
    }

    fn at(dictionary: &str, place: usize) -> Option<Offer> {
        Some(Offer {
            dictionary: dictionary.to_owned(),
            place,
        })
    }

    #[test]
    fn a_word_another_dictionary_has_is_offered_by_it_at_its_place() {
        let sources = folder(
            "has",
            "むへんかん\t無変換\n",
            &[("base.tsv", "むへんかん\t無返還\nむへんかん\t無変換\n")],
        );
        assert_eq!(offered(&sources, "むへんかん", "無変換"), at("base.tsv", 1));
        assert_eq!(offered(&sources, "むへんかん", "無返還"), at("base.tsv", 2));
    }

    #[test]
    fn a_word_only_the_user_custom_dictionary_has_is_not_offered() {
        let sources = folder(
            "only",
            "こうしきさいと\t公式サイト\n",
            &[("base.tsv", "こうしき\t公式\n")],
        );
        assert_eq!(offered(&sources, "こうしきさいと", "公式サイト"), None);
    }

    #[test]
    fn the_first_dictionary_in_the_list_offering_it_is_named() {
        let sources = folder(
            "first",
            "",
            &[("a.tsv", "きしゃ\t記者\n"), ("b.tsv", "きしゃ\t記者\n")],
        );
        assert_eq!(offered(&sources, "きしゃ", "記者"), at("a.tsv", 1));
    }

    #[test]
    fn a_dictionary_off_the_list_does_not_offer_it() {
        let mut sources = folder("off", "", &[("a.tsv", "きしゃ\t記者\n")]);
        sources.listed = Some(vec![DictionarySource::UserCustom]);
        assert_eq!(offered(&sources, "きしゃ", "記者"), None);
    }

    #[test]
    fn a_form_another_dictionary_makes_from_a_conjugating_stem_is_offered() {
        let sources = folder("stem", "", &[("base.tsv", "か\t書\t五段-カ行\n")]);
        assert_eq!(offered(&sources, "かく", "書く"), at("base.tsv", 1));
    }

    #[test]
    fn a_word_with_okurigana_is_offered_when_its_stem_converts_with_the_okurigana() {
        let sources = folder("okuri", "", &[("base.tsv", "か*k\t書\n")]);
        assert_eq!(offered(&sources, "か*く", "書く"), at("base.tsv", 1));
        assert_eq!(
            offered(&sources, "か*け", "書け"),
            at("base.tsv", 1),
            "any okurigana of the row"
        );
        assert_eq!(
            offered(&sources, "か*さ", "書さ"),
            None,
            "not one of another row"
        );
    }

    #[test]
    fn a_word_with_okurigana_is_offered_by_a_word_of_the_whole_reading_too() {
        let sources = folder("whole", "", &[("base.tsv", "かく\t書く\n")]);
        assert_eq!(offered(&sources, "か*く", "書く"), at("base.tsv", 1));
    }

    #[test]
    fn a_pair_the_user_hid_is_not_offered_though_a_later_line_registers_it() {
        // Removing the word leaves the hide line, which then hides the pair
        // from every dictionary again.
        let sources = folder(
            "hidden",
            "!きしゃ\t汽車\nきしゃ\t汽車\n",
            &[("base.tsv", "きしゃ\t汽車\n")],
        );
        assert_eq!(offered(&sources, "きしゃ", "汽車"), None);
    }

    #[test]
    fn a_pair_hidden_before_an_okurigana_line_stays_hidden_without_it() {
        let sources = folder(
            "hidden-okuri",
            "!かった\t勝った\nか*っ\t勝っ\n",
            &[("base.tsv", "か\t勝\t五段-タ行\n")],
        );
        assert_eq!(offered(&sources, "か*っ", "勝っ"), at("base.tsv", 1));
        assert_eq!(
            offered(&sources, "かった", "勝った"),
            None,
            "the hide line comes back"
        );
    }

    #[test]
    fn the_user_custom_dictionary_offers_none_of_its_own_words() {
        let sources = folder("own", "きしゃ\t記者\nきしゃ\t記者\n", &[]);
        assert_eq!(offered(&sources, "きしゃ", "記者"), None);
    }

    #[test]
    fn a_word_with_placeholders_is_not_looked_at() {
        let sources = folder(
            "placeholders",
            "",
            &[("base.tsv", "{}こ\t{}個\nきょう\t{-:date %Y}\n")],
        );
        assert_eq!(offered(&sources, "{}こ", "{}個"), None);
        assert_eq!(offered(&sources, "きょう", "{-:date %Y}"), None);
    }

    #[test]
    fn every_listed_word_another_dictionary_offers_is_found_at_once() {
        let sources = folder(
            "all",
            "むへんかん\t無変換\nこうしきさいと\t公式サイト\nか*く\t書く\n",
            &[("base.tsv", "むへんかん\t無変換\nか\t書\t五段-カ行\n")],
        );
        let found = offers(&sources, file_name).unwrap();
        assert_eq!(
            found,
            HashMap::from([
                (word("むへんかん", "無変換"), at("base.tsv", 1).unwrap()),
                (word("か*く", "書く"), at("base.tsv", 1).unwrap()),
            ])
        );
    }
}
