//! The hint shown by the caret while a word is registered: that it is
//! being registered, and the keys that leave it and paste into it. Unlike
//! the mode indicator it stays for as long as the word is typed, above the
//! line so that the indicator below it leaves it uncovered.

use kanaemi_core::{Output, Registering};

#[cfg(windows)]
mod window;
#[cfg(windows)]
pub(crate) use window::*;

/// What the hint is to tell of after `output`, `None` to hide it. The
/// candidate list of a reading in the word takes its place while it shows.
pub(crate) fn wanted(output: &Output) -> Option<Registering> {
    output.registering.filter(|_| output.candidates.is_none())
}

#[cfg(test)]
mod tests {
    use kanaemi_core::{Candidate, CandidateView, Mode};

    use super::*;

    fn output(registering: Option<Registering>, candidates: Option<CandidateView>) -> Output {
        Output {
            consumed: true,
            erase: None,
            commit: None,
            preedit: "»ぬぬ « ".to_owned(),
            cursor: 0,
            candidates,
            mode: Mode::Kana,
            indicator: None,
            send: None,
            paste: false,
            registering,
            effects: Vec::new(),
        }
    }

    const REGISTERING: Registering = Registering {
        cancel: None,
        paste: None,
    };

    #[test]
    fn the_hint_shows_while_a_word_is_registered() {
        assert_eq!(wanted(&output(Some(REGISTERING), None)), Some(REGISTERING));
    }

    #[test]
    fn the_hint_hides_when_no_word_is_registered() {
        assert_eq!(wanted(&output(None, None)), None);
    }

    #[test]
    fn the_candidate_list_takes_the_place_of_the_hint() {
        let candidates = CandidateView {
            items: vec![Candidate {
                surface: "沼".to_owned(),
                source: None,
                preview: None,
            }],
            selected: 0,
            page: 0,
            pages: 1,
            more: Vec::new(),
            ranked_by_model: None,
            toggle_model: None,
        };
        assert_eq!(wanted(&output(Some(REGISTERING), Some(candidates))), None);
    }
}
