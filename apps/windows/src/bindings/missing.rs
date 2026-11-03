//! What the input method uses from the Text Services Framework but the
//! metadata `windows-bindgen` reads does not carry: the headers it was
//! scraped from leave out the category and class GUIDs, `ctffunc.h`,
//! `inputscope.h`, the macros of `handleapi.h` and the key codes of
//! `ime.h`. Declared here from the Windows SDK headers until the metadata
//! covers them.

use super::{HANDLE, HWND, ITfFunction, ITfFunction_Impl};
use windows_core::{GUID, HRESULT, IUnknown, interface};

pub const CLSID_TF_CategoryMgr: GUID = GUID::from_u128(0xa4b544a1_438d_4b41_9325_869523e2d6c7);
pub const CLSID_TF_InputProcessorProfiles: GUID =
    GUID::from_u128(0x33c53a50_f456_4884_b049_85fd643ecfed);
pub const GUID_PROP_INPUTSCOPE: GUID = GUID::from_u128(0x1713dd5a_68e7_4a5b_9af6_592a595c778d);
pub const GUID_TFCAT_TIP_KEYBOARD: GUID = GUID::from_u128(0x34745c63_b2f0_4784_8b67_5e12c8701a31);
pub const GUID_TFCAT_TIPCAP_IMMERSIVESUPPORT: GUID =
    GUID::from_u128(0x13a016df_560b_46cd_947a_4c3af1e0e35d);
pub const GUID_TFCAT_TIPCAP_SYSTRAYSUPPORT: GUID =
    GUID::from_u128(0x25504fb4_7bab_4bc1_9c69_cf81890f0ef5);

pub type InputScope = i32;
pub const IS_PASSWORD: InputScope = 31;
pub const IS_PRIVATE: InputScope = 61;
pub const IS_NUMERIC_PASSWORD: InputScope = 63;
pub const IS_NUMERIC_PIN: InputScope = 64;
pub const IS_ALPHANUMERIC_PIN: InputScope = 65;
pub const IS_ALPHANUMERIC_PIN_SET: InputScope = 66;

/// From `handleapi.h`, where it is a macro.
pub const INVALID_HANDLE_VALUE: HANDLE = HANDLE(-1isize as *mut core::ffi::c_void);

/// From `ime.h`.
pub const VK_DBE_HIRAGANA: i32 = 0xf2;

#[interface("88f567c6-1757-49f8-a1b2-89234c1eeff9")]
pub unsafe trait ITfFnConfigure: ITfFunction {
    pub fn Show(&self, hwndparent: HWND, langid: u16, rguidprofile: *const GUID) -> HRESULT;
}

/// Only `GetInputScopes` is called; the rest are declared to keep the
/// vtable's layout.
#[interface("fde1eaee-6924-4cdf-91e7-da38cff5559d")]
pub unsafe trait ITfInputScope: IUnknown {
    pub fn GetInputScopes(
        &self,
        pprginputscopes: *mut *mut InputScope,
        pccount: *mut u32,
    ) -> HRESULT;
    pub fn GetPhrase(
        &self,
        ppbstrphrases: *mut *mut *mut core::ffi::c_void,
        pccount: *mut u32,
    ) -> HRESULT;
    pub fn GetRegularExpression(&self, pbstrregexp: *mut *mut core::ffi::c_void) -> HRESULT;
    pub fn GetSRGS(&self, pbstrsrgs: *mut *mut core::ffi::c_void) -> HRESULT;
    pub fn GetXML(&self, pbstrxml: *mut *mut core::ffi::c_void) -> HRESULT;
}
