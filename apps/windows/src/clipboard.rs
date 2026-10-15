//! The text on the clipboard, for the key that pastes into the text to
//! register.

use windows::Win32::Foundation::HGLOBAL;
use windows::Win32::System::DataExchange::{CloseClipboard, GetClipboardData, OpenClipboard};
use windows::Win32::System::Memory::{GlobalLock, GlobalSize, GlobalUnlock};
use windows::Win32::System::Ole::CF_UNICODETEXT;

/// `None` when the clipboard holds no text, or another program has it open.
pub(crate) fn read() -> Option<String> {
    unsafe { OpenClipboard(None) }.ok()?;
    let text = read_open();
    let _ = unsafe { CloseClipboard() };
    text
}

fn read_open() -> Option<String> {
    let handle = unsafe { GetClipboardData(u32::from(CF_UNICODETEXT.0)) }.ok()?;
    let memory = HGLOBAL(handle.0);
    let start = unsafe { GlobalLock(memory) } as *const u16;
    if start.is_null() {
        return None;
    }
    // Another program put the data there: it is read no further than the
    // block holds, whether or not it ends with the NUL it should.
    let units = unsafe { GlobalSize(memory) } / std::mem::size_of::<u16>();
    let held = unsafe { std::slice::from_raw_parts(start, units) };
    let text = terminated(held).map(String::from_utf16_lossy);
    let _ = unsafe { GlobalUnlock(memory) };
    text
}

/// The text of `units` before its NUL; `None` without one.
fn terminated(units: &[u16]) -> Option<&[u16]> {
    units.iter().position(|&u| u == 0).map(|end| &units[..end])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_text_ends_at_its_nul_and_text_without_one_is_refused() {
        let text: Vec<u16> = "漢字\0x".encode_utf16().collect();
        assert_eq!(terminated(&text), Some(&text[..2]));
        let unterminated: Vec<u16> = "漢字".encode_utf16().collect();
        assert_eq!(terminated(&unterminated), None);
    }
}
