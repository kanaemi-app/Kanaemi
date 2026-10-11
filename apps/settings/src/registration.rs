//! The keys bound while the text to register is typed, in both modes at
//! once: what each mode does for a pressed key, from the table for both
//! modes, the table of that mode and Kanaemi's own bindings, and the fewest
//! lines of those tables that make each mode do what is wanted.

use kanaemi_config::{
    BindingLine, REGISTRATION_TABLE, RegistrationTables, UNBOUND, format_action,
    format_binding_key, mode_table,
};
use kanaemi_core::{Action, Binding, Bindings, Chord, Gesture, Mode};

use crate::remaps::Edit;

/// The modes the text to register is typed in, in the order to show them.
pub const MODES: [Mode; 2] = [Mode::Kana, Mode::Abc];

/// The registration scene in kana mode, by the table that binds it.
pub const KANA_SCENE: &str = "registration.kana-mode";
/// The registration scene in ABC mode, by the table that binds it.
pub const ABC_SCENE: &str = "registration.abc-mode";

/// The scene of the text to register typed in `mode`.
pub fn scene(mode: Mode) -> &'static str {
    match mode {
        Mode::Kana => KANA_SCENE,
        Mode::Abc => ABC_SCENE,
    }
}

/// The mode a scene is the text to register typed in, if it is one.
pub fn mode_of(scene: &str) -> Option<Mode> {
    MODES.into_iter().find(|mode| self::scene(*mode) == scene)
}

/// A pressed key, and how it is pressed.
pub type Pressed = (Chord, Gesture);

/// One pressed key, and what each mode does for it, `None` where it is
/// bound to nothing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub key: Pressed,
    pub actions: Vec<(Mode, Option<Action>)>,
}

impl Row {
    /// Whether every mode does what Kanaemi makes it do.
    pub fn is_default(&self) -> bool {
        self.actions
            .iter()
            .all(|(mode, action)| *action == default_action(*mode, self.key))
    }
}

/// What `mode` does for `key`, `None` when it is bound to nothing.
pub fn action(tables: &RegistrationTables, mode: Mode, key: Pressed) -> Option<Action> {
    let mode_line = tables.modes.get(&mode).and_then(|lines| line(lines, key));
    match mode_line.or_else(|| line(&tables.shared, key)) {
        Some(action) => action,
        None => default_action(mode, key),
    }
}

/// What `mode` does for `key` with nothing written.
pub fn default_action(mode: Mode, key: Pressed) -> Option<Action> {
    defaults(mode)
        .into_iter()
        .find(|b| (b.from, b.gesture) == key)
        .map(|b| b.to)
}

fn defaults(mode: Mode) -> Vec<Binding> {
    let shipped = Bindings::default();
    match mode {
        Mode::Kana => shipped.registration_kana,
        Mode::Abc => shipped.registration_abc,
    }
}

/// What a table writes for `key`: `None` with no line, `Some(None)` for a
/// line taking the binding out.
fn line(lines: &[BindingLine], key: Pressed) -> Option<Option<Action>> {
    lines
        .iter()
        .find(|(from, gesture, _)| (*from, *gesture) == key)
        .map(|(_, _, action)| *action)
}

/// Every key Kanaemi binds in some mode or a table writes: Kanaemi's own
/// first, in their order, then the written ones.
pub fn rows(tables: &RegistrationTables) -> Vec<Row> {
    let defaults = MODES
        .into_iter()
        .flat_map(defaults)
        .map(|b| (b.from, b.gesture));
    let written = MODES
        .iter()
        .filter_map(|mode| tables.modes.get(mode))
        .flatten();
    let written = tables
        .shared
        .iter()
        .chain(written)
        .map(|(from, gesture, _)| (*from, *gesture));
    let mut keys: Vec<Pressed> = Vec::new();
    for key in defaults.chain(written) {
        if !keys.contains(&key) {
            keys.push(key);
        }
    }
    keys.into_iter()
        .map(|key| Row {
            key,
            actions: MODES.map(|mode| (mode, action(tables, mode, key))).to_vec(),
        })
        .collect()
}

