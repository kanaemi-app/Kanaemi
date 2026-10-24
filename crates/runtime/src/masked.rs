//! What the log is told of an event and what the core made of it, with the
//! text the user typed masked: each character is left as only its kind, so
//! a log sent with a report shows what happened without what was written.

use std::fmt;

use kanaemi_core::{Chord, Effect, Event, Key, KeyEvent, KeyKind, Mode, Modifiers, Output};

/// `text` with each letter, kana, kanji and digit replaced by one of its
/// kind; marks, symbols and spaces stay as they are.
pub fn masked(text: &str) -> String {
    text.chars().map(kind_of).collect()
}

/// A message the program wrote in English, masked from its first quote on
/// and, before it, where it is not ASCII: a message on a string, such as a
/// panic on one sliced in the middle of a character, quotes it, and what
/// the user typed may be ASCII. Where a quote ends cannot be told, as the
/// text quoted may hold quotes typed.
pub fn masked_message(text: &str) -> String {
    let quoted = text.find(['`', '\'', '"']).unwrap_or(text.len());
    let (head, tail) = text.split_at(quoted);
    head.chars()
        .map(|c| if c.is_ascii() { c } else { kind_of(c) })
        .chain(tail.chars().map(kind_of))
        .collect()
}

/// Whether `text` has anything [`masked`] hides, though the kind it leaves
/// may be the very character, as `a` is: a key code beside it would tell
/// the character.
pub fn is_masked(text: &str) -> bool {
    text.chars().any(|c| kind(c).is_some())
}

fn kind_of(c: char) -> char {
    kind(c).unwrap_or(c)
}

/// The character `c` is masked as, `None` for one left as it is.
fn kind(c: char) -> Option<char> {
    let kind = match c {
        'ぁ'..='ゖ' | 'ゝ' | 'ゞ' => 'あ',
        'ァ'..='ヿ' | 'ｦ'..='ﾟ' => 'ア',
        '々'
        | '〆'
        | '\u{3400}'..='\u{4DBF}'
        | '\u{4E00}'..='\u{9FFF}'
        | '\u{F900}'..='\u{FAFF}'
        | '\u{20000}'..='\u{3134F}' => '漢',
        'a'..='z' | 'ａ'..='ｚ' => 'a',
        'A'..='Z' | 'Ａ'..='Ｚ' => 'A',
        '0'..='9' | '０'..='９' => '0',
        c if c.is_alphanumeric() => 'x',
        _ => return None,
    };
    Some(kind)
}

/// An event as the log tells it.
pub struct LoggedEvent<'a>(pub &'a Event);

/// What the core made of an event, as the log tells it; what is left as it
/// was before (nothing committed, nothing marked) is left out.
pub struct LoggedOutput<'a>(pub &'a Output);

/// A key with its modifiers as the log tells it.
pub struct LoggedChord(pub Chord);

impl fmt::Display for LoggedEvent<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self.0 {
            Event::Key(KeyEvent {
                key,
                mods,
                kind,
                time_ms,
            }) => {
                let name = match kind {
                    KeyKind::Press => "press",
                    KeyKind::Release => "release",
                    KeyKind::Repeat => "repeat",
                };
                write!(f, "{name}:")?;
                // A letter let go or repeating with Ctrl held may have been
                // pressed before Ctrl went down.
                let shortcut = kind == KeyKind::Press;
                write_chord(f, Chord { key, mods }, shortcut)?;
                write!(f, "@{time_ms}")
            }
            Event::FocusIn { password: false } => f.write_str("focus-in"),
            Event::FocusIn { password: true } => f.write_str("focus-in:password"),
            Event::FocusOut => f.write_str("focus-out"),
            Event::Flush => f.write_str("flush"),
            Event::Select(index) => write!(f, "select:{index}"),
            Event::SetMode(mode) => write!(f, "set-mode:{}", mode_name(mode)),
            Event::Erased(gone) => write!(f, "erased:{gone}"),
            Event::CaretMoved => f.write_str("caret-moved"),
        }
    }
}

