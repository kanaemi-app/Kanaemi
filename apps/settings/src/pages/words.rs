//! The words registered in the user custom dictionary, to add to, edit and
//! remove where the dictionary is looked into. What may be written, and how,
//! is the engine's: this only shows it.

use std::collections::HashMap;

use kanaemi_engine::{UserWord, add_user_word, edit_user_word, remove_user_word, user_words};
use kanaemi_functions::builtin_sources;

use super::dictionaries::in_background;
use super::functions::function_files;
use super::*;
use crate::messages::describe_word;
use crate::offered::{Offer, Sources, offers};

/// How many words the list shows at most, so a long dictionary opens at
/// once; the filter finds the rest.
const WORDS_SHOWN: usize = 200;

/// What the form below a word, or below the list, is open for.
#[derive(Clone, PartialEq)]
enum Form {
    Adding,
    Editing(UserWord),
}

/// The user custom dictionary as its button to look into a dictionary opens
/// it: the words to edit in place of the entries the other dictionaries show.
/// `labels` names each dictionary of the list, by the name the settings file
/// gives it, as the list shows it.
#[component]
pub(super) fn UserWords(
    custom: PathBuf,
    functions: PathBuf,
    labels: Vec<(String, String)>,
    on_close: EventHandler<()>,
) -> Element {
    let mut ctx = use_context::<Ctx>();
    // The store is read again when the window comes back, as the IME may
    // have registered more meanwhile, and is written after each change, so
    // every part of the page that reads the file reads it again.
    let _ = ctx.store.read();
    let mut form = use_signal(|| None::<Form>);
    let mut filter = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);
    let mut only_offered = use_signal(|| false);
    let mut confirming = use_signal(|| false);
    // Checked off the window's thread, as it opens every dictionary and the
    // model; again whenever the store is, as the words or the list may have
    // changed.
    let checked = use_resource(move || {
        let store = ctx.store.read();
        let folder = store.dir.join(DICTIONARY_DIR);
        let loaded = store.state.as_ref().ok();
        let sources = Sources {
            dir: store.dir.clone(),
            listed: loaded.and_then(|l| l.settings.dictionaries.clone()),
            custom: store.files().custom,
            selections: store.files().selections,
            ranking_model: loaded.is_none_or(|l| l.settings.ranking_model),
        };
        drop(store);
        let labels = labels.clone();
        async move {
            in_background(move || {
                offers(&sources, |source| {
                    let name = source_name(&folder, source);
                    labels
                        .iter()
                        .find(|(listed, _)| *listed == name)
                        .map_or(name, |(_, label)| label.clone())
                })
                .map_err(|e| e.to_string())
            })
            .await
        }
    });
    let offered: HashMap<UserWord, Offer> = match &*checked.read() {
        Some(Ok(offered)) => offered.clone(),
        _ => HashMap::new(),
    };
    let checking = checked.read().is_none();
    let words = user_words(&custom).map_err(|e| e.to_string());
    let query: String = filter().trim().nfc().collect();
    let narrowing = (only_offered() && !offered.is_empty()).then_some(&offered);
    let (shown, more, all_matching) = match &words {
        Ok(words) => {
            let matching = matching(words, &query, narrowing);
            let more = matching.len().saturating_sub(WORDS_SHOWN);
            let shown: Vec<UserWord> = matching
                .iter()
                .take(WORDS_SHOWN)
                .copied()
                .cloned()
                .collect();
            let all: Vec<UserWord> = matching.into_iter().cloned().collect();
            (shown, more, all)
        }
        Err(_) => (Vec::new(), 0, Vec::new()),
    };
    let any = words.as_ref().is_ok_and(|w| !w.is_empty());
    let offered_count = words
        .as_ref()
        .map(|words| words.iter().filter(|w| offered.contains_key(w)).count())
        .unwrap_or(0);
    let total = words.as_ref().map(Vec::len).unwrap_or(0);
    let done = move |_| {
        form.set(None);
        ctx.store.write();
    };
    let remove_all = {
        let custom = custom.clone();
        let doomed = all_matching.clone();
        move |_| {
            let failed = doomed
                .iter()
                .filter_map(|word| remove_user_word(&custom, word).err())
                .count();
            if failed == 0 {
                error.set(None);
            } else {
                error.set(Some(format!("{failed} 語を消せませんでした")));
            }
            confirming.set(false);
            ctx.store.write();
        }
    };
    let narrowed = narrowing.is_some();
    rsx! {
        div { class: "modal-backdrop", onclick: move |_| on_close.call(()),
            div {
                class: "modal user-words",
                onclick: move |e| e.stop_propagation(),
                header {
                    div {
                        h2 { "ユーザー辞書" }
                        p { class: "item-description",
                            "登録した語です。語を足したり、読みと表記を直したりできます。送り仮名のある語は、読みの送り仮名の前に * を入れ（か*く）、表記は送り仮名まで書きます（書く）。数は読みに {{}} と書き、表記の {{kanji}} のような置き場所で書き方を決めます。"
                        }
                        p { class: "item-description",
                            if checking {
                                "ほかの辞書がもう出す語かを調べています…"
                            } else {
                                "「ほかの辞書」は、登録した語がなくても、その辞書がその読みで出す語と、候補の何番目に出るかです。消しても、その読みで変換すれば出ます。"
                            }
                        }
                    }
                    button { onclick: move |_| on_close.call(()), "閉じる" }
                }
                if offered_count > 0 {
                    div { class: "view-switch",
                        button {
                            class: if !narrowed { "selected" } else { "" },
                            onclick: move |_| {
                                only_offered.set(false);
                                confirming.set(false);
                            },
                            "すべて（{total}）"
                        }
                        button {
                            class: if narrowed { "selected" } else { "" },
                            onclick: move |_| {
                                only_offered.set(true);
                                confirming.set(false);
                            },
                            "ほかの辞書にもある語（{offered_count}）"
                        }
                    }
                }
                if any {
                    input {
                        class: "filter",
                        placeholder: "読みで絞り込む…",
                        // Not given the value it holds: written back while an
                        // IME is composing in it, it would end the composing.
                        oninput: move |e| {
                            filter.set(e.value());
                            confirming.set(false);
                        },
                    }
                }
                div { class: "modal-body",
                    match &words {
                        Err(e) => rsx! { p { class: "error", "ユーザー辞書を読めません：{e}" } },
                        Ok(_) if shown.is_empty() => rsx! {
                            p { class: "description",
                                if any {
                                    "合う語はありません"
                                } else {
                                    "語はありません"
                                }
                            }
                        },
                        Ok(_) => rsx! {
                            table { class: "rules words",
                                thead {
                                    tr {
                                        th { "読み" }
                                        th { "表記" }
                                        th { "ほかの辞書" }
                                        th {}
                                    }
                                }
                                tbody {
                                    for word in shown {
                                        if form() == Some(Form::Editing(word.clone())) {
                                            tr { key: "{word.reading()}\t{word.surface()}",
                                                td { colspan: 4,
                                                    WordForm {
                                                        original: Some(word.clone()),
                                                        custom: custom.clone(),
                                                        functions: functions.clone(),
                                                        on_done: done,
                                                        on_cancel: move |_| form.set(None),
                                                    }
                                                }
                                            }
                                        } else {
                                            tr { key: "{word.reading()}\t{word.surface()}",
                                                td { class: "word-text", "{word.reading()}" }
                                                td { class: "word-text", "{word.surface()}" }
                                                td { class: "item-description",
                                                    if let Some(offer) = offered.get(&word) {
                                                        {unbroken_at_hyphens(&offer.dictionary)}
                                                        span { class: "offer-place", "・{offer.place} 番目" }
                                                    }
                                                }
                                                td { class: "word-actions",
                                                    button {
                                                        onclick: {
                                                            let word = word.clone();
                                                            move |_| form.set(Some(Form::Editing(word.clone())))
                                                        },
                                                        "直す"
                                                    }
                                                    button {
                                                        onclick: {
                                                            let custom = custom.clone();
                                                            let word = word.clone();
                                                            move |_| {
                                                                match remove_user_word(&custom, &word) {
                                                                    Ok(()) => error.set(None),
                                                                    Err(e) => error.set(Some(format!("消せません：{e}"))),
                                                                }
                                                                ctx.store.write();
                                                            }
                                                        },
                                                        "消す"
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        },
                    }
                    if more > 0 {
                        p { class: "description",
                            "ほかに {more} 語あります。読みで絞り込んでください。"
                        }
                    }
                    if narrowed && !all_matching.is_empty() {
                        if confirming() {
                            p { class: "description",
                                "{all_matching.len()} 語をユーザー辞書から消します。出さないとした行は残します。"
                            }
                            div { class: "modal-footer",
                                button { onclick: move |_| confirming.set(false), "キャンセル" }
                                button { class: "primary", onclick: remove_all, "消す" }
                            }
                        } else {
                            div { class: "modal-footer",
                                button {
                                    onclick: move |_| confirming.set(true),
                                    "この {all_matching.len()} 語を消す…"
                                }
                            }
                        }
                    } else if form() == Some(Form::Adding) {
                        WordForm {
                            original: None,
                            custom: custom.clone(),
                            functions: functions.clone(),
                            on_done: done,
                            on_cancel: move |_| form.set(None),
                        }
                    } else {
                        button {
                            class: "add",
                            onclick: move |_| form.set(Some(Form::Adding)),
                            "語を足す"
                        }
                    }
                    if let Some(error) = error() {
                        p { class: "error", "{error}" }
                    }
                }
            }
        }
    }
}

/// The reading and the surface of a word to add, or of `original` to edit,
/// written only when the engine takes them.
#[component]
fn WordForm(
    original: Option<UserWord>,
    custom: PathBuf,
    functions: PathBuf,
    on_done: EventHandler<()>,
    on_cancel: EventHandler<()>,
) -> Element {
    // The fields start from the word and are never given a value again:
    // written back while an IME is composing in them, it would end the
    // composing.
    let initial = use_hook(|| {
        original
            .as_ref()
            .map(|word| (word.reading(), word.surface()))
            .unwrap_or_default()
    });
    let mut reading = use_signal(|| initial.0.clone());
    let mut surface = use_signal(|| initial.1.clone());
    let mut error = use_signal(|| None::<String>);
    let save = {
        let original = original.clone();
        move |_| {
            let written = UserWord::new(&reading(), &surface(), |name| {
                has_function(&functions, name)
            })
            .and_then(|word| match &original {
                Some(old) => edit_user_word(&custom, old, &word),
                None => add_user_word(&custom, &word),
            });
            match written {
                Ok(()) => on_done.call(()),
                Err(e) => error.set(Some(describe_word(&e))),
            }
        }
    };
    let blank = reading().trim().is_empty() && surface().trim().is_empty();
    rsx! {
        div {
            class: "binding-form word-form",
            onkeydown: move |e: KeyboardEvent| {
                if e.key() == Key::Escape {
                    on_cancel.call(());
                }
            },
            div { class: "binding-form-row",
                label { class: "word-field",
                    span { "読み" }
                    input {
                        placeholder: "か*く",
                        initial_value: "{initial.0}",
                        oninput: move |e| reading.set(e.value()),
                    }
                }
                label { class: "word-field",
                    span { "表記" }
                    input {
                        placeholder: "書く",
                        initial_value: "{initial.1}",
                        oninput: move |e| surface.set(e.value()),
                    }
                }
            }
            if let Some(message) = error() {
                p { class: "error", "{message}" }
            }
            div { class: "binding-form-buttons",
                button { class: "primary", disabled: blank, onclick: save,
                    if original.is_some() {
                        "保存"
                    } else {
                        "足す"
                    }
                }
                button { onclick: move |_| on_cancel.call(()), "キャンセル" }
            }
        }
    }
}

/// Whether `word` is found by `query` (trimmed and NFC): its reading starts
/// with it, as the other dictionaries are looked into, which have no index of
/// their surfaces.
fn found_by(word: &UserWord, query: &str) -> bool {
    word.reading().starts_with(query)
}

/// The words `query` finds, and, given `offered`, only those it holds.
fn matching<'a>(
    words: &'a [UserWord],
    query: &str,
    offered: Option<&HashMap<UserWord, Offer>>,
) -> Vec<&'a UserWord> {
    words
        .iter()
        .filter(|w| found_by(w, query) && offered.is_none_or(|o| o.contains_key(*w)))
        .collect()
}

/// `name` with its hyphens made non-breaking: a source name such as
/// `kanaemi-dict` split across lines reads as two names, while the rest of a
/// dictionary's name may still wrap in the narrow column it is shown in.
fn unbroken_at_hyphens(name: &str) -> String {
    name.replace('-', "\u{2011}")
}

/// Whether a placeholder may name `name`: a built-in function, or one of the
/// functions folder, on or off. Whether it runs, the functions page tells.
fn has_function(folder: &Path, name: &str) -> bool {
    builtin_sources().any(|(builtin, _)| builtin == name)
        || function_files(folder).iter().any(|(file, _)| file == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_are_narrowed_to_the_readings_the_query_starts_as_other_dictionaries_are() {
        let word = |reading, surface| UserWord::new(reading, surface, |_| true).unwrap();
        assert!(found_by(&word("かんじ", "漢字"), "かん"));
        assert!(
            !found_by(&word("かんじ", "漢字"), "んじ"),
            "not anywhere in the reading"
        );
        assert!(
            !found_by(&word("かんじ", "漢字"), "漢"),
            "not by the surface"
        );
        assert!(found_by(&word("か*く", "書く"), "か*く"));
        assert!(found_by(&word("かんじ", "漢字"), ""));
    }

    #[test]
    fn narrowed_to_offered_words_only_those_another_dictionary_offers_are_left() {
        let word = |reading, surface| UserWord::new(reading, surface, |_| true).unwrap();
        let words = [
            word("かんじ", "漢字"),
            word("かんじ", "感じ"),
            word("きしゃ", "記者"),
        ];
        let offer = Offer {
            dictionary: "基本辞書".to_owned(),
            place: 1,
        };
        let offered = HashMap::from([
            (word("かんじ", "漢字"), offer.clone()),
            (word("きしゃ", "記者"), offer),
        ]);
        assert_eq!(matching(&words, "", Some(&offered)), [&words[0], &words[2]]);
        assert_eq!(
            matching(&words, "かん", Some(&offered)),
            [&words[0]],
            "and to the query"
        );
        assert_eq!(
            matching(&words, "かん", None),
            [&words[0], &words[1]],
            "every word the query finds, not narrowed"
        );
    }

    #[test]
    fn a_dictionary_name_keeps_its_hyphenated_words_whole() {
        assert_eq!(
            unbroken_at_hyphens("基本（kanaemi-dict）"),
            "基本（kanaemi\u{2011}dict）"
        );
        assert_eq!(unbroken_at_hyphens("基本辞書"), "基本辞書");
    }

    #[test]
    fn a_function_is_built_in_or_in_the_functions_folder() {
        let dir =
            std::env::temp_dir().join(format!("kanaemi-settings-words-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("mine.luau"), "").unwrap();
        let builtin = builtin_sources().next().unwrap().0;

        assert!(has_function(&dir, builtin));
        assert!(has_function(&dir, "mine"));
        assert!(!has_function(&dir, "笑"));
        let _ = fs::remove_dir_all(&dir);
    }
}
