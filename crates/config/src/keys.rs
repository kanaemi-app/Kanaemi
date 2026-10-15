//! How the settings file names keys, actions and the bindings tables.

use kanaemi_core::{Action, Binding, Bindings, Chord, Form, Gesture, Key, Mode, Modifiers, Os};

/// Takes a binding out.
pub const UNBOUND: &str = "@none";

/// The bindings table that remaps keys sent to the application, beside the
/// table of each scene.
pub const APPLICATION_TABLE: &str = "application";

/// The table inside the application table that remaps keys on `os` only.
pub fn os_table(os: Os) -> &'static str {
    match os {
        Os::MacOs => "macos",
        Os::Windows => "windows",
        Os::Linux => "linux",
    }
}

/// The bindings table of the text to register, holding a table for each
/// mode written over it.
pub const REGISTRATION_TABLE: &str = "registration";

/// The table inside the registration table that binds keys in `mode` only.
/// Named apart from the keys, so that the かな key is bound beside it.
pub fn mode_table(mode: Mode) -> &'static str {
    match mode {
        Mode::Kana => "kana-mode",
        Mode::Abc => "abc-mode",
    }
}

/// The modes the text to register is typed in.
pub(crate) const MODES: [Mode; 2] = [Mode::Kana, Mode::Abc];

/// How many candidates `@select-N` can pick, each by its place on a page.
const PICKABLE: u8 = 9;

/// Where a key is pressed, by the table its bindings are written in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Scene {
    Reading,
    Completion,
    Candidates,
    /// The text to register being typed: in a mode, or in either.
    Registration(Option<Mode>),
    Kana,
    Abc,
}

impl Scene {
    const ALL: [Scene; 8] = [
        Self::Reading,
        Self::Completion,
        Self::Candidates,
        Self::Registration(None),
        Self::Registration(Some(Mode::Kana)),
        Self::Registration(Some(Mode::Abc)),
        Self::Kana,
        Self::Abc,
    ];

    /// Its table under `[keys]`, a table of the registration table by its
    /// path from there.
    fn name(self) -> String {
        match self {
            Self::Reading => "reading".to_owned(),
            Self::Completion => "completion".to_owned(),
            Self::Candidates => "candidates".to_owned(),
            Self::Registration(None) => REGISTRATION_TABLE.to_owned(),
            Self::Registration(Some(mode)) => format!("{REGISTRATION_TABLE}.{}", mode_table(mode)),
            Self::Kana => "kana".to_owned(),
            Self::Abc => "abc".to_owned(),
        }
    }

    pub(crate) fn named(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.name() == name)
    }

    /// What a key can be bound to here.
    pub(crate) fn allows(self, action: Action) -> bool {
        use Action::*;
        match self {
            Self::Reading => !matches!(
                action,
                Forget | Pick(_) | UndoCommit | NextPage | PreviousPage | Paste
            ),
            Self::Completion => !matches!(action, Forget | UndoCommit | RereadKana | Paste),
            Self::Candidates => !matches!(
                action,
                Delete | Left | Right | Home | End | UndoCommit | RereadKana | Paste
            ),
            Self::Registration(_) => matches!(
                action,
                Commit
                    | Cancel
                    | Backspace
                    | Delete
                    | Left
                    | Right
                    | Home
                    | End
                    | Abc
                    | Kana
                    | Begin
                    | Paste
            ),
            Self::Kana => matches!(action, Abc | Kana | Begin | UndoCommit | RereadKana),
            Self::Abc => matches!(action, Abc | Kana),
        }
    }

    /// Its bindings; in either mode of the registration, those of both
    /// alike.
    fn bindings(self, bindings: &Bindings) -> Vec<Binding> {
        match self {
            Self::Reading => bindings.reading.clone(),
            Self::Completion => bindings.completion.clone(),
            Self::Candidates => bindings.candidates.clone(),
            Self::Registration(Some(Mode::Kana)) => bindings.registration_kana.clone(),
            Self::Registration(Some(Mode::Abc)) => bindings.registration_abc.clone(),
            Self::Registration(None) => bindings
                .registration_kana
                .iter()
                .filter(|b| bindings.registration_abc.contains(b))
                .copied()
                .collect(),
            Self::Kana => bindings.kana.clone(),
            Self::Abc => bindings.abc.clone(),
        }
    }

    /// Its bindings, those of each mode the registration's are written in.
    pub(crate) fn bindings_mut(self, bindings: &mut Bindings) -> Vec<&mut Vec<Binding>> {
        match self {
            Self::Reading => vec![&mut bindings.reading],
            Self::Completion => vec![&mut bindings.completion],
            Self::Candidates => vec![&mut bindings.candidates],
            Self::Registration(Some(Mode::Kana)) => vec![&mut bindings.registration_kana],
            Self::Registration(Some(Mode::Abc)) => vec![&mut bindings.registration_abc],
            Self::Registration(None) => {
                vec![
                    &mut bindings.registration_kana,
                    &mut bindings.registration_abc,
                ]
            }
            Self::Kana => vec![&mut bindings.kana],
            Self::Abc => vec![&mut bindings.abc],
        }
    }
}