impl fmt::Display for LoggedOutput<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let output = self.0;
        let answer = if output.consumed {
            "consumed"
        } else {
            "passed"
        };
        write!(f, "{{{answer} mode={}", mode_name(output.mode))?;
        if let Some(mode) = output.indicator {
            write!(f, " indicator={}", mode_name(mode))?;
        }
        if let Some(text) = &output.erase {
            write!(f, " erase={:?}", masked(text))?;
        }
        if let Some(text) = &output.commit {
            write!(f, " commit={:?}", masked(text))?;
        }
        if !output.preedit.is_empty() {
            write!(
                f,
                " preedit={:?} cursor={}",
                masked(&output.preedit),
                output.cursor
            )?;
        }
        if let Some(view) = &output.candidates {
            write!(
                f,
                " candidates={}/{},page={}/{}",
                view.selected + 1,
                view.items.len(),
                view.page + 1,
                view.pages
            )?;
            if let Some(model) = view.ranked_by_model {
                write!(f, ",model={}", if model { "on" } else { "off" })?;
            }
        }
        if let Some(chord) = output.send {
            write!(f, " send={}", LoggedChord(chord))?;
        }
        if output.paste {
            f.write_str(" paste")?;
        }
        if output.registering.is_some() {
            f.write_str(" registering")?;
        }
        for (i, effect) in output.effects.iter().enumerate() {
            f.write_str(if i == 0 { " effects=" } else { "," })?;
            f.write_str(effect_name(effect))?;
        }
        f.write_str("}")
    }
}

impl fmt::Display for LoggedChord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_chord(f, self.0, true)
    }
}

/// Writes `chord`, a letter held with Ctrl or Cmd left as it is where it
/// can be a `shortcut`, not text.
fn write_chord(f: &mut fmt::Formatter<'_>, chord: Chord, shortcut: bool) -> fmt::Result {
    let Chord { key, mods } = chord;
    let Modifiers {
        shift,
        ctrl,
        cmd,
        alt,
    } = mods;
    for (held, name) in [(shift, "shift"), (ctrl, "ctrl"), (alt, "alt"), (cmd, "cmd")] {
        if held {
            write!(f, "{name}+")?;
        }
    }
    match key {
        // Ctrl with Alt is AltGr on Windows, which types text.
        Key::Char(c) if shortcut && (ctrl || cmd) && !(ctrl && alt) => write!(f, "{c}"),
        Key::Char(c) => write!(f, "{}", kind_of(c)),
        Key::F(n) => write!(f, "f{n}"),
        key => f.write_str(key_name(key)),
    }
}

fn key_name(key: Key) -> &'static str {
    match key {
        Key::Space => "space",
        Key::Tab => "tab",
        Key::Enter => "enter",
        Key::Esc => "esc",
        Key::Backspace => "backspace",
        Key::Delete => "delete",
        Key::ShiftLeft => "shift-left",
        Key::ShiftRight => "shift-right",
        Key::CtrlLeft => "ctrl-left",
        Key::CtrlRight => "ctrl-right",
        Key::CmdLeft => "cmd-left",
        Key::CmdRight => "cmd-right",
        Key::AltLeft => "alt-left",
        Key::AltRight => "alt-right",
        Key::Eisu => "eisu",
        Key::Kana => "kana",
        Key::Henkan => "henkan",
        Key::Muhenkan => "muhenkan",
        Key::ZenkakuHankaku => "zenkaku-hankaku",
        Key::CapsLock => "caps-lock",
        Key::Left => "left",
        Key::Right => "right",
        Key::Up => "up",
        Key::Down => "down",
        Key::Home => "home",
        Key::End => "end",
        Key::PageUp => "page-up",
        Key::PageDown => "page-down",
        Key::Modifier => "modifier",
        Key::Other => "other",
        // Told by the chord, which masks a character.
        Key::Char(_) | Key::F(_) => "char",
    }
}

fn mode_name(mode: Mode) -> &'static str {
    match mode {
        Mode::Abc => "abc",
        Mode::Kana => "kana",
    }
}

