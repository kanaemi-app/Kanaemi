//! What every platform shows near the cursor while a word is registered.

use kanaemi_config::{format_chord, key_label};
use kanaemi_core::{Chord, Registering};

/// That a word is being registered, the key that leaves it, and the key that
/// pastes into it.
pub fn registration_hint(registering: Registering) -> String {
    let label = |key: Option<Chord>| key.and_then(format_chord).map(|key| key_label(&key));
    let keys: Vec<String> = [
        label(registering.cancel).map(|key| format!("{key} で抜ける")),
        label(registering.paste).map(|key| format!("{key} でクリップボードから貼り付け")),
    ]
    .into_iter()
    .flatten()
    .collect();
    if keys.is_empty() {
        return "辞書登録中".to_owned();
    }
    format!("辞書登録中　{}", keys.join("。"))
}

#[cfg(test)]
mod tests {
    use kanaemi_core::{Key, Modifiers};

    use super::*;

    const ESC: Chord = Chord {
        key: Key::Esc,
        mods: Modifiers {
            shift: false,
            ctrl: false,
            cmd: false,
            alt: false,
        },
    };

    const CTRL_Y: Chord = Chord {
        key: Key::Char('y'),
        mods: Modifiers {
            shift: false,
            ctrl: true,
            cmd: false,
            alt: false,
        },
    };

    #[test]
    fn the_hint_names_the_keys_as_keyboards_label_them() {
        assert_eq!(
            registration_hint(Registering {
                cancel: Some(ESC),
                paste: Some(CTRL_Y)
            }),
            "辞書登録中　Esc で抜ける。Ctrl+Y でクリップボードから貼り付け"
        );
    }

    #[test]
    fn a_key_not_told_is_left_out() {
        assert_eq!(
            registration_hint(Registering {
                cancel: Some(ESC),
                paste: None
            }),
            "辞書登録中　Esc で抜ける"
        );
        assert_eq!(
            registration_hint(Registering {
                cancel: None,
                paste: Some(CTRL_Y)
            }),
            "辞書登録中　Ctrl+Y でクリップボードから貼り付け"
        );
    }

    #[test]
    fn with_no_key_told_the_hint_tells_only_of_registering() {
        assert_eq!(
            registration_hint(Registering {
                cancel: None,
                paste: None
            }),
            "辞書登録中"
        );
    }
}
