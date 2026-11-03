//! What makes the DLL a COM server TSF can find: the class factory, the
//! exports COM calls, and the registration of the text service.

#![allow(non_snake_case)]

use std::ffi::{OsString, c_void};
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::path::PathBuf;
use std::sync::atomic::{AtomicIsize, Ordering};

use crate::bindings::*;
use windows_core::*;

use crate::registration::registered_dll;
use crate::tip::TextService;

pub const CLSID_KANAEMI: GUID = GUID::from_u128(0x04448b65_d29f_4f36_84c7_5fdba91ef4b9);
pub const PROFILE_KANAEMI: GUID = GUID::from_u128(0x33dc0029_2833_47af_b58e_3507230dc30a);
/// What the candidate list is, to an application that draws it itself.
pub const CANDIDATE_LIST_ELEMENT: GUID = GUID::from_u128(0x1915853b_4879_4e18_b9b3_465bbd7da083);
const LANGID_JAPANESE: LANGID = LANGID(0x0411);
/// What the text service is: a keyboard, and one that works in immersive
/// applications and shows in the system tray.
const CATEGORIES: [GUID; 3] = [
    GUID_TFCAT_TIP_KEYBOARD,
    GUID_TFCAT_TIPCAP_IMMERSIVESUPPORT,
    GUID_TFCAT_TIPCAP_SYSTRAYSUPPORT,
];
const NAME: &str = "かなえみ";

static MODULE: AtomicIsize = AtomicIsize::new(0);

/// The DLL's own module, which owns the window classes it registers.
pub fn instance() -> HINSTANCE {
    HINSTANCE(MODULE.load(Ordering::Relaxed) as *mut c_void)
}

#[implement(IClassFactory)]
struct Factory;

impl IClassFactory_Impl for Factory_Impl {
    fn CreateInstance(
        &self,
        outer: Ref<IUnknown>,
        riid: *const GUID,
        ppv: *mut *mut c_void,
    ) -> Result<()> {
        if outer.is_some() {
            return Err(CLASS_E_NOAGGREGATION.into());
        }
        let unknown: IUnknown = TextService::new().into();
        unsafe { unknown.query(riid, ppv).ok() }
    }

    fn LockServer(&self, _lock: BOOL) -> Result<()> {
        Ok(())
    }
}

#[unsafe(no_mangle)]
extern "system" fn DllMain(module: HINSTANCE, reason: u32, _: *mut c_void) -> BOOL {
    if reason == DLL_PROCESS_ATTACH as u32 {
        MODULE.store(module.0 as isize, Ordering::Relaxed);
    }
    true.into()
}

#[unsafe(no_mangle)]
extern "system" fn DllGetClassObject(
    rclsid: *const GUID,
    riid: *const GUID,
    ppv: *mut *mut c_void,
) -> HRESULT {
    if unsafe { *rclsid } != CLSID_KANAEMI {
        return CLASS_E_CLASSNOTAVAILABLE;
    }
    let factory: IClassFactory = Factory.into();
    unsafe { factory.query(riid, ppv) }
}

/// The DLL stays loaded: an application may activate the text service
/// again at any time, and its state lives in thread-locals.
#[unsafe(no_mangle)]
extern "system" fn DllCanUnloadNow() -> HRESULT {
    S_FALSE
}

#[unsafe(no_mangle)]
extern "system" fn DllRegisterServer() -> HRESULT {
    match register() {
        Ok(()) => S_OK,
        Err(error) => error.code(),
    }
}

#[unsafe(no_mangle)]
extern "system" fn DllUnregisterServer() -> HRESULT {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED as u32);
        if let Ok(categories) =
            CoCreateInstance::<_, ITfCategoryMgr>(&CLSID_TF_CategoryMgr, None, CLSCTX_INPROC_SERVER)
        {
            for category in CATEGORIES {
                let _ = categories.UnregisterCategory(&CLSID_KANAEMI, &category, &CLSID_KANAEMI);
            }
        }
        // Takes the profiles with it, and what is left of the text service
        // under the TIP key.
        if let Ok(profiles) = CoCreateInstance::<_, ITfInputProcessorProfiles>(
            &CLSID_TF_InputProcessorProfiles,
            None,
            CLSCTX_INPROC_SERVER,
        ) {
            let _ = profiles.Unregister(&CLSID_KANAEMI);
        }
        let _ = RegDeleteTreeW(HKEY_LOCAL_MACHINE, &HSTRING::from(clsid_key()));
    }
    S_OK
}

