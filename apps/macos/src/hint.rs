//! Shows, by the caret, that a word is being registered and the key that
//! pastes into it, for as long as the text to register is typed.

use std::cell::RefCell;

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{MainThreadMarker, MainThreadOnly, msg_send};
use objc2_app_kit::{
    NSBackingStoreType, NSBox, NSBoxType, NSColor, NSFont, NSPanel, NSTextField, NSTitlePosition,
    NSWindowStyleMask,
};
use objc2_foundation::{NSDictionary, NSPoint, NSRect, NSSize, NSString};

use crate::candidate_window;
use crate::candidates::{Size, place_above};

const PADDING: NSSize = NSSize::new(10.0, 5.0);
// The keycap's colour of the logo, as the ABC mode indicator.
const COLOR: [u8; 3] = [0x1D, 0x24, 0x33];
// NSPopUpMenuWindowLevel: above ordinary windows, like a candidate panel.
const LEVEL: isize = 101;

thread_local! {
    static PANEL: RefCell<Option<Hint>> = const { RefCell::new(None) };
}

struct Hint {
    panel: Retained<NSPanel>,
    background: Retained<NSBox>,
    text: Retained<NSTextField>,
    shown: String,
}

/// The caret's line rectangle in screen coordinates, if the client reports one.
fn caret_rect(client: &AnyObject) -> Option<NSRect> {
    let mut rect = NSRect::ZERO;
    let _: Option<Retained<NSDictionary>> = unsafe {
        msg_send![client, attributesForCharacterIndex: 0usize, lineHeightRectangle: &mut rect]
    };
    (rect.size.height > 0.0).then_some(rect)
}

fn create(mtm: MainThreadMarker) -> Hint {
    let style = NSWindowStyleMask::Borderless | NSWindowStyleMask::NonactivatingPanel;
    let panel: Retained<NSPanel> = unsafe {
        msg_send![
            NSPanel::alloc(mtm),
            initWithContentRect: NSRect::ZERO,
            styleMask: style,
            backing: NSBackingStoreType::Buffered,
            defer: false
        ]
    };
    panel.setLevel(LEVEL);
    panel.setIgnoresMouseEvents(true);
    panel.setHasShadow(true);
    // Transparent, so that only the rounded box shows.
    panel.setOpaque(false);
    panel.setBackgroundColor(Some(&NSColor::clearColor()));
    unsafe { panel.setReleasedWhenClosed(false) };

    let background = NSBox::initWithFrame(NSBox::alloc(mtm), NSRect::ZERO);
    background.setBoxType(NSBoxType::Custom);
    background.setTitlePosition(NSTitlePosition::NoTitle);
    background.setBorderWidth(0.0);
    background.setCornerRadius(7.0);
    background.setContentViewMargins(NSSize::ZERO);
    let [r, g, b] = COLOR;
    let channel = |c: u8| f64::from(c) / 255.0;
    background.setFillColor(&NSColor::colorWithSRGBRed_green_blue_alpha(
        channel(r),
        channel(g),
        channel(b),
        0.92,
    ));

    let text = NSTextField::labelWithString(&NSString::from_str(""), mtm);
    text.setFont(Some(&NSFont::systemFontOfSize(12.5)));
    text.setTextColor(Some(&NSColor::whiteColor()));

    if let Some(view) = panel.contentView() {
        view.addSubview(&background);
        view.addSubview(&text);
    }
    Hint {
        panel,
        background,
        text,
        shown: String::new(),
    }
}

/// Shows `message` by the caret, or moves it there again.
pub fn show(message: &str, client: &AnyObject) {
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    let Some(caret) = caret_rect(client) else {
        return hide();
    };
    PANEL.with_borrow_mut(|slot| {
        let hint = slot.get_or_insert_with(|| create(mtm));
        if hint.shown != message {
            hint.text.setStringValue(&NSString::from_str(message));
            hint.text.sizeToFit();
            hint.shown = message.to_owned();
        }
        let label = hint.text.frame().size;
        let size = NSSize::new(
            label.width + 2.0 * PADDING.width,
            label.height + 2.0 * PADDING.height,
        );
        hint.background.setFrame(NSRect::new(NSPoint::ZERO, size));
        hint.text
            .setFrameOrigin(NSPoint::new(PADDING.width, PADDING.height));
        let line = candidate_window::rect(caret);
        let (x, y) = place_above(
            line,
            Size {
                width: size.width,
                height: size.height,
            },
            candidate_window::screen_of(line, mtm),
        );
        hint.panel
            .setFrame_display(NSRect::new(NSPoint::new(x, y), size), true);
        hint.panel.orderFrontRegardless();
    });
}

pub fn hide() {
    PANEL.with_borrow(|slot| {
        if let Some(hint) = slot {
            hint.panel.orderOut(None);
        }
    });
}
