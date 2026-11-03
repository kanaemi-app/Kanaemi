//! Loads a DLL as COM loads a text service, and prints each named export with
//! the file it runs in, one per line:
//!
//!     kanaemi-load <dll> <export>...
//!
//! The packaging checks with it that the ARM64X kanaemi.dll hands each kind
//! of process the DLL built for it.

#[cfg(not(windows))]
fn main() {
    eprintln!("kanaemi-load runs only on Windows");
    std::process::exit(1);
}

#[cfg(windows)]
fn main() {
    use std::ffi::CString;

    use kanaemi::bindings::*;
    use windows_core::{Error, HSTRING, PCSTR, PCWSTR, PWSTR};

    let mut args = std::env::args().skip(1);
    let Some(dll) = args.next() else {
        fail("usage: kanaemi-load <dll> <export>...");
    };
    // COM loads an in-process server by its full path, looking for what the
    // server needs in the server's folder.
    let module = unsafe {
        LoadLibraryExW(
            &HSTRING::from(dll.as_str()),
            None,
            LOAD_WITH_ALTERED_SEARCH_PATH as u32,
        )
    };
    if module.0.is_null() {
        fail(&format!("{dll}: {}", Error::from_thread()));
    }
    for name in args {
        let symbol = CString::new(name.as_str()).unwrap_or_else(|_| fail(&name));
        let Some(export) = (unsafe { GetProcAddress(module, PCSTR(symbol.as_ptr().cast())) })
        else {
            fail(&format!("{name}: not exported"));
        };
        let mut owner = HMODULE::default();
        unsafe {
            GetModuleHandleExW(
                (GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS
                    | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT) as u32,
                PCWSTR(export as *const u16),
                &mut owner,
            )
        }
        .ok()
        .unwrap_or_else(|error| fail(&format!("{name}: {error}")));
        let mut path = vec![0u16; 1024];
        let len =
            unsafe { GetModuleFileNameW(Some(owner), PWSTR(path.as_mut_ptr()), path.len() as u32) };
        println!("{name} {}", String::from_utf16_lossy(&path[..len as usize]));
    }
}

#[cfg(windows)]
fn fail(message: &str) -> ! {
    eprintln!("{message}");
    std::process::exit(1);
}