/// Every bindings table under `[keys]`: each scene's, then the application's.
/// A table inside another is named by its path from `[keys]`.
pub fn binding_tables() -> Vec<String> {
    Scene::ALL
        .into_iter()
        .map(Scene::name)
        .chain([APPLICATION_TABLE.to_owned()])
        .collect()
}

/// A key of a scene's bindings table, pressed so, as the settings file
/// writes it.
pub fn format_binding_key(chord: Chord, gesture: Gesture) -> Option<String> {
    let mut from = format_chord(chord)?;
    match gesture {
        Gesture::Press => {}
        Gesture::Tap => from.push_str(TAP),
        Gesture::Hold => from.push_str(HOLD),
    }
    Some(from)
}

/// The bindings of `table` as the settings file writes them, as (pressed key,
/// action or key sent). Empty for a table that does not exist. The
/// registration table has what both its modes bind alike, and each mode's
/// table what that mode binds.
pub fn bindings_table(bindings: &Bindings, table: &str) -> Vec<(String, String)> {
    match Scene::named(table) {
        Some(scene) => scene
            .bindings(bindings)
            .iter()
            .filter_map(|b| Some((format_binding_key(b.from, b.gesture)?, format_action(b.to))))
            .collect(),
        None if table == APPLICATION_TABLE => bindings
            .application
            .iter()
            .filter_map(|r| Some((format_chord(r.from)?, format_chord(r.to)?)))
            .collect(),
        None => Vec::new(),
    }
}

/// What a key can be bound to in the scene table `table`, in the order to
/// list them; none for the application table, which sends keys instead.
pub fn actions(table: impl AsRef<str>) -> Vec<Action> {
    let Some(scene) = Scene::named(table.as_ref()) else {
        return Vec::new();
    };
    every_action()
        .into_iter()
        .filter(|a| scene.allows(*a))
        .collect()
}

fn every_action() -> Vec<Action> {
    const FORMS: [Form; 5] = [
        Form::Hiragana,
        Form::Katakana,
        Form::HalfKatakana,
        Form::FullAlphanumeric,
        Form::Alphanumeric,
    ];
    let mut all = vec![Action::Next, Action::Previous, Action::Commit];
    all.extend(FORMS.map(Action::Form));
    all.extend(FORMS.map(Action::CommitForm));
    all.extend([
        Action::Cancel,
        Action::Backspace,
        Action::Delete,
        Action::Left,
        Action::Right,
        Action::Home,
        Action::End,
        Action::Complete,
        Action::CompletePrevious,
        Action::NextPage,
        Action::PreviousPage,
        Action::Register,
        Action::Forget,
        Action::Abc,
        Action::Kana,
        Action::Begin,
    ]);
    all.extend((0..PICKABLE).map(Action::Pick));
    all.extend([Action::UndoCommit, Action::RereadKana, Action::Paste]);
    all
}

/// An action as the settings file writes it, `@next`.
pub fn format_action(action: Action) -> String {
    let form = |form| match form {
        Form::Hiragana => "hiragana",
        Form::Katakana => "katakana",
        Form::HalfKatakana => "half-katakana",
        Form::FullAlphanumeric => "full-alphanumeric",
        Form::Alphanumeric => "alphanumeric",
    };
    let name = match action {
        Action::Next => "next".to_owned(),
        Action::Previous => "previous".to_owned(),
        Action::Commit => "commit".to_owned(),
        Action::Form(f) => form(f).to_owned(),
        Action::CommitForm(f) => format!("commit-{}", form(f)),
        Action::Cancel => "cancel".to_owned(),
        Action::Backspace => "backspace".to_owned(),
        Action::Delete => "delete".to_owned(),
        Action::Left => "left".to_owned(),
        Action::Right => "right".to_owned(),
        Action::Home => "home".to_owned(),
        Action::End => "end".to_owned(),
        Action::Register => "register".to_owned(),
        Action::Forget => "forget".to_owned(),
        Action::Abc => "abc".to_owned(),
        Action::Kana => "kana".to_owned(),
        Action::Begin => "begin".to_owned(),
        Action::Pick(n) => format!("select-{}", u16::from(n) + 1),
        Action::UndoCommit => "undo-commit".to_owned(),
        Action::Complete => "complete".to_owned(),
        Action::CompletePrevious => "complete-previous".to_owned(),
        Action::RereadKana => "reread-kana".to_owned(),
        Action::NextPage => "next-page".to_owned(),
        Action::PreviousPage => "previous-page".to_owned(),
        Action::Paste => "paste".to_owned(),
    };
    format!("@{name}")
}