fn clsid_key() -> String {
    format!(r"Software\Classes\CLSID\{{{CLSID_KANAEMI:?}}}")
}

/// Keeps the DLL loaded until the process ends. COM unloads it when the
/// application uninitializes COM, whatever `DllCanUnloadNow` answers, and a
/// thread still running its code would then crash the application.
pub(crate) fn pin() -> Result<()> {
    let mut module = HMODULE::default();
    // The module's base address lies within the module.
    unsafe {
        GetModuleHandleExW(
            (GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_PIN) as u32,
            PCWSTR(instance().0 as *const u16),
            &mut module,
        )
    }
    .ok()
}

/// Where the DLL itself is, to find what is installed beside it.
pub(crate) fn module_path() -> Option<std::path::PathBuf> {
    let wide = module_wide_path();
    (!wide.is_empty()).then(|| std::path::PathBuf::from(String::from_utf16_lossy(&wide)))
}

fn module_wide_path() -> Vec<u16> {
    let mut buffer = vec![0u16; 1024];
    let len = unsafe {
        GetModuleFileNameW(
            Some(instance()),
            PWSTR(buffer.as_mut_ptr()),
            buffer.len() as u32,
        )
    };
    buffer.truncate(len as usize);
    buffer
}

fn set_value(key: HKEY, name: PCWSTR, value: &[u16]) -> Result<()> {
    let mut data = value.to_vec();
    data.push(0);
    let bytes = unsafe { std::slice::from_raw_parts(data.as_ptr().cast::<u8>(), data.len() * 2) };
    status(unsafe { RegSetValueExW(key, name, None, REG_SZ, Some(bytes)) })
}

/// The registry's answer, as an error unless it succeeded.
fn status(status: LSTATUS) -> Result<()> {
    if status.0 == 0 {
        Ok(())
    } else {
        // `HRESULT_FROM_WIN32`: windows-core converts no Win32 error code.
        Err(HRESULT((status.0 as u32 & 0xffff | 0x8007_0000) as i32).into())
    }
}

fn register() -> Result<()> {
    let mut key = HKEY::default();
    status(unsafe {
        RegCreateKeyExW(
            HKEY_LOCAL_MACHINE,
            &HSTRING::from(format!(r"{}\InprocServer32", clsid_key())),
            None,
            None,
            REG_OPTION_NON_VOLATILE as u32,
            ACCESS_MASK(KEY_WRITE as u32),
            None,
            &mut key,
            None,
        )
    })?;
    let dll: Vec<u16> = {
        let module = PathBuf::from(OsString::from_wide(&module_wide_path()));
        registered_dll(&module).as_os_str().encode_wide().collect()
    };
    let written = set_value(key, PCWSTR::null(), &dll).and_then(|()| {
        let model: Vec<u16> = "Apartment".encode_utf16().collect();
        set_value(key, w!("ThreadingModel"), &model)
    });
    unsafe {
        let _ = RegCloseKey(key);
    }
    written?;
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED as u32);
        let profiles: ITfInputProcessorProfileMgr =
            CoCreateInstance(&CLSID_TF_InputProcessorProfiles, None, CLSCTX_INPROC_SERVER)?;
        let name: Vec<u16> = NAME.encode_utf16().collect();
        profiles
            .RegisterProfile(
                &CLSID_KANAEMI,
                LANGID_JAPANESE,
                &PROFILE_KANAEMI,
                name.as_ptr(),
                name.len() as u32,
                std::ptr::null(),
                0,
                0,
                HKL::default(),
                0,
                true,
                0,
            )
            .ok()?;
        let categories: ITfCategoryMgr =
            CoCreateInstance(&CLSID_TF_CategoryMgr, None, CLSCTX_INPROC_SERVER)?;
        for category in CATEGORIES {
            categories
                .RegisterCategory(&CLSID_KANAEMI, &category, &CLSID_KANAEMI)
                .ok()?;
        }
    }
    Ok(())
}
