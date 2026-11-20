//! Placeholders in a surface, which a function fills each time the item is
//! converted.
//!
//! An item holds a placeholder as [`OPEN`] and [`CLOSE`] around what was
//! written between its braces, so a literal `{` or `}` in a word stays itself.

pub(crate) const OPEN: char = '\u{FDD0}';
pub(crate) const CLOSE: char = '\u{FDD1}';

/// The functions placeholders name. The engine itself has only the one of the
/// empty name, which gives its source back.
pub trait Functions {
    fn has(&self, name: &str) -> bool;

    /// The values the function fills a placeholder with, each a candidate of
    /// its own; `None` or none when it fails or gives none.
    fn call(&self, call: &Call) -> Option<Vec<Value>>;
}

/// One value a function fills a placeholder with.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Value {
    pub text: String,
    /// The argument the function gives this value alone for, which names the
    /// item the value's records go to.
    pub argument: Option<String>,
}

impl From<String> for Value {
    fn from(text: String) -> Self {
        Self {
            text,
            argument: None,
        }
    }
}

/// A surface filled with one value for each placeholder.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Filled {
    pub(crate) text: String,
    /// The surface the records of this candidate go to: the item's, with each
    /// placeholder whose value has an argument called with it.
    pub(crate) surface: String,
}

/// At most this many candidates are filled from one item, however many values
/// its placeholders' functions give.
const MAX_FILLED: usize = 64;

/// One call of a function, to fill one placeholder.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Call<'a> {
    pub name: &'a str,
    pub source: &'a str,
    pub argument: Option<&'a str>,
}

/// What a placeholder hands its function.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Position {
    /// The reading's next number, counting only placeholders that name none.
    Next,
    /// The reading's number at this index, from 0.
    At(usize),
    /// The whole reading.
    Reading,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Placeholder<'a> {
    pub(crate) position: Position,
    pub(crate) name: &'a str,
    pub(crate) argument: Option<&'a str>,
}

impl<'a> Placeholder<'a> {
    /// `inside` is what is between the braces, unescaped. `None` when it is
    /// not written as a placeholder may be.
    pub(crate) fn parse(inside: &'a str) -> Option<Self> {
        let head = inside.find(' ').map_or(inside, |space| &inside[..space]);
        let (position, rest) = match head.find(':') {
            Some(colon) => (Position::parse(&inside[..colon])?, &inside[colon + 1..]),
            None => (Position::Next, inside),
        };
        let (name, argument) = match rest.split_once(' ') {
            Some((name, argument)) => (name, Some(argument)),
            None => (rest, None),
        };
        if name.contains([':', '{', '}', '\\', OPEN, CLOSE]) {
            return None;
        }
        Some(Self {
            position,
            name,
            argument,
        })
    }
}

impl Position {
    fn parse(text: &str) -> Option<Self> {
        match text {
            "" => Some(Self::Next),
            "-" => Some(Self::Reading),
            _ if text.bytes().all(|b| b.is_ascii_digit()) => text.parse().ok().map(Self::At),
            _ => None,
        }
    }
}

/// Each placeholder of `surface`, in order; `None` when one is not written as
/// a placeholder may be.
pub(crate) fn placeholders(surface: &str) -> Option<Vec<Placeholder<'_>>> {
    let mut found = Vec::new();
    let mut rest = surface;
    while let Some(open) = rest.find(OPEN) {
        let inside = &rest[open + OPEN.len_utf8()..];
        let close = inside.find(CLOSE)?;
        found.push(Placeholder::parse(&inside[..close])?);
        rest = &inside[close + CLOSE.len_utf8()..];
    }
    Some(found)
}

/// Whether every placeholder of `surface` has what it takes from a reading
/// with `numbers` numbers.
pub(crate) fn fits(surface: &str, numbers: usize) -> bool {
    let Some(placeholders) = placeholders(surface) else {
        return false;
    };
    let mut next = 0;
    placeholders.iter().all(|p| match p.position {
        Position::Next => {
            next += 1;
            next <= numbers
        }
        Position::At(at) => at < numbers,
        Position::Reading => true,
    })
}

/// `surface` filled with the first value of each placeholder's function; see
/// [`fill_all`].
pub(crate) fn fill(
    surface: &str,
    numbers: &[String],
    reading: &str,
    functions: Option<&dyn Functions>,
) -> Option<String> {
    fill_all(surface, numbers, reading, functions)
        .into_iter()
        .next()
        .map(|filled| filled.text)
}

