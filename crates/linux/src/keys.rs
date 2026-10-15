//! Key events as IBus and Fcitx5 both give them, X keysyms with a modifier
//! state, turned into the core's keys, and back for the keys the core sends
//! to the application.

use std::fmt;

use kanaemi_core::{Chord, Key, KeyEvent, KeyKind, Modifiers};
use kanaemi_runtime::{is_masked, masked};

const SHIFT_MASK: u32 = 1 << 0;
const CONTROL_MASK: u32 = 1 << 2;
const MOD1_MASK: u32 = 1 << 3;
/// Super as Qt sends it; GTK sets this and [`SUPER_MASK`] both.
const MOD4_MASK: u32 = 1 << 6;
const SUPER_MASK: u32 = 1 << 26;
pub const RELEASE_MASK: u32 = 1 << 30;

const XK_CAPS_LOCK: u32 = 0xffe5;

/// The modifier keys the core tells apart, by keysym.
const SIDED: [(u32, Key); 8] = [
    (0xffe1, Key::ShiftLeft),
    (0xffe2, Key::ShiftRight),
    (0xffe3, Key::CtrlLeft),
    (0xffe4, Key::CtrlRight),
    (0xffe9, Key::AltLeft),
    (0xffea, Key::AltRight),
    (0xffeb, Key::CmdLeft),
    (0xffec, Key::CmdRight),
];

/// The keys the core knows by name rather than by what they type.
const NAMED: [(u32, Key); 21] = [
    (0xff08, Key::Backspace),
    (0xff09, Key::Tab),
    (0xff0d, Key::Enter),
    (0xff1b, Key::Esc),
    (0x0020, Key::Space),
    (0xffff, Key::Delete),
    (0xff51, Key::Left),
    (0xff52, Key::Up),
    (0xff53, Key::Right),
    (0xff54, Key::Down),
    (0xff50, Key::Home),
    (0xff57, Key::End),
    (0xff55, Key::PageUp),
    (0xff56, Key::PageDown),
    (0xff23, Key::Henkan),
    (0xff22, Key::Muhenkan),
    (0xff30, Key::Eisu),
    (0xff27, Key::Kana),
    (0xff2a, Key::ZenkakuHankaku),
    // Zenkaku and Hankaku, which a keymap may give the key by turns.
    (0xff28, Key::ZenkakuHankaku),
    (0xff29, Key::ZenkakuHankaku),
];

/// The keypad's keys that the main keys have too, and Tab as X names it
/// with Shift (ISO_Left_Tab). The keys the core sends are always the main
/// ones.
const KEYPAD: [(u32, Key); 13] = [
    (0xff80, Key::Space),
    (0xff89, Key::Tab),
    (0xfe20, Key::Tab),
    (0xff8d, Key::Enter),
    (0xff95, Key::Home),
    (0xff96, Key::Left),
    (0xff97, Key::Up),
    (0xff98, Key::Right),
    (0xff99, Key::Down),
    (0xff9c, Key::End),
    (0xff9a, Key::PageUp),
    (0xff9b, Key::PageDown),
    (0xff9f, Key::Delete),
];

const XK_F1: u32 = 0xffbe;
const XK_F12: u32 = 0xffc9;

/// Remembers the keys down that are not modifiers, because the modifiers
/// held as a key is let go may make it read as another character.
#[derive(Debug, Default)]
pub struct Keys {
    /// Each key down, by hardware key code, as its press read; the last
    /// pressed last.
    typed: Vec<(u32, Key)>,
}

impl Keys {
    /// Whether a press of `keycode` is of a key already down: X repeats a
    /// key held as more presses.
    fn repeating(&self, keycode: u32, state: u32) -> bool {
        state & RELEASE_MASK == 0
            && keycode != 0
            && self.typed.iter().any(|(code, _)| *code == keycode)
    }

    /// Whether `keycode` is down as a key that types text, as its press read.
    fn typed_text(&self, keycode: u32) -> bool {
        keycode != 0
            && self
                .typed
                .iter()
                .any(|&(code, key)| code == keycode && matches!(key, Key::Char(_)))
    }