fn effect_name(effect: &Effect) -> &'static str {
    match effect {
        Effect::Committed { .. } => "committed",
        Effect::Registered { .. } => "registered",
        Effect::Forgotten { .. } => "forgotten",
        Effect::Withdrawn { .. } => "withdrawn",
        Effect::Typed(_) => "typed",
        Effect::Erased(_) => "erased",
        Effect::FocusMoved => "focus-moved",
    }
}

#[cfg(test)]
mod tests {
    use kanaemi_core::{Candidate, CandidateView, Registering};

    use super::*;

    fn key(key: Key, mods: Modifiers, kind: KeyKind) -> String {
        LoggedEvent(&Event::Key(KeyEvent {
            key,
            mods,
            kind,
            time_ms: 42,
        }))
        .to_string()
    }

    fn plain() -> Modifiers {
        Modifiers::default()
    }

    fn output() -> Output {
        Output {
            consumed: false,
            erase: None,
            commit: None,
            preedit: String::new(),
            cursor: 0,
            candidates: None,
            mode: Mode::Kana,
            indicator: None,
            send: None,
            paste: false,
            registering: None,
            effects: Vec::new(),
        }
    }

    #[test]
    fn each_kind_of_character_is_left_as_one_of_its_kind() {
        assert_eq!(masked("ていあん"), "ああああ");
        assert_eq!(masked("バランスｶﾅ"), "アアアアアア");
        assert_eq!(masked("提案々"), "漢漢漢");
        assert_eq!(masked("abcXYZｑＱ"), "aaaAAAaA");
        assert_eq!(masked("2026０"), "00000");
        assert_eq!(masked("é"), "x");
    }

    #[test]
    fn text_is_told_to_be_masked_even_where_its_kind_looks_the_same() {
        assert!(is_masked("a"));
        assert!(is_masked("あ"));
        assert!(is_masked("x;"));
        assert!(!is_masked(";"));
        assert!(!is_masked("\r"));
        assert!(!is_masked(""));
    }

    #[test]
    fn marks_symbols_and_spaces_stay() {
        assert_eq!(masked("›けってい*\u{200B}"), "›ああああ*\u{200B}");
        assert_eq!(masked("»決定 « |;、。「」😀"), "»漢漢 « |;、。「」😀");
    }

    #[test]
    fn a_message_keeps_its_ascii_and_masks_the_rest_and_what_it_quotes() {
        assert_eq!(
            masked_message(
                "byte index 4 is not a char boundary; it is inside 'て' of `›secret9て`"
            ),
            "byte index 4 is not a char boundary; it is inside 'あ' aa `›aaaaaa0あ`",
        );
        assert_eq!(
            masked_message("left: \"ab\" right: \"かな\" in 日本"),
            "left: \"aa\" aaaaa: \"ああ\" aa 漢漢",
        );
        assert_eq!(
            masked_message("of `あ`secret9`"),
            "of `あ`aaaaaa0`",
            "a quote typed ends nothing"
        );
        assert_eq!(masked_message("not 日本"), "not 漢漢");
    }

    #[test]
    fn a_letter_typed_with_altgr_or_held_into_a_repeat_is_masked() {
        let altgr = Modifiers {
            ctrl: true,
            alt: true,
            ..plain()
        };
        assert_eq!(
            key(Key::Char('ł'), altgr, KeyKind::Press),
            "press:ctrl+alt+x@42"
        );
        let ctrl = Modifiers {
            ctrl: true,
            ..plain()
        };
        assert_eq!(
            key(Key::Char('k'), ctrl, KeyKind::Repeat),
            "repeat:ctrl+a@42"
        );
    }

    #[test]
    fn a_key_typed_as_text_is_masked_and_a_named_key_is_not() {
        assert_eq!(key(Key::Char('n'), plain(), KeyKind::Press), "press:a@42");
        let shift = Modifiers {
            shift: true,
            ..plain()
        };
        assert_eq!(
            key(Key::Char('D'), shift, KeyKind::Press),
            "press:shift+A@42"
        );
        assert_eq!(
            key(Key::Enter, plain(), KeyKind::Release),
            "release:enter@42"
        );
        assert_eq!(
            key(Key::Backspace, plain(), KeyKind::Repeat),
            "repeat:backspace@42"
        );
        assert_eq!(key(Key::F(7), plain(), KeyKind::Press), "press:f7@42");
    }

