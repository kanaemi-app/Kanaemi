//! What a field's input scopes say about recording what is typed there.
//! Applications tell TSF the kind of text a field takes as input scopes,
//! such as `IS_PRIVATE` in a browser's InPrivate window.

/// `IS_PASSWORD`, `IS_NUMERIC_PASSWORD`, `IS_NUMERIC_PIN`,
/// `IS_ALPHANUMERIC_PIN` and `IS_ALPHANUMERIC_PIN_SET`: a field that takes
/// a secret.
const SECRET: [i32; 5] = [31, 63, 64, 65, 66];
/// `IS_PRIVATE`: a field that asks that what is typed there not be
/// remembered, such as one in a browser's InPrivate window.
const PRIVATE: i32 = 61;

/// Whether a field with `scopes` asks not to be recorded: one that takes a
/// secret, or says it is private.
pub(crate) fn asks_for_no_record(scopes: &[i32]) -> bool {
    scopes
        .iter()
        .any(|scope| *scope == PRIVATE || SECRET.contains(scope))
}

#[cfg(windows)]
mod read {
    use std::mem::{ManuallyDrop, take};

    use crate::bindings::*;
    use windows_core::*;

    // The values above are the SDK's.
    const _: () = {
        assert!(super::PRIVATE == IS_PRIVATE);
        assert!(super::SECRET[0] == IS_PASSWORD);
        assert!(super::SECRET[1] == IS_NUMERIC_PASSWORD);
        assert!(super::SECRET[2] == IS_NUMERIC_PIN);
        assert!(super::SECRET[3] == IS_ALPHANUMERIC_PIN);
        assert!(super::SECRET[4] == IS_ALPHANUMERIC_PIN_SET);
    };

    /// The input scopes of `context` at its selection, or at its start
    /// when it has none, read in the edit session of `ec`. A field that
    /// tells none has none.
    pub(crate) fn of(context: &ITfContext, ec: TfEditCookie) -> Vec<i32> {
        read(context, ec).unwrap_or_default()
    }

    fn read(context: &ITfContext, ec: TfEditCookie) -> Result<Vec<i32>> {
        let property = unsafe { context.GetAppProperty(&GUID_PROP_INPUTSCOPE)? };
        let mut selection = [TF_SELECTION::default()];
        let mut fetched = 0;
        let found = unsafe {
            context.GetSelection(
                ec,
                TF_DEFAULT_SELECTION,
                1,
                selection.as_mut_ptr(),
                &mut fetched,
            )
        };
        let selected = if found.is_ok() {
            ManuallyDrop::into_inner(take(&mut selection[0].range))
        } else {
            None
        };
        let range = match selected {
            Some(range) => range,
            None => unsafe { context.GetStart(ec)? },
        };
        let mut value = unsafe { property.GetValue(ec, &range)? };
        let unknown = unsafe {
            let inner = &value.Anonymous.Anonymous;
            if inner.vt == VARTYPE(VT_UNKNOWN as u16) {
                (*inner.Anonymous.punkVal).clone()
            } else {
                None
            }
        };
        unsafe {
            let _ = VariantClear(&mut value);
        }
        let Some(unknown) = unknown else {
            return Ok(Vec::new());
        };
        let scope: ITfInputScope = unknown.cast()?;
        let mut scopes = std::ptr::null_mut();
        let mut count = 0;
        unsafe { scope.GetInputScopes(&mut scopes, &mut count) }.ok()?;
        if scopes.is_null() {
            return Ok(Vec::new());
        }
        let read = unsafe { std::slice::from_raw_parts(scopes, count as usize) }.to_vec();
        unsafe { CoTaskMemFree(scopes.cast()) };
        Ok(read)
    }
}

#[cfg(windows)]
pub(crate) use read::of;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_private_field_asks_for_no_record() {
        assert!(asks_for_no_record(&[PRIVATE]));
        assert!(asks_for_no_record(&[57, PRIVATE]), "IS_TEXT and IS_PRIVATE");
    }

    #[test]
    fn a_field_for_a_password_or_pin_asks_for_no_record() {
        for scope in SECRET {
            assert!(asks_for_no_record(&[scope]), "{scope}");
        }
    }

    #[test]
    fn other_fields_are_recorded() {
        assert!(!asks_for_no_record(&[]));
        assert!(!asks_for_no_record(&[0]), "IS_DEFAULT");
        assert!(!asks_for_no_record(&[1, 50]), "IS_URL and IS_SEARCH");
    }
}