    /// The core's key for a key event, its release marked in `state` by
    /// [`RELEASE_MASK`] as IBus marks it; `None` when it is not a press
    /// or release the core needs. X repeats no modifier, so a modifier's
    /// press is always a new one. A key code of 0 says nothing of which key
    /// it is, and such a key reads as its keysym says.
    pub fn translate(
        &mut self,
        keyval: u32,
        keycode: u32,
        state: u32,
        time_ms: u64,
    ) -> Option<KeyEvent> {
        let logged = LoggedKeysym {
            keyval,
            keycode,
            state,
            repeat: self.repeating(keycode, state),
            typed_text: self.typed_text(keycode),
        };
        tracing::info!(key = %logged, time_ms, "key");
        let mods = Modifiers {
            shift: state & SHIFT_MASK != 0,
            ctrl: state & CONTROL_MASK != 0,
            alt: state & MOD1_MASK != 0,
            cmd: state & (SUPER_MASK | MOD4_MASK) != 0,
        };
        let release = state & RELEASE_MASK != 0;
        let event = |key, mods, kind| KeyEvent {
            key,
            mods,
            kind,
            time_ms,
        };
        if let Some(&(_, key)) = SIDED.iter().find(|(sym, _)| *sym == keyval) {
            let kind = if release {
                KeyKind::Release
            } else {
                KeyKind::Press
            };
            return Some(event(key, mods, kind));
        }
        if keyval == XK_CAPS_LOCK {
            // Each press is one, as on platforms that tell only of its lock
            // turning; no binding waits on its release.
            return (!release).then(|| event(Key::CapsLock, mods, KeyKind::Press));
        }
        let down = (keycode != 0)
            .then(|| self.typed.iter().position(|(code, _)| *code == keycode))
            .flatten();
        if release {
            let key = match down {
                Some(index) => self.typed.remove(index).1,
                None => key(keyval),
            };
            return Some(event(key, mods, KeyKind::Release));
        }
        // X gives no sign of a repeat but repeats only the key pressed last,
        // so a press of that key still down is one: it stays the key first
        // pressed, whatever modifier went down since. Any other press, of a
        // key whose release went elsewhere among them, is read afresh.
        let (key, kind) = match down {
            Some(index) if index + 1 == self.typed.len() => (self.typed[index].1, KeyKind::Repeat),
            _ => {
                let key = key(keyval);
                if keycode != 0 {
                    self.typed.retain(|(code, _)| *code != keycode);
                    self.typed.push((keycode, key));
                }
                (key, KeyKind::Press)
            }
        };
        // A symbol typed with Shift is that symbol (`:`), as a binding writes
        // it; only a letter keeps its Shift (`shift+a`).
        let mods = match key {
            Key::Char(c) if !c.is_ascii_alphabetic() => Modifiers {
                shift: false,
                ..mods
            },
            _ => mods,
        };
        Some(event(key, mods, kind))
    }
}

fn key(keyval: u32) -> Key {
    named(keyval)
        .or_else(|| character(keyval).map(Key::Char))
        .unwrap_or(Key::Other)
}

fn named(keyval: u32) -> Option<Key> {
    if (XK_F1..=XK_F12).contains(&keyval) {
        return Some(Key::F((keyval - XK_F1 + 1) as u8));
    }
    NAMED
        .iter()
        .chain(&KEYPAD)
        .find(|(sym, _)| *sym == keyval)
        .map(|(_, key)| *key)
}

/// What a keysym types: Latin-1 keysyms are their code points, the keypad's
/// digits and signs are theirs with `0xff80` added, and Unicode keysyms
/// carry theirs with `0x0100_0000` added.
fn character(keyval: u32) -> Option<char> {
    let code = match keyval {
        0x20..=0x7e | 0xa0..=0xff => keyval,
        0xffaa..=0xffb9 | 0xffbd => keyval - 0xff80,
        0x0100_0000..=0x0110_ffff => keyval - 0x0100_0000,
        _ => return None,
    };
    char::from_u32(code).filter(|c| !c.is_control())
}

/// A key event as the framework gave it, as the log tells it. A key typed as
/// text is told only by the kind of its character: its codes would tell
/// the character.
pub struct LoggedKeysym {
    pub keyval: u32,
    pub keycode: u32,
    pub state: u32,
    /// A press of a key already down.
    pub repeat: bool,
    /// Whether the key was pressed as text, whatever it reads as now.
    pub typed_text: bool,
}