/// `surface` filled with each value of each placeholder's function, given
/// what it takes from `reading` and its `numbers`, as typed: every
/// combination, the first placeholder's values varying slowest. None when a
/// placeholder has nothing to take, or its function is missing or gives no
/// value.
pub(crate) fn fill_all(
    surface: &str,
    numbers: &[String],
    reading: &str,
    functions: Option<&dyn Functions>,
) -> Vec<Filled> {
    let mut filled = vec![Filled {
        text: String::new(),
        surface: String::new(),
    }];
    let mut next = numbers.iter();
    let mut rest = surface;
    while let Some(open) = rest.find(OPEN) {
        let before = &rest[..open];
        let inside = &rest[open + OPEN.len_utf8()..];
        let Some(close) = inside.find(CLOSE) else {
            return Vec::new();
        };
        let written = &inside[..close];
        let Some(placeholder) = Placeholder::parse(written) else {
            return Vec::new();
        };
        let source = match placeholder.position {
            Position::Next => next.next().map(String::as_str),
            Position::At(at) => numbers.get(at).map(String::as_str),
            Position::Reading => Some(reading),
        };
        let Some(source) = source else {
            return Vec::new();
        };
        let call = Call {
            name: placeholder.name,
            source,
            argument: placeholder.argument,
        };
        let values = match functions {
            Some(functions) if functions.has(call.name) => {
                functions.call(&call).unwrap_or_default()
            }
            _ if call.name.is_empty() => vec![call.source.to_owned().into()],
            _ => Vec::new(),
        };
        // One value is recorded by the item as written, as before functions
        // could give more.
        let several = values.len() > 1;
        filled = filled
            .into_iter()
            .flat_map(|f| {
                values.iter().map(move |value| {
                    let called = match &value.argument {
                        Some(argument) if several => placeholder.called_with(argument),
                        _ => written.to_owned(),
                    };
                    Filled {
                        text: format!("{}{before}{}", f.text, value.text),
                        surface: format!("{}{before}{OPEN}{called}{CLOSE}", f.surface),
                    }
                })
            })
            .take(MAX_FILLED)
            .collect();
        if filled.is_empty() {
            return filled;
        }
        rest = &inside[close + CLOSE.len_utf8()..];
    }
    for f in &mut filled {
        f.text.push_str(rest);
        f.surface.push_str(rest);
    }
    filled
}