/// What makes each mode do `wanted(mode)` for `key`: the shared line, if
/// one leaves fewer lines in all, and the mode lines. Lines already written
/// so stay as they are.
pub fn writing(
    tables: &RegistrationTables,
    key: Pressed,
    wanted: impl Fn(Mode) -> Option<Action>,
) -> Vec<Edit> {
    let Some(name) = format_binding_key(key.0, key.1) else {
        return Vec::new();
    };
    let wanted = MODES.map(|mode| (mode, wanted(mode)));
    // What each mode does with the shared line `shared` and no line of its
    // own.
    let beneath = |shared: Option<Option<Action>>, mode: Mode| match shared {
        Some(action) => action,
        None => default_action(mode, key),
    };
    let mode_lines = |shared: Option<Option<Action>>| {
        wanted.map(|(mode, action)| (mode, (beneath(shared, mode) != action).then_some(action)))
    };
    let count = |shared: Option<Option<Action>>| {
        usize::from(shared.is_some())
            + mode_lines(shared)
                .iter()
                .filter(|(_, l)| l.is_some())
                .count()
    };
    // With two modes a shared line is worth it only when both want the same,
    // so a tie keeps the shared line already written rather than moving it.
    let now = line(&tables.shared, key);
    let mut shared = now;
    for candidate in [None].into_iter().chain(wanted.map(|(_, a)| Some(a))) {
        if count(candidate) < count(shared) {
            shared = candidate;
        }
    }
    let path = |mode: Option<Mode>| {
        ["keys", REGISTRATION_TABLE]
            .into_iter()
            .chain(mode.map(mode_table))
            .chain([name.as_str()])
            .map(str::to_owned)
            .collect::<Vec<_>>()
    };
    let mut edits = Vec::new();
    let mut edit = |at: Option<Mode>, now: Option<Option<Action>>, then: Option<Option<Action>>| {
        if now != then {
            edits.push((path(at), then.map(written)));
        }
    };
    edit(None, now, shared);
    for (mode, then) in mode_lines(shared) {
        let now = tables.modes.get(&mode).and_then(|lines| line(lines, key));
        edit(Some(mode), now, then);
    }
    edits
}