impl fmt::Display for LoggedKeysym {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let release = self.state & RELEASE_MASK != 0;
        f.write_str(if release { "release" } else { "press" })?;
        let pressed = !release && !self.repeat;
        let shortcut = pressed && self.state & (CONTROL_MASK | SUPER_MASK | MOD4_MASK) != 0;
        let shown = match character(self.keyval) {
            // Let go or repeating, the modifiers may have changed since the
            // press, and with them the keysym, which would tell the one the
            // press masked.
            Some(_) if !pressed => {
                f.write_str(" hidden")?;
                None
            }
            // Pressed as text, the key may go as a named one, as KP_1 goes
            // as KP_End once Shift is down.
            None if !pressed && self.typed_text => {
                f.write_str(" hidden")?;
                None
            }
            Some(c) => {
                let text = String::from(c);
                if shortcut || !is_masked(&text) {
                    write!(f, " keyval={:#x} keycode={}", self.keyval, self.keycode)?;
                }
                Some(if shortcut { text } else { masked(&text) })
            }
            // Keysyms of function keys and of vendor keys name no text;
            // the others, such as those of kana and Cyrillic layouts, may
            // type it in a way not read here.
            None if matches!(self.keyval, 0xfd00..=0xffff | 0x1008_0000..=0x1008_ffff) => {
                write!(f, " keyval={:#x} keycode={}", self.keyval, self.keycode)?;
                None
            }
            None => {
                f.write_str(" hidden")?;
                None
            }
        };
        if let Some(shown) = shown {
            write!(f, " char={shown}")?;
        }
        write!(f, " state={:#x}", self.state)?;
        if self.repeat {
            f.write_str(" repeat")?;
        }
        Ok(())
    }
}

/// The keysym and modifier state to forward for `chord`, for the keys the
/// core can send.
pub fn key_to_send(chord: Chord) -> Option<(u32, u32)> {
    let (keyval, _) = NAMED.iter().find(|(_, key)| *key == chord.key)?;
    let mut state = 0;
    for (on, mask) in [
        (chord.mods.shift, SHIFT_MASK),
        (chord.mods.ctrl, CONTROL_MASK),
        (chord.mods.alt, MOD1_MASK),
        // Qt, XIM and Wayland read only Mod4.
        (chord.mods.cmd, SUPER_MASK | MOD4_MASK),
    ] {
        if on {
            state |= mask;
        }
    }
    Some((*keyval, state))
}

