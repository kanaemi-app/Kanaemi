use crate::{Candidate, Chord};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
/// The input mode the user switches between. A reading and its candidates
/// are typed in kana mode; the text to register can be typed in either.
pub enum Mode {
    Abc,
    Kana,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CandidateView {
    pub items: Vec<Candidate>,
    pub selected: usize,
    /// The page shown, from 0.
    pub page: usize,
    /// How many pages the whole list takes; at least 1.
    pub pages: usize,
    /// For a reading to complete with, highlighted: the candidates
    /// converting it gives after its first, in order, as many as fit a page.
    pub more: Vec<String>,
    /// For candidates, what [`crate::Converter::ranks_by_model`] said when
    /// they were converted, for the host to show. `None` for readings to
    /// complete with.
    pub ranked_by_model: Option<bool>,
    /// For candidates, the first key bound to toggle the model, for the host
    /// to show with how they were ranked. `None` for readings to complete
    /// with.
    pub toggle_model: Option<Chord>,
}

/// A word being registered, a reading inside it included. The keys are the
/// first bound in the mode the text to register is typed in, told only while
/// it is typed outside a reading, where they do what they are told for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Registering {
    /// Leaves the registration without registering.
    pub cancel: Option<Chord>,
    /// Pastes into the text to register.
    pub paste: Option<Chord>,
}

/// What the host learns from an event, in the order it happened.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Effect {
    /// A candidate was committed for `reading` (okurigana included), as
    /// converted with `okurigana`, the chunk the core passed to
    /// [`crate::Converter::convert`].
    Committed {
        reading: String,
        okurigana: Option<String>,
        surface: String,
    },
    /// A word was registered. `reading` stops before any okurigana.
    /// `okurigana_head` is the okurigana's first chunk, typed where it was
    /// marked; the okurigana goes on past it only by romaji that chunk left
    /// over (`っ` of `った`).
    Registered {
        reading: String,
        okurigana: Option<String>,
        okurigana_head: Option<String>,
        surface: String,
    },
    /// `surface`, converted from `reading` with `okurigana` as for
    /// [`Effect::Committed`], is not to be offered for `reading` again.
    Forgotten {
        reading: String,
        okurigana: Option<String>,
        surface: String,
    },
    /// A commit reported as [`Effect::Committed`] was undone, to be learned
    /// as never made.
    Withdrawn {
        reading: String,
        okurigana: Option<String>,
        surface: String,
    },
    /// Text went into the field, converted or not.
    Typed(String),
    /// Text just before the caret was taken out of the field.
    Erased(String),
    /// The focus moved to another field: what was learned of the last one ends.
    FocusMoved,
}

/// The whole state to show after an event, so the host never has to remember the last one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Output {
    /// Whether the key was handled here; `false` means the host passes it to the application.
    pub consumed: bool,
    /// Text just before the caret to take out of the field before anything
    /// else is shown; the host tells with [`crate::Event::Erased`] once done.
    pub erase: Option<String>,
    /// Text to commit before passing the key on.
    pub commit: Option<String>,
    pub preedit: String,
    /// In characters from the start of the preedit.
    pub cursor: usize,
    pub candidates: Option<CandidateView>,
    pub mode: Mode,
    /// The mode to show near the cursor, set only by the event that changed it.
    pub indicator: Option<Mode>,
    /// A key the host sends to the application in place of the one pressed.
    pub send: Option<Chord>,
    /// Whether the host is to read the text on the clipboard and give it
    /// with [`crate::Core::paste`]: the core reads nothing itself.
    pub paste: bool,
    /// Set while a word is being registered, for the host to show near the
    /// cursor.
    pub registering: Option<Registering>,
    pub effects: Vec<Effect>,
}