/// An action as the settings file writes it, or the binding taken out.
fn written(action: Option<Action>) -> String {
    match action {
        Some(action) => format_action(action),
        None => UNBOUND.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::path::Path;

    use kanaemi_config::{Settings, binding_tables, parse_action, parse_binding_key};

    use super::*;
    use crate::remaps::tests::edited;

    fn pressed(text: &str) -> Pressed {
        parse_binding_key(text).unwrap()
    }

    fn act(text: &str) -> Action {
        parse_action(text).unwrap()
    }

    /// The lines of a table, each key with what it is bound to.
    type Lines<'a> = &'a [(&'a str, Option<&'a str>)];

    fn tables(shared: Lines, modes: &[(Mode, Lines)]) -> RegistrationTables {
        let entries = |list: Lines| {
            list.iter()
                .map(|(from, to)| {
                    let (chord, gesture) = pressed(from);
                    (chord, gesture, to.map(act))
                })
                .collect::<Vec<_>>()
        };
        RegistrationTables {
            shared: entries(shared),
            modes: modes
                .iter()
                .map(|(mode, list)| (*mode, entries(list)))
                .collect::<HashMap<_, _>>(),
        }
    }

    fn shared_line(key: &str, value: Option<&str>) -> Edit {
        (
            ["keys", REGISTRATION_TABLE, key]
                .map(str::to_owned)
                .to_vec(),
            value.map(str::to_owned),
        )
    }

    fn mode_line(mode: Mode, key: &str, value: Option<&str>) -> Edit {
        (
            ["keys", REGISTRATION_TABLE, mode_table(mode), key]
                .map(str::to_owned)
                .to_vec(),
            value.map(str::to_owned),
        )
    }

    /// The tables a file holds once `edits` are written to one holding
    /// `text`.
    fn after(text: &str, edits: &[Edit]) -> RegistrationTables {
        let text = edited(text, edits);
        let (settings, problems) = Settings::load(&text, Path::new("/nonexistent"));
        assert!(problems.is_empty(), "{problems:?}\n{text}");
        settings.registration_tables
    }

    #[test]
    fn each_mode_is_a_scene_named_by_the_table_that_binds_it() {
        let tables = binding_tables();

        for mode in MODES {
            assert!(tables.contains(&scene(mode).to_owned()), "{mode:?}");
            assert_eq!(
                scene(mode),
                format!("{REGISTRATION_TABLE}.{}", mode_table(mode))
            );
            assert_eq!(mode_of(scene(mode)), Some(mode));
        }
        assert_eq!(mode_of(REGISTRATION_TABLE), None);
    }

    #[test]
    fn with_nothing_written_each_mode_does_kanaemis_own() {
        let none = RegistrationTables::default();

        for mode in MODES {
            assert_eq!(action(&none, mode, pressed("enter")), Some(Action::Commit));
            assert_eq!(action(&none, mode, pressed("ctrl+o")), None);
        }
    }

    #[test]
    fn the_shared_table_is_written_over_kanaemis_own_and_a_mode_table_over_both() {
        let written = tables(
            &[("enter", Some("@cancel")), ("ctrl+o", Some("@commit"))],
            &[(
                Mode::Abc,
                &[("enter", None), ("ctrl+o", Some("@backspace"))],
            )],
        );

        assert_eq!(
            action(&written, Mode::Kana, pressed("enter")),
            Some(Action::Cancel)
        );
        assert_eq!(action(&written, Mode::Abc, pressed("enter")), None);
        assert_eq!(
            action(&written, Mode::Kana, pressed("ctrl+o")),
            Some(Action::Commit)
        );
        assert_eq!(
            action(&written, Mode::Abc, pressed("ctrl+o")),
            Some(Action::Backspace)
        );
    }

    #[test]
    fn every_key_bound_by_default_or_written_is_a_row() {
        let written = tables(
            &[("ctrl+o", Some("@commit"))],
            &[(
                Mode::Kana,
                &[("left-ctrl#tap", Some("@kana")), ("enter", None)],
            )],
        );

        let rows = rows(&written);

        let keys: Vec<Pressed> = rows.iter().map(|r| r.key).collect();
        let mut expected: Vec<Pressed> = Vec::new();
        for b in MODES.into_iter().flat_map(defaults) {
            if !expected.contains(&(b.from, b.gesture)) {
                expected.push((b.from, b.gesture));
            }
        }
        expected.extend([pressed("ctrl+o"), pressed("left-ctrl#tap")]);
        assert_eq!(keys, expected);
        let enter = rows.iter().find(|r| r.key == pressed("enter")).unwrap();
        assert_eq!(
            enter.actions,
            [(Mode::Kana, None), (Mode::Abc, Some(Action::Commit))]
        );
    }

    #[test]
    fn a_row_is_kanaemis_own_while_each_mode_does_its_default() {
        let written = tables(&[], &[(Mode::Abc, &[("esc", Some("@commit"))])]);

        let rows = rows(&written);

        let row = |key| rows.iter().find(|r| r.key == pressed(key)).unwrap();
        assert!(row("enter").is_default());
        assert!(!row("esc").is_default());
    }

    #[test]
    fn what_both_modes_do_by_default_writes_nothing() {
        let key = pressed("enter");

        let edits = writing(&RegistrationTables::default(), key, |mode| {
            default_action(mode, key)
        });

        assert_eq!(edits, []);
    }

    #[test]
    fn a_key_added_in_both_modes_is_one_line_in_the_shared_table() {
        let edits = writing(&RegistrationTables::default(), pressed("ctrl+o"), |_| {
            Some(Action::Commit)
        });

        assert_eq!(edits, [shared_line("ctrl+o", Some("@commit"))]);
    }

    #[test]
    fn one_mode_apart_from_the_default_is_written_in_its_table_only() {
        let key = pressed("enter");

        let edits = writing(&RegistrationTables::default(), key, |mode| match mode {
            Mode::Kana => Some(Action::Cancel),
            Mode::Abc => default_action(mode, key),
        });

        assert_eq!(edits, [mode_line(Mode::Kana, "enter", Some("@cancel"))]);
    }

    #[test]
    fn each_mode_wanting_its_own_writes_a_line_in_each_mode_table() {
        let edits = writing(&RegistrationTables::default(), pressed("ctrl+o"), |mode| {
            Some(match mode {
                Mode::Kana => Action::Commit,
                Mode::Abc => Action::Cancel,
            })
        });

        assert_eq!(
            edits,
            [
                mode_line(Mode::Kana, "ctrl+o", Some("@commit")),
                mode_line(Mode::Abc, "ctrl+o", Some("@cancel")),
            ]
        );
    }

    #[test]
    fn taking_a_key_off_in_both_modes_writes_none_once_in_the_shared_table() {
        let edits = writing(&RegistrationTables::default(), pressed("enter"), |_| None);

        assert_eq!(edits, [shared_line("enter", Some(UNBOUND))]);
    }

    #[test]
    fn taking_a_key_off_in_one_mode_writes_none_in_that_table() {
        let key = pressed("left-shift#tap");

        let edits = writing(&RegistrationTables::default(), key, |mode| {
            (mode == Mode::Kana)
                .then(|| default_action(mode, key))
                .flatten()
        });

        assert_eq!(
            edits,
            [mode_line(Mode::Abc, "left-shift#tap", Some(UNBOUND))]
        );
    }

    #[test]
    fn going_back_to_kanaemis_own_removes_every_line_of_the_key() {
        let written = tables(
            &[("enter", Some("@cancel"))],
            &[
                (Mode::Kana, &[("enter", None)]),
                (Mode::Abc, &[("enter", Some("@backspace"))]),
            ],
        );
        let key = pressed("enter");

        let edits = writing(&written, key, |mode| default_action(mode, key));

        assert_eq!(
            edits,
            [
                shared_line("enter", None),
                mode_line(Mode::Kana, "enter", None),
                mode_line(Mode::Abc, "enter", None),
            ]
        );
    }

    #[test]
    fn a_line_already_written_as_wanted_is_left_alone() {
        let written = tables(
            &[("ctrl+o", Some("@commit"))],
            &[(Mode::Abc, &[("ctrl+o", Some("@cancel"))])],
        );

        let edits = writing(&written, pressed("ctrl+o"), |mode| {
            Some(match mode {
                Mode::Kana => Action::Commit,
                Mode::Abc => Action::Cancel,
            })
        });

        assert_eq!(edits, []);
    }

    #[test]
    fn a_mode_line_equal_to_the_shared_one_beneath_it_is_removed() {
        let written = tables(
            &[("ctrl+o", Some("@commit"))],
            &[(Mode::Abc, &[("ctrl+o", Some("@cancel"))])],
        );

        let edits = writing(&written, pressed("ctrl+o"), |_| Some(Action::Commit));

        assert_eq!(edits, [mode_line(Mode::Abc, "ctrl+o", None)]);
    }

    #[test]
    fn a_key_is_written_the_one_way_the_file_names_it() {
        let edits = writing(
            &RegistrationTables::default(),
            pressed("shift+ctrl+o"),
            |_| Some(Action::Commit),
        );

        assert_eq!(edits, [shared_line("ctrl+shift+o", Some("@commit"))]);
        let edits = writing(
            &RegistrationTables::default(),
            pressed("space#hold"),
            |_| Some(Action::Begin),
        );
        assert_eq!(edits, [shared_line("space#hold", Some("@begin"))]);
    }

    #[test]
    fn what_is_written_makes_each_mode_do_what_was_wanted() {
        let text = "[keys.registration]\nenter = \"@cancel\"\n\"shift+ctrl+o\" = \"@commit\"\n\
                    [keys.registration.kana-mode]\nenter = \"@none\"\n\
                    [keys.registration.abc-mode]\n\"ctrl+shift+o\" = \"@backspace\"\n";
        let (settings, problems) = Settings::load(text, Path::new("/nonexistent"));
        assert!(problems.is_empty(), "{problems:?}");
        let wants = [
            [Some(Action::Begin), None],
            [Some(Action::Commit), Some(Action::Commit)],
            [None, None],
            [None, Some(Action::Cancel)],
        ];
        for key in ["enter", "ctrl+shift+o", "left-shift#tap", "ctrl+x"].map(pressed) {
            for want in wants {
                let wanted = |mode: Mode| want[usize::from(mode == Mode::Abc)];

                let edits = writing(&settings.registration_tables, key, wanted);

                let written = after(text, &edits);
                for mode in MODES {
                    assert_eq!(
                        action(&written, mode, key),
                        wanted(mode),
                        "{key:?} in {mode:?}"
                    );
                }
            }
        }
    }
}