    #[test]
    fn a_letter_held_with_ctrl_or_cmd_is_a_shortcut_and_is_kept() {
        let ctrl = Modifiers {
            ctrl: true,
            ..plain()
        };
        assert_eq!(key(Key::Char('m'), ctrl, KeyKind::Press), "press:ctrl+m@42");
        let cmd = Modifiers {
            cmd: true,
            shift: true,
            ..plain()
        };
        assert_eq!(
            key(Key::Char('V'), cmd, KeyKind::Press),
            "press:shift+cmd+V@42"
        );
    }

    #[test]
    fn a_letter_let_go_is_masked_whatever_is_held_as_it_goes() {
        let ctrl = Modifiers {
            ctrl: true,
            ..plain()
        };
        assert_eq!(
            key(Key::Char('k'), ctrl, KeyKind::Release),
            "release:ctrl+a@42"
        );
    }

    #[test]
    fn events_that_are_no_key_are_told_by_name() {
        let told = |event| LoggedEvent(&event).to_string();
        assert_eq!(told(Event::FocusIn { password: false }), "focus-in");
        assert_eq!(told(Event::FocusIn { password: true }), "focus-in:password");
        assert_eq!(told(Event::FocusOut), "focus-out");
        assert_eq!(told(Event::Flush), "flush");
        assert_eq!(told(Event::Select(2)), "select:2");
        assert_eq!(told(Event::SetMode(Mode::Abc)), "set-mode:abc");
        assert_eq!(told(Event::Erased(false)), "erased:false");
        assert_eq!(told(Event::CaretMoved), "caret-moved");
    }

    #[test]
    fn an_output_that_changes_nothing_tells_only_the_mode() {
        assert_eq!(LoggedOutput(&output()).to_string(), "{passed mode=kana}");
    }

    #[test]
    fn the_text_of_an_output_is_masked() {
        let told = LoggedOutput(&Output {
            consumed: true,
            erase: Some("かな".to_owned()),
            commit: Some("提案".to_owned()),
            preedit: "›s".to_owned(),
            cursor: 2,
            ..output()
        })
        .to_string();
        assert_eq!(
            told,
            "{consumed mode=kana erase=\"ああ\" commit=\"漢漢\" preedit=\"›a\" cursor=2}"
        );
    }

    #[test]
    fn candidates_are_told_by_count_and_place_not_by_text() {
        let candidate = |surface: &str| Candidate {
            surface: surface.to_owned(),
            source: Some("辞書".to_owned()),
            preview: None,
        };
        let told = LoggedOutput(&Output {
            consumed: true,
            preedit: "»提案".to_owned(),
            cursor: 3,
            candidates: Some(CandidateView {
                items: vec![candidate("提案"), candidate("ていあん")],
                selected: 1,
                page: 0,
                pages: 3,
                more: vec!["X".to_owned()],
                ranked_by_model: Some(false),
                toggle_model: None,
            }),
            ..output()
        })
        .to_string();
        assert_eq!(
            told,
            "{consumed mode=kana preedit=\"»漢漢\" cursor=3 candidates=2/2,page=1/3,model=off}"
        );
    }

    #[test]
    fn what_the_host_is_asked_to_do_is_told() {
        let enter = Chord {
            key: Key::Enter,
            mods: plain(),
        };
        let told = LoggedOutput(&Output {
            mode: Mode::Abc,
            indicator: Some(Mode::Abc),
            send: Some(enter),
            paste: true,
            registering: Some(Registering {
                cancel: None,
                paste: None,
            }),
            effects: vec![
                Effect::Typed("か".to_owned()),
                Effect::Committed {
                    reading: "か".to_owned(),
                    okurigana: None,
                    surface: "火".to_owned(),
                },
                Effect::FocusMoved,
            ],
            ..output()
        })
        .to_string();
        assert_eq!(
            told,
            "{passed mode=abc indicator=abc send=enter paste registering effects=typed,committed,focus-moved}"
        );
    }
}