/// The action `@name` names, as [`format_action`] writes it.
pub fn parse_action(text: impl AsRef<str>) -> Option<Action> {
    let text = text.as_ref();
    every_action()
        .into_iter()
        .find(|a| format_action(*a) == text)
}

/// A key of a scene's bindings table and how it is pressed: a chord as
/// [`parse_chord`] reads it; a modifier key alone with `#tap` after it for
/// its tap (`left-shift#tap`); or a key that types a character with `#hold`
/// after it for holding it (`space#hold`).
pub fn parse_binding_key(text: impl AsRef<str>) -> Option<(Chord, Gesture)> {
    let text = text.as_ref();
    let annotated = |suffix| text.strip_suffix(suffix).filter(|k: &&str| !k.is_empty());
    if let Some(key) = annotated(TAP) {
        let chord = parse_chord(key)?;
        return (is_modifier(chord.key) && chord.mods == Modifiers::default())
            .then_some((chord, Gesture::Tap));
    }
    if let Some(key) = annotated(HOLD) {
        // Pressed alone, a held key acts only once it is let go, when the
        // press can no longer go on to the application; only a character
        // can still be typed in its place.
        let chord = parse_chord(key)?;
        let typing = matches!(chord.key, Key::Char(_) | Key::Space)
            && !(chord.mods.ctrl || chord.mods.cmd || chord.mods.alt);
        return typing.then_some((chord, Gesture::Hold));
    }
    Some((parse_chord(text)?, Gesture::Press))
}

/// Written after a modifier key for its tap.
const TAP: &str = "#tap";
/// Written after a key that types a character for holding it.
const HOLD: &str = "#hold";

/// A key with its modifiers, as `ctrl+h` or `shift+space`.
pub fn parse_chord(text: impl AsRef<str>) -> Option<Chord> {
    let mut mods = Modifiers::default();
    let mut rest = text.as_ref();
    // Modifiers come first; whatever is left is the key, so `+` itself can be one.
    while let Some((modifier, key)) = rest.split_once('+')
        && !key.is_empty()
    {
        match modifier {
            "ctrl" => mods.ctrl = true,
            "cmd" => mods.cmd = true,
            "alt" => mods.alt = true,
            "shift" => mods.shift = true,
            _ => return None,
        }
        rest = key;
    }
    let key = match parse_key(rest)? {
        // Hosts report a shifted letter as the capital it types.
        Key::Char(c) if mods.shift => Key::Char(c.to_ascii_uppercase()),
        key => key,
    };
    Some(Chord { key, mods })
}

/// How a chord is written in the settings file; `None` for a key the file has
/// no name for.
pub fn format_chord(chord: Chord) -> Option<String> {
    let key = match chord.key {
        // A shifted letter is read back as its capital.
        Key::Char(c) if chord.mods.shift => c.to_ascii_lowercase().to_string(),
        key => format_key(key)?,
    };
    let mods = chord.mods;
    let names = [
        (mods.ctrl, "ctrl+"),
        (mods.cmd, "cmd+"),
        (mods.alt, "alt+"),
        (mods.shift, "shift+"),
    ];
    let mut text: String = names
        .iter()
        .filter(|(on, _)| *on)
        .map(|(_, n)| *n)
        .collect();
    text.push_str(&key);
    Some(text)
}

/// The keys the settings file writes by name, then the function keys: what
/// can be written after any modifiers besides a single character.
pub fn key_names() -> Vec<String> {
    NAMED_KEYS
        .iter()
        .map(|(name, _)| (*name).to_owned())
        .chain((1..=12).map(|n| format!("f{n}")))
        .collect()
}

/// How a key is written in the settings file.
fn format_key(key: Key) -> Option<String> {
    match key {
        Key::Char(c) => Some(c.to_string()),
        Key::F(n) => Some(format!("f{n}")),
        key => NAMED_KEYS
            .iter()
            .find(|(_, named)| *named == key)
            .map(|(name, _)| (*name).to_owned()),
    }
}

/// The keys the application table may send, as the file writes them.
pub fn sendable_keys() -> Vec<&'static str> {
    NAMED_KEYS
        .iter()
        .filter(|(_, key)| sendable(*key))
        .map(|(name, _)| *name)
        .collect()
}

