//! Where a popup goes on the screen, beside the text it is about.

/// Where a popup of `size` goes by `at`, a screen rectangle of the text: just
/// below it, or above it when there is no room below, and moved in from the
/// edges of `work`, the part of the screen windows may use.
pub(crate) fn place(at: Rect, size: (i32, i32), gap: i32, work: Rect) -> (i32, i32) {
    let (width, height) = size;
    let below = at.bottom + gap;
    let y = if below + height <= work.bottom {
        below
    } else {
        at.top - gap - height
    };
    let x = at.left.min(work.right - width).max(work.left);
    (x, y.max(work.top))
}

/// Where a popup of `size` goes by `at` to stay off the line below the
/// text, where [`place`] puts another: just above it, or below it when there
/// is no room above.
pub(crate) fn place_above(at: Rect, size: (i32, i32), gap: i32, work: Rect) -> (i32, i32) {
    let (width, height) = size;
    let above = at.top - gap - height;
    let y = if above >= work.top {
        above
    } else {
        at.bottom + gap
    };
    let x = at.left.min(work.right - width).max(work.left);
    (x, y)
}

/// A rectangle by its edges, as Windows gives one.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCREEN: Rect = Rect {
        left: 0,
        top: 0,
        right: 1920,
        bottom: 1040,
    };

    fn text(left: i32, top: i32) -> Rect {
        Rect {
            left,
            top,
            right: left + 40,
            bottom: top + 20,
        }
    }

    #[test]
    fn a_popup_goes_just_below_the_text() {
        assert_eq!(place(text(100, 200), (300, 100), 2, SCREEN), (100, 222));
    }

    #[test]
    fn a_popup_by_the_right_edge_moves_in() {
        assert_eq!(place(text(1850, 200), (300, 100), 2, SCREEN).0, 1620);
    }

    #[test]
    fn a_popup_without_room_below_goes_above() {
        assert_eq!(place(text(100, 1000), (300, 100), 2, SCREEN), (100, 898));
    }

    #[test]
    fn a_popup_kept_off_the_line_below_goes_just_above_the_text() {
        assert_eq!(
            place_above(text(100, 200), (300, 30), 2, SCREEN),
            (100, 168)
        );
    }

    #[test]
    fn a_popup_without_room_above_goes_below() {
        assert_eq!(place_above(text(100, 10), (300, 30), 2, SCREEN), (100, 32));
    }
}