/// The keysym and modifier state of each Backspace that erases `text` just
/// before the caret.
pub fn erasing(text: &str) -> impl Iterator<Item = (u32, u32)> {
    let backspace = Chord {
        key: Key::Backspace,
        mods: Modifiers::default(),
    };
    let presses = kanaemi_runtime::backspaces(text);
    key_to_send(backspace)
        .into_iter()
        .flat_map(move |key| std::iter::repeat_n(key, presses))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One event read on its own, with no key down before it.
    fn translate(keyval: u32, state: u32, time_ms: u64) -> Option<KeyEvent> {
        Keys::default().translate(keyval, 38, state, time_ms)
    }

    fn logged(keyval: u32, keycode: u32, state: u32) -> String {
        LoggedKeysym {
            keyval,
            keycode,
            state,
            repeat: false,
            typed_text: false,
        }
        .to_string()
    }

    #[test]
    fn a_key_typing_text_repeating_with_ctrl_held_since_is_hidden() {
        let repeat = LoggedKeysym {
            keyval: u32::from('k'),
            keycode: 45,
            state: CONTROL_MASK,
            repeat: true,
            typed_text: true,
        };
        assert_eq!(repeat.to_string(), "press hidden state=0x4 repeat");
    }

    #[test]
    fn a_key_pressed_as_text_is_hidden_as_it_goes_though_it_now_reads_as_named() {
        // KP_1 pressed with Num Lock on, then Shift, then let go as KP_End.
        let end = LoggedKeysym {
            keyval: 0xff9c,
            keycode: 87,
            state: SHIFT_MASK | RELEASE_MASK,
            repeat: false,
            typed_text: true,
        };
        assert_eq!(end.to_string(), "release hidden state=0x40000001");
    }

    #[test]
    fn a_key_down_as_text_is_known_so() {
        let mut keys = Keys::default();
        keys.translate(0xffb1, 87, 0, 0);
        keys.translate(0xff0d, 36, 0, 0);
        assert!(keys.typed_text(87));
        assert!(!keys.typed_text(36));
        assert!(!keys.typed_text(0));
    }

    #[test]
    fn a_key_pressed_again_while_down_is_a_repeat() {
        let mut keys = Keys::default();
        keys.translate(u32::from('k'), 45, 0, 0);
        assert!(keys.repeating(45, 0));
        assert!(!keys.repeating(46, 0));
        assert!(!keys.repeating(45, RELEASE_MASK));
        assert!(!keys.repeating(0, 0), "a key code of 0 tells no key");
    }

    #[test]
    fn a_key_typed_as_text_is_logged_without_its_codes_or_character() {
        assert_eq!(logged(u32::from('k'), 45, 0), "press char=a state=0x0");
        assert_eq!(
            logged(0x0100_3042, 0, RELEASE_MASK),
            "release hidden state=0x40000000"
        );
        assert_eq!(
            logged(u32::from(';'), 47, 0),
            "press keyval=0x3b keycode=47 char=; state=0x0"
        );
    }

    #[test]
    fn a_letter_let_go_with_ctrl_held_is_still_masked() {
        assert_eq!(
            logged(u32::from('k'), 45, CONTROL_MASK | RELEASE_MASK),
            "release hidden state=0x40000004"
        );
        // 1 pressed, then Shift, then 1 let go.
        assert_eq!(
            logged(u32::from('!'), 10, SHIFT_MASK | RELEASE_MASK),
            "release hidden state=0x40000001"
        );
    }

    #[test]
    fn a_keysym_not_known_as_a_named_key_is_hidden() {
        // kana_A, from a kana layout.
        assert_eq!(logged(0x4b1, 30, 0), "press hidden state=0x0");
        assert_eq!(
            logged(0x1008_ff13, 123, 0),
            "press keyval=0x1008ff13 keycode=123 state=0x0"
        );
    }

    #[test]
    fn a_named_key_or_a_shortcut_is_logged_with_its_codes() {
        assert_eq!(
            logged(0xff0d, 36, 0),
            "press keyval=0xff0d keycode=36 state=0x0"
        );
        assert_eq!(
            logged(u32::from('m'), 58, CONTROL_MASK),
            "press keyval=0x6d keycode=58 char=m state=0x4"
        );
    }

    #[test]
    fn the_two_shift_keys_are_their_own_keysyms() {
        assert_eq!(translate(0xffe2, 0, 0).unwrap().key, Key::ShiftRight);
        let release = translate(0xffe1, SHIFT_MASK | RELEASE_MASK, 0).unwrap();
        assert_eq!(
            (release.key, release.kind),
            (Key::ShiftLeft, KeyKind::Release)
        );
    }

    #[test]
    fn a_key_that_types_is_its_character() {
        let a = translate(u32::from('a'), 0, 0).unwrap();
        assert_eq!((a.key, a.mods.shift), (Key::Char('a'), false));
        let upper = translate(u32::from('A'), SHIFT_MASK, 0).unwrap();
        assert_eq!((upper.key, upper.mods.shift), (Key::Char('A'), true));
        let colon = translate(u32::from(':'), SHIFT_MASK, 0).unwrap();
        assert_eq!((colon.key, colon.mods.shift), (Key::Char(':'), false));
        assert_eq!(translate(0x0100_3042, 0, 0).unwrap().key, Key::Char('あ'));
    }

    #[test]
    fn named_keys_are_named() {
        assert_eq!(translate(0x20, 0, 0).unwrap().key, Key::Space);
        assert_eq!(translate(0xffc2, 0, 0).unwrap().key, Key::F(5));
        assert_eq!(translate(0xff23, 0, 0).unwrap().key, Key::Henkan);
        assert_eq!(translate(0xff09, 0, 0).unwrap().key, Key::Tab);
        let back_tab = translate(0xfe20, SHIFT_MASK, 0).unwrap();
        assert_eq!((back_tab.key, back_tab.mods.shift), (Key::Tab, true));
        assert_eq!(translate(0xff63, 0, 0).unwrap().key, Key::Other, "Insert");
        assert_eq!(translate(0xff55, 0, 0).unwrap().key, Key::PageUp);
        assert_eq!(translate(0xff56, 0, 0).unwrap().key, Key::PageDown);
        assert_eq!(translate(0xff9a, 0, 0).unwrap().key, Key::PageUp, "keypad");
        assert_eq!(
            translate(0xff9b, 0, 0).unwrap().key,
            Key::PageDown,
            "keypad"
        );
    }

    #[test]
    fn a_named_key_let_go_is_released() {
        let e = translate(0x20, RELEASE_MASK, 0).unwrap();
        assert_eq!((e.key, e.kind), (Key::Space, KeyKind::Release));
    }

    const A: u32 = 38;
    const S: u32 = 39;

    #[test]
    fn a_key_let_go_is_the_key_its_press_was() {
        let mut keys = Keys::default();
        keys.translate(u32::from('A'), A, SHIFT_MASK, 0);
        let e = keys.translate(u32::from('a'), A, RELEASE_MASK, 0).unwrap();
        assert_eq!((e.key, e.kind), (Key::Char('A'), KeyKind::Release));
        let e = keys.translate(u32::from('a'), A, RELEASE_MASK, 0).unwrap();
        assert_eq!(
            e.key,
            Key::Char('a'),
            "a release with no press reads as it is"
        );
    }

    #[test]
    fn the_key_pressed_last_repeating_is_the_key_first_pressed() {
        let mut keys = Keys::default();
        keys.translate(u32::from('a'), A, 0, 0);
        keys.translate(0xffe1, 50, 0, 0);
        let repeat = keys.translate(u32::from('A'), A, SHIFT_MASK, 0).unwrap();
        assert_eq!(repeat.key, Key::Char('a'), "a modifier between counts not");
        assert_eq!(repeat.kind, KeyKind::Repeat);
        let e = keys
            .translate(u32::from('A'), A, SHIFT_MASK | RELEASE_MASK, 0)
            .unwrap();
        assert_eq!(e.key, Key::Char('a'));
    }

    #[test]
    fn a_press_after_another_key_is_read_afresh_though_its_release_went_elsewhere() {
        let mut keys = Keys::default();
        keys.translate(u32::from('A'), A, SHIFT_MASK, 0);
        keys.translate(u32::from('s'), S, 0, 0);
        let e = keys.translate(u32::from('a'), A, 0, 0).unwrap();
        assert_eq!(e.key, Key::Char('a'));
    }

    #[test]
    fn a_key_without_a_key_code_reads_as_its_keysym() {
        let mut keys = Keys::default();
        keys.translate(u32::from('A'), 0, SHIFT_MASK, 0);
        let e = keys.translate(u32::from('a'), 0, RELEASE_MASK, 0).unwrap();
        assert_eq!(e.key, Key::Char('a'));
    }

    #[test]
    fn caps_lock_let_go_is_left_out() {
        assert_eq!(translate(XK_CAPS_LOCK, RELEASE_MASK, 0), None);
    }

    #[test]
    fn the_keypad_is_the_keys_it_copies() {
        assert_eq!(translate(0xff8d, 0, 0).unwrap().key, Key::Enter, "KP_Enter");
        assert_eq!(translate(0xffb1, 0, 0).unwrap().key, Key::Char('1'), "KP_1");
        assert_eq!(
            translate(0xffab, 0, 0).unwrap().key,
            Key::Char('+'),
            "KP_Add"
        );
        assert_eq!(translate(0xff96, 0, 0).unwrap().key, Key::Left, "KP_Left");
        let enter = Chord {
            key: Key::Enter,
            mods: Modifiers::default(),
        };
        assert_eq!(key_to_send(enter), Some((0xff0d, 0)));
    }

    #[test]
    fn modifiers_come_from_the_state() {
        let h = translate(u32::from('h'), CONTROL_MASK | SUPER_MASK, 0).unwrap();
        assert!(h.mods.ctrl && h.mods.cmd && !h.mods.alt);
        assert!(translate(u32::from('a'), MOD4_MASK, 0).unwrap().mods.cmd);
    }

    #[test]
    fn caps_lock_is_the_cores_caps_lock_with_the_modifiers_held() {
        let e = translate(XK_CAPS_LOCK, SHIFT_MASK, 0).unwrap();
        assert_eq!(
            (e.key, e.kind, e.mods.shift),
            (Key::CapsLock, KeyKind::Press, true)
        );
    }

    #[test]
    fn zenkaku_hankaku_is_one_key_whichever_keysym_x_gives_it() {
        for keysym in [0xff2a, 0xff28, 0xff29] {
            assert_eq!(
                translate(keysym, 0, 0).unwrap().key,
                Key::ZenkakuHankaku,
                "{keysym:#x}"
            );
        }
    }

    #[test]
    fn the_keys_the_core_sends_have_keysyms() {
        let send = |key, ctrl| {
            key_to_send(Chord {
                key,
                mods: Modifiers {
                    ctrl,
                    ..Default::default()
                },
            })
        };
        assert_eq!(send(Key::Backspace, false), Some((0xff08, 0)));
        assert_eq!(send(Key::Left, true), Some((0xff51, CONTROL_MASK)));
        assert_eq!(send(Key::Char('a'), false), None);
    }

    #[test]
    fn text_is_erased_by_a_backspace_for_each_character() {
        let keys: Vec<_> = erasing("記者𥸮").collect();
        assert_eq!(keys, [(0xff08, 0); 3]);
    }

    #[test]
    fn super_is_sent_as_gtk_and_qt_both_read_it() {
        let chord = Chord {
            key: Key::Left,
            mods: Modifiers {
                cmd: true,
                ..Default::default()
            },
        };
        assert_eq!(key_to_send(chord), Some((0xff51, SUPER_MASK | MOD4_MASK)));
    }
}
