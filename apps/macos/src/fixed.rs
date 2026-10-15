//! Marked text an application fixed as it was shown. Asked to commit the
//! composition, some applications, TextEdit among them, first turn the marked
//! text into text and drop what the IME commits, so the marks would stay in
//! the document. Once the application is done, the IME puts what it committed
//! in place of that text.

/// Whether `found`, the text now where `shown` was marked, is that marked
/// text left as it was: no longer marked, and not replaced by what was
/// committed.
pub fn left_as_shown(shown: &str, found: &str, still_marked: bool) -> bool {
    !still_marked && !shown.is_empty() && found == shown
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marked_text_turned_into_text_as_it_was_is_left_as_shown() {
        assert!(left_as_shown("»けいやく « ", "»けいやく « ", false));
    }

    #[test]
    fn marked_text_replaced_by_what_was_committed_is_not() {
        assert!(!left_as_shown("»けいやく « ", "けいやく", false));
        assert!(!left_as_shown("»けいやく « ", "けいやく契約", false));
    }

    #[test]
    fn text_still_marked_is_the_application_s_to_keep() {
        assert!(!left_as_shown("»けいやく « ", "»けいやく « ", true));
    }

    #[test]
    fn nothing_marked_leaves_nothing() {
        assert!(!left_as_shown("", "", false));
    }
}