const NAMED_KEYS: &[(&str, Key)] = &[
    ("space", Key::Space),
    ("tab", Key::Tab),
    ("enter", Key::Enter),
    ("esc", Key::Esc),
    ("backspace", Key::Backspace),
    ("delete", Key::Delete),
    ("left", Key::Left),
    ("right", Key::Right),
    ("up", Key::Up),
    ("down", Key::Down),
    ("home", Key::Home),
    ("end", Key::End),
    ("page-up", Key::PageUp),
    ("page-down", Key::PageDown),
    ("left-shift", Key::ShiftLeft),
    ("right-shift", Key::ShiftRight),
    ("left-ctrl", Key::CtrlLeft),
    ("right-ctrl", Key::CtrlRight),
    ("left-cmd", Key::CmdLeft),
    ("right-cmd", Key::CmdRight),
    ("left-alt", Key::AltLeft),
    ("right-alt", Key::AltRight),
    ("eisu", Key::Eisu),
    ("kana", Key::Kana),
    ("henkan", Key::Henkan),
    ("muhenkan", Key::Muhenkan),
    ("zenkaku-hankaku", Key::ZenkakuHankaku),
    ("caps-lock", Key::CapsLock),
];

/// Only named keys can be sent: a character's key code depends on the
/// keyboard layout.
pub(crate) fn sendable(key: Key) -> bool {
    matches!(
        key,
        Key::Backspace
            | Key::Delete
            | Key::Enter
            | Key::Esc
            | Key::Space
            | Key::Left
            | Key::Right
            | Key::Up
            | Key::Down
            | Key::Home
            | Key::End
    )
}

/// A modifier key, which a host passes on to the application whatever it is
/// bound to.
pub(crate) fn is_modifier(key: Key) -> bool {
    matches!(
        key,
        Key::ShiftLeft
            | Key::ShiftRight
            | Key::CtrlLeft
            | Key::CtrlRight
            | Key::CmdLeft
            | Key::CmdRight
            | Key::AltLeft
            | Key::AltRight
    )
}

fn parse_key(name: &str) -> Option<Key> {
    if let Some((_, key)) = NAMED_KEYS.iter().find(|(named, _)| *named == name) {
        return Some(*key);
    }
    if let Some(n) = name.strip_prefix('f').and_then(|n| n.parse::<u8>().ok())
        && (1..=12).contains(&n)
    {
        return Some(Key::F(n));
    }
    let mut chars = name.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) if !c.is_control() => Some(Key::Char(c)),
        _ => None,
    }
}

/// A key as written in the settings file, the way keyboards label it:
/// `ctrl+h` is shown as `Ctrl+H`.
pub fn key_label(text: &str) -> String {
    if text.starts_with('@') {
        return text.to_owned();
    }
    if let Some(key) = text.strip_suffix("#tap").filter(|k| !k.is_empty()) {
        return format!("{} の単独押し", key_label(key));
    }
    if let Some(key) = text.strip_suffix("#hold").filter(|k| !k.is_empty()) {
        return format!("{} を押さえたまま", key_label(key));
    }
    let mut parts = Vec::new();
    let mut rest = text;
    while let Some((modifier, key)) = rest.split_once('+')
        && !key.is_empty()
    {
        parts.push(
            match modifier {
                "ctrl" => "Ctrl",
                "cmd" => CMD,
                "alt" => ALT,
                "shift" => "Shift",
                other => other,
            }
            .to_owned(),
        );
        rest = key;
    }
    parts.push(match rest {
        "space" => "Space".to_owned(),
        "enter" => "Enter".to_owned(),
        "esc" => "Esc".to_owned(),
        "backspace" => "Backspace".to_owned(),
        "delete" => "Delete".to_owned(),
        "left" => "←".to_owned(),
        "right" => "→".to_owned(),
        "up" => "↑".to_owned(),
        "down" => "↓".to_owned(),
        "home" => "Home".to_owned(),
        "end" => "End".to_owned(),
        "page-up" => "Page Up".to_owned(),
        "page-down" => "Page Down".to_owned(),
        "left-shift" => "左 Shift".to_owned(),
        "right-shift" => "右 Shift".to_owned(),
        "left-ctrl" => "左 Ctrl".to_owned(),
        "right-ctrl" => "右 Ctrl".to_owned(),
        "left-cmd" => format!("左 {CMD}"),
        "right-cmd" => format!("右 {CMD}"),
        "left-alt" => format!("左 {ALT}"),
        "right-alt" => format!("右 {ALT}"),
        "eisu" => "英数".to_owned(),
        "kana" => "かな".to_owned(),
        "henkan" => "変換".to_owned(),
        "muhenkan" => "無変換".to_owned(),
        "zenkaku-hankaku" => "半角/全角".to_owned(),
        "caps-lock" => "Caps Lock".to_owned(),
        key => key.to_uppercase(),
    });
    parts.join("+")
}

#[cfg(target_os = "macos")]
const CMD: &str = "Cmd";
#[cfg(not(target_os = "macos"))]
const CMD: &str = "Win";
#[cfg(target_os = "macos")]
const ALT: &str = "Option";
#[cfg(not(target_os = "macos"))]
const ALT: &str = "Alt";
