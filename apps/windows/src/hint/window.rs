use std::cell::{Cell, RefCell};

use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::*;

use crate::listing::PADDING;
use crate::popup::{self, Popup};

const CLASS: PCWSTR = w!("KanaemiRegistrationHint");

thread_local! {
    static POPUP: Popup = const { Popup::new() };
    static SHOWN: RefCell<Vec<u16>> = const { RefCell::new(Vec::new()) };
    /// Whether the hint is to show while the application is in front.
    static WANTED: Cell<bool> = const { Cell::new(false) };
}

/// Shows `text` above `at`, a screen rectangle of the composition, until
/// it is hidden.
pub(crate) fn show(text: &str, at: RECT, owner: Option<HWND>) {
    let Some(window) = POPUP.with(|popup| popup.owned_by(owner, create)) else {
        return;
    };
    SHOWN.set(text.encode_utf16().collect());
    WANTED.set(true);
    let (width, height) = size(window);
    let (left, top) = popup::position_above(at, (width, height), 2);
    unsafe {
        let _ = SetWindowPos(
            window,
            Some(HWND_TOPMOST),
            left,
            top,
            width,
            height,
            SWP_NOACTIVATE | SWP_SHOWWINDOW,
        );
        let _ = InvalidateRect(Some(window), None, true);
    }
}

pub(crate) fn hide() {
    WANTED.set(false);
    conceal();
}

/// Hides the hint while the application is in the background, to show
/// again with [`reveal`] as it comes back.
pub(crate) fn conceal() {
    if let Some(window) = POPUP.with(Popup::existing) {
        unsafe {
            let _ = ShowWindow(window, SW_HIDE);
        }
    }
}

pub(crate) fn reveal() {
    if WANTED.get()
        && let Some(window) = POPUP.with(Popup::existing)
    {
        unsafe {
            let _ = ShowWindow(window, SW_SHOWNOACTIVATE);
        }
    }
}

fn create(owner: Option<HWND>) -> Result<HWND> {
    let instance = crate::com::instance();
    let class = WNDCLASSW {
        lpfnWndProc: Some(procedure),
        hInstance: instance,
        lpszClassName: CLASS,
        ..Default::default()
    };
    // A second registration in the same process fails harmlessly.
    unsafe { RegisterClassW(&class) };
    unsafe {
        CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TRANSPARENT,
            CLASS,
            PCWSTR::null(),
            WS_POPUP | WS_BORDER,
            0,
            0,
            0,
            0,
            owner,
            None,
            Some(instance),
            None,
        )
    }
}

fn size(window: HWND) -> (i32, i32) {
    let dc = unsafe { GetDC(Some(window)) };
    let previous = unsafe { SelectObject(dc, crate::candidates::font().into()) };
    let mut size = SIZE::default();
    SHOWN.with_borrow(|text| unsafe {
        let _ = GetTextExtentPoint32W(dc, text, &mut size);
    });
    unsafe {
        SelectObject(dc, previous);
        ReleaseDC(Some(window), dc);
    }
    let border = 2;
    (size.cx + PADDING * 2 + border, size.cy + PADDING + border)
}

fn paint(window: HWND) {
    let mut ps = PAINTSTRUCT::default();
    let dc = unsafe { BeginPaint(window, &mut ps) };
    let previous = unsafe { SelectObject(dc, crate::candidates::font().into()) };
    let mut client = RECT::default();
    unsafe {
        let _ = GetClientRect(window, &mut client);
        FillRect(dc, &client, GetSysColorBrush(COLOR_WINDOW));
        SetBkMode(dc, TRANSPARENT);
        SetTextColor(dc, COLORREF(GetSysColor(COLOR_WINDOWTEXT)));
    }
    let mut text = SHOWN.with_borrow(Vec::clone);
    unsafe {
        DrawTextW(
            dc,
            &mut text,
            &mut client,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX,
        );
        SelectObject(dc, previous);
        let _ = EndPaint(window, &ps);
    }
}

extern "system" fn procedure(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        WM_PAINT => {
            // A panic must not unwind into the application's message loop.
            let _ = std::panic::catch_unwind(|| paint(window));
            LRESULT(0)
        }
        WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),
        _ => unsafe { DefWindowProcW(window, message, wparam, lparam) },
    }
}