impl Placeholder<'_> {
    /// The placeholder written to call its function with `argument` instead.
    fn called_with(&self, argument: &str) -> String {
        let position = match self.position {
            Position::Next => String::new(),
            Position::At(at) => format!("{at}:"),
            Position::Reading => "-:".to_owned(),
        };
        format!("{position}{} {argument}", self.name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn marked(inside: &str) -> String {
        format!("{OPEN}{inside}{CLOSE}")
    }

    #[test]
    fn a_position_and_a_function_are_split_at_the_colon() {
        let parsed = |inside| Placeholder::parse(inside).unwrap();
        assert_eq!(
            parsed("1:kanji"),
            Placeholder {
                position: Position::At(1),
                name: "kanji",
                argument: None
            }
        );
        assert_eq!(parsed(":kanji").position, Position::Next);
        assert_eq!(parsed("-:date").position, Position::Reading);
        assert_eq!(parsed("1:").name, "");
    }

    #[test]
    fn without_a_colon_the_whole_is_the_function() {
        let one = Placeholder::parse("1").unwrap();
        assert_eq!((one.position, one.name), (Position::Next, "1"));
        assert_eq!(Placeholder::parse("").unwrap().name, "");
    }

    #[test]
    fn the_argument_is_everything_past_the_first_space() {
        let date = Placeholder::parse("-:date %H:%M %S").unwrap();
        assert_eq!((date.name, date.argument), ("date", Some("%H:%M %S")));
        let plain = Placeholder::parse("date %Y:%m").unwrap();
        assert_eq!(
            (plain.position, plain.name, plain.argument),
            (Position::Next, "date", Some("%Y:%m"))
        );
        assert_eq!(Placeholder::parse("date ").unwrap().argument, Some(""));
    }

    #[test]
    fn a_position_must_be_digits_a_dash_or_nothing() {
        assert_eq!(Placeholder::parse("x:kanji"), None);
        assert_eq!(Placeholder::parse("1-:kanji"), None);
        assert_eq!(Placeholder::parse("a:b:c"), None);
    }

    #[test]
    fn a_placeholder_fits_when_the_reading_has_its_numbers() {
        let surface = format!("{}{}", marked("kanji"), marked("1:"));
        assert!(fits(&surface, 2));
        assert!(!fits(&surface, 1));
        assert!(!fits(&marked("kanji"), 0));
        assert!(fits(&marked("-:date"), 0));
        assert!(fits("個", 0));
    }

    #[test]
    fn placeholders_without_a_position_take_the_numbers_in_turn() {
        let surface = format!("{}月{}日{}", marked("1:"), marked("shout"), marked(""));
        let numbers = ["12".to_owned(), "1".to_owned()];
        assert_eq!(
            fill(&surface, &numbers, "", Some(&Shout)).as_deref(),
            Some("1月12!日1")
        );
    }

    #[test]
    fn the_empty_function_gives_the_number_as_typed() {
        let numbers = ["１２".to_owned()];
        assert_eq!(
            fill(&marked(""), &numbers, "", None).as_deref(),
            Some("１２")
        );
    }

    #[test]
    fn a_dash_hands_the_whole_reading() {
        assert_eq!(
            fill(&format!("「{}」", marked("-:")), &[], "きょう", None).as_deref(),
            Some("「きょう」")
        );
    }

    #[test]
    fn nothing_is_filled_without_a_number_or_a_function() {
        assert_eq!(fill(&marked("kanji"), &[], "", None), None);
        assert_eq!(fill(&marked("3:"), &["1".to_owned()], "", None), None);
        assert_eq!(fill(&marked("-:missing"), &[], "よみ", None), None);
        assert_eq!(fill(&marked("-:kanji"), &[], "よみ", None), None);
    }

    struct Shout;

    impl Functions for Shout {
        fn has(&self, name: &str) -> bool {
            matches!(name, "shout" | "kanji")
        }

        fn call(&self, call: &Call) -> Option<Vec<Value>> {
            match call.name {
                "shout" => Some(vec![
                    format!("{}{}", call.source, call.argument.unwrap_or("!")).into(),
                ]),
                _ => None,
            }
        }
    }

    #[test]
    fn a_user_function_takes_its_source_and_argument() {
        let shout = |surface: &str| fill(surface, &["1".to_owned()], "よみ", Some(&Shout));
        assert_eq!(shout(&marked("-:shout")).as_deref(), Some("よみ!"));
        assert_eq!(shout(&marked("shout ?!")).as_deref(), Some("1?!"));
    }

    #[test]
    fn the_engine_has_no_function_but_the_empty_one() {
        let numbers = ["1".to_owned()];
        assert_eq!(fill(&marked("kanji"), &numbers, "", None), None);
        assert_eq!(fill(&marked("wide-num"), &numbers, "", Some(&Shout)), None);
        assert_eq!(
            fill(&marked(""), &numbers, "", Some(&Shout)).as_deref(),
            Some("1")
        );
    }

    #[test]
    fn a_surface_without_placeholders_stays_as_written() {
        assert_eq!(fill("{個}", &[], "", None).as_deref(), Some("{個}"));
    }

    /// `ways` gives its source in each way its argument names, `a` and `b`,
    /// each with its way as the argument that gives it alone; `both` gives the
    /// two with no argument.
    struct Ways;

    impl Functions for Ways {
        fn has(&self, name: &str) -> bool {
            matches!(name, "ways" | "both")
        }

        fn call(&self, call: &Call) -> Option<Vec<Value>> {
            let ways = call.argument.unwrap_or("a b");
            let values = ways.split(' ').map(|way| Value {
                text: format!("{}{way}", call.source),
                argument: (call.name == "ways").then(|| way.to_owned()),
            });
            Some(values.collect())
        }
    }

    #[test]
    fn each_value_a_function_gives_fills_a_candidate_of_its_own() {
        let filled = fill_all(
            &format!("{}個", marked("ways")),
            &["1".to_owned()],
            "",
            Some(&Ways),
        );
        let texts: Vec<_> = filled.iter().map(|f| f.text.as_str()).collect();
        assert_eq!(texts, ["1a個", "1b個"]);
    }

    #[test]
    fn a_value_is_recorded_by_the_placeholder_called_with_its_argument() {
        let surface = format!("{}個{}", marked("ways"), marked("-:ways b"));
        let filled = fill_all(&surface, &["1".to_owned()], "よみ", Some(&Ways));
        let recorded: Vec<_> = filled.iter().map(|f| f.surface.as_str()).collect();
        let a = format!("{}個{}", marked("ways a"), marked("-:ways b"));
        let b = format!("{}個{}", marked("ways b"), marked("-:ways b"));
        assert_eq!(recorded, [a, b]);
    }

    #[test]
    fn a_value_without_an_argument_is_recorded_by_its_item() {
        let surface = marked("both");
        let filled = fill_all(&surface, &["1".to_owned()], "", Some(&Ways));
        assert_eq!(filled.len(), 2);
        assert!(filled.iter().all(|f| f.surface == surface));
    }

    #[test]
    fn two_placeholders_of_several_values_give_every_pair() {
        let surface = format!("{}月{}日", marked("ways"), marked("ways"));
        let numbers = ["1".to_owned(), "2".to_owned()];
        let texts: Vec<_> = fill_all(&surface, &numbers, "", Some(&Ways))
            .into_iter()
            .map(|f| f.text)
            .collect();
        assert_eq!(texts, ["1a月2a日", "1a月2b日", "1b月2a日", "1b月2b日"]);
    }

    #[test]
    fn a_position_is_kept_where_the_placeholder_is_called_again() {
        let filled = fill_all(
            &marked("1:ways"),
            &["1".to_owned(), "2".to_owned()],
            "",
            Some(&Ways),
        );
        assert_eq!(filled[0].surface, marked("1:ways a"));
    }

    #[test]
    fn a_single_value_is_recorded_by_its_item_as_written() {
        let surface = marked(":ways a");
        let filled = fill_all(&surface, &["1".to_owned()], "", Some(&Ways));
        assert_eq!(filled[0].surface, surface);
    }

    #[test]
    fn a_function_giving_no_value_fills_nothing() {
        struct Empty;
        impl Functions for Empty {
            fn has(&self, _: &str) -> bool {
                true
            }
            fn call(&self, _: &Call) -> Option<Vec<Value>> {
                Some(Vec::new())
            }
        }
        assert_eq!(
            fill_all(&marked("x"), &["1".to_owned()], "", Some(&Empty)),
            []
        );
    }
}
