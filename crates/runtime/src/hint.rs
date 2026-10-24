//! What every platform shows with the candidates and while a word is registered.

use kanaemi_config::{format_chord, key_label};
use kanaemi_core::{CandidateView, Chord, Registering};

/// That a word is being registered, the key that leaves it, and the key that
/// pastes into it.
pub fn registration_hint(registering: Registering) -> String {
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

/// How the candidates were ranked, and the key that toggles the model.
pub fn ranking_label(view: &CandidateView) -> Option<String> {
    let ranking = if view.ranked_by_model? {
        "モデルあり"
    } else {
        "モデルなし"
    };
    Some(match label(view.toggle_model) {
        Some(key) => format!("{ranking}（{key} で切り替え）"),
        None => ranking.to_owned(),
    })
}

fn label(key: Option<Chord>) -> Option<String> {
    key.and_then(format_chord).map(|key| key_label(&key))
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

    const CTRL_R: Chord = Chord {
        key: Key::Char('r'),
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

    fn view(ranked_by_model: Option<bool>, toggle_model: Option<Chord>) -> CandidateView {
        CandidateView {
            items: Vec::new(),
            selected: 0,
            page: 0,
            pages: 1,
            more: Vec::new(),
            ranked_by_model,
            toggle_model,
        }
    }

    #[test]
    fn the_ranking_is_labelled_with_the_key_that_toggles_the_model() {
        assert_eq!(
            ranking_label(&view(Some(true), Some(CTRL_R))).as_deref(),
            Some("モデルあり（Ctrl+R で切り替え）")
        );
        assert_eq!(
            ranking_label(&view(Some(false), Some(CTRL_R))).as_deref(),
            Some("モデルなし（Ctrl+R で切り替え）")
        );
    }

    #[test]
    fn with_no_key_told_the_ranking_is_labelled_alone() {
        assert_eq!(
            ranking_label(&view(Some(true), None)).as_deref(),
            Some("モデルあり")
        );
    }

    #[test]
    fn a_list_not_ranked_is_not_labelled() {
        assert_eq!(ranking_label(&view(None, Some(CTRL_R))), None);
    }
}
