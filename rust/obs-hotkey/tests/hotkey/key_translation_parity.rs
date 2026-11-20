//! Tier 3: the original extracted `obs_get_hotkey_translation` and
//! `obs_hotkeys_store_key_translation` versus the shims on one recording
//! fixture core (oracle/hotkey_key_translation.c), through the real-named
//! slot, bfree and bstrdup adapters. Compared: the free/copy trace and the
//! number of core reads, the returned pointer (NULL, the default, the stored
//! slot), the stored slot (NULL, its text, never the caller's pointer), the
//! allocation balance and every other key left untouched.
//!
//! Excluded: a key outside `0..OBS_KEY_LAST_VALUE` (an out-of-bounds index in
//! both, unchecked as in C) and a live core missing for any key but
//! `OBS_KEY_NONE` (a NULL dereference in both). No intentional differences.
//! Mutants show the probes catch each broken contract.
use core::ffi::{CStr, c_char, c_int};
use core::mem::size_of;
use core::ptr::{NonNull, null, null_mut};
use obs_hotkey::ffi::key_translation::{
    obs_get_hotkey_translation, obs_hotkeys_store_key_translation,
};
use obs_hotkey::key_translation::{KeyTranslations, OBS_KEY_NONE, store, translation};
use proptest::prelude::*;
use std::ffi::CString;

type Query = unsafe extern "C" fn(c_int, *const c_char) -> *const c_char;
type Store = unsafe extern "C" fn(c_int, *const c_char);
unsafe extern "C" {
    fn oracle_hotkey_key_translation_query_probe(
        key: c_int,
        stored: *const c_char,
        def: *const c_char,
        candidate: Query,
    ) -> u32;
    fn oracle_hotkey_key_translation_store_probe(
        key: c_int,
        initial: *const c_char,
        arg: *const c_char,
        candidate: Store,
    ) -> u32;
    fn oracle_hotkey_key_translation_trace(
        query: bool,
        key: c_int,
        initial: *const c_char,
        arg: *const c_char,
        out: *mut c_char,
        size: usize,
        reads: *mut u32,
    );
    fn oracle_hotkey_key_translation_layout(out: *mut usize);
    fn obs_hotkey_key_translation_slot(key: c_int) -> *mut *mut c_char;
    fn obs_hotkey_key_translation_free(translation: *mut c_char);
    fn obs_hotkey_key_translation_dup(translation: *const c_char) -> *mut c_char;
}

fn layout() -> [usize; 7] {
    let mut out = [0; 7];
    // SAFETY: the oracle writes seven size_t measurements from the real headers.
    unsafe { oracle_hotkey_key_translation_layout(out.as_mut_ptr()) };
    out
}

fn last_key() -> c_int {
    c_int::try_from(layout()[0]).unwrap()
}

fn text(value: &Option<CString>) -> *const c_char {
    value.as_ref().map_or(null(), |s| s.as_ptr())
}

fn query_probe(key: c_int, stored: &Option<CString>, def: &Option<CString>, q: Query) -> u32 {
    // SAFETY: key is below OBS_KEY_LAST_VALUE; the strings outlive the call
    // and the oracle owns the fixture core every adapter reaches.
    unsafe { oracle_hotkey_key_translation_query_probe(key, text(stored), text(def), q) }
}

fn store_probe(key: c_int, initial: &Option<CString>, arg: &Option<CString>, s: Store) -> u32 {
    // SAFETY: as in `query_probe`.
    unsafe { oracle_hotkey_key_translation_store_probe(key, text(initial), text(arg), s) }
}

fn check(key: c_int, stored: &Option<CString>, arg: &Option<CString>) {
    assert_eq!(
        query_probe(key, stored, arg, obs_get_hotkey_translation),
        0,
        "query key={key} stored={stored:?} def={arg:?}"
    );
    assert_eq!(
        store_probe(key, stored, arg, obs_hotkeys_store_key_translation),
        0,
        "store key={key} initial={stored:?} arg={arg:?}"
    );
}

fn trace(query: bool, key: c_int, initial: Option<&str>, arg: Option<&str>) -> (String, u32) {
    let initial = initial.map(|s| CString::new(s).unwrap());
    let arg = arg.map(|s| CString::new(s).unwrap());
    let mut out = [0 as c_char; 32];
    let mut reads = 0;
    // SAFETY: the oracle writes a terminated trace of at most 31 bytes.
    unsafe {
        oracle_hotkey_key_translation_trace(
            query,
            key,
            text(&initial),
            text(&arg),
            out.as_mut_ptr(),
            out.len(),
            &mut reads,
        );
        (
            CStr::from_ptr(out.as_ptr()).to_str().unwrap().to_owned(),
            reads,
        )
    }
}

fn some(s: &str) -> Option<CString> {
    Some(CString::new(s).unwrap())
}

#[test]
fn real_header_layout() {
    let [last, array, slot, key, none, a, b] = layout();
    assert_eq!(array, last * size_of::<*mut c_char>());
    assert_eq!(slot, size_of::<*mut c_char>());
    assert_eq!(key, size_of::<c_int>());
    assert_eq!(c_int::try_from(none).unwrap(), OBS_KEY_NONE);
    assert!(a != b && a != none && b != none && a < last && b < last);
}

#[test]
fn original_traces_match_the_characterization() {
    let [_, _, _, _, _, a, _] = layout();
    let a = c_int::try_from(a).unwrap();
    // Query: no core access for OBS_KEY_NONE; one read when unset; a stored
    // name is read again to return it.
    assert_eq!(
        trace(true, OBS_KEY_NONE, None, Some("default")),
        (String::new(), 0)
    );
    assert_eq!(trace(true, a, None, Some("default")), (String::new(), 1));
    assert_eq!(trace(true, a, Some("stored"), None), (String::new(), 2));
    // Store: free the old slot (NULL too), clear it, then copy the caller's.
    assert_eq!(trace(false, a, None, Some("first")), ("fD".into(), 3));
    assert_eq!(
        trace(false, a, Some("first"), Some("second")),
        ("FD".into(), 3)
    );
    assert_eq!(trace(false, a, Some("first"), None), ("F".into(), 2));
    assert_eq!(trace(false, a, None, None), ("f".into(), 2));
}

#[test]
fn characterized_edges() {
    let [last, _, _, _, _, a, b] = layout();
    let keys = [
        OBS_KEY_NONE,
        1,
        c_int::try_from(a).unwrap(),
        c_int::try_from(b).unwrap(),
        c_int::try_from(last - 1).unwrap(),
    ];
    let texts = [
        None,
        some(""),
        some("translated A"),
        some("a longer replacement translation"),
        some("\u{e9}\u{4e2d}"),
    ];
    for key in keys {
        for stored in &texts {
            for arg in &texts {
                check(key, stored, arg);
            }
        }
    }
}

fn bytes() -> impl Strategy<Value = Option<CString>> {
    prop::option::of(prop::collection::vec(1u8..=255, 0..48).prop_map(|v| CString::new(v).unwrap()))
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: obs_c_oracle::parity::cases(256), failure_persistence: None,
        ..ProptestConfig::default()
    })]
    #[test]
    fn query_and_store_match_c(key in 0..last_key(), stored in bytes(), arg in bytes()) {
        check(key, &stored, &arg);
    }
}

/// The adapters as the shim sees them, for the mutants.
struct Adapters;
impl KeyTranslations for Adapters {
    type Text = NonNull<c_char>;
    fn get(&mut self, key: i32) -> Option<NonNull<c_char>> {
        // SAFETY: fixture adapter; key is in range.
        NonNull::new(unsafe { *obs_hotkey_key_translation_slot(key) })
    }
    fn set(&mut self, key: i32, value: Option<NonNull<c_char>>) {
        // SAFETY: fixture adapter; key is in range.
        unsafe {
            *obs_hotkey_key_translation_slot(key) = value.map_or(null_mut(), NonNull::as_ptr)
        };
    }
    fn release(&mut self, value: Option<NonNull<c_char>>) {
        // SAFETY: fixture adapter; frees what the slot held.
        unsafe { obs_hotkey_key_translation_free(value.map_or(null_mut(), NonNull::as_ptr)) };
    }
    fn duplicate(&mut self, value: NonNull<c_char>) -> Option<NonNull<c_char>> {
        // SAFETY: fixture adapter; value is NUL-terminated.
        NonNull::new(unsafe { obs_hotkey_key_translation_dup(value.as_ptr()) })
    }
}

unsafe extern "C" fn reads_for_none(key: c_int, def: *const c_char) -> *const c_char {
    let _ = Adapters.get(key);
    // SAFETY: as the shim.
    unsafe { obs_get_hotkey_translation(key, def) }
}

unsafe extern "C" fn prefers_default(key: c_int, def: *const c_char) -> *const c_char {
    if def.is_null() {
        // SAFETY: as the shim.
        unsafe { obs_get_hotkey_translation(key, def) }
    } else {
        def
    }
}

unsafe extern "C" fn reads_once(key: c_int, def: *const c_char) -> *const c_char {
    if key == OBS_KEY_NONE {
        return null();
    }
    Adapters
        .get(key)
        .map_or(def, |text| text.as_ptr().cast_const())
}

unsafe extern "C" fn leaks_old(key: c_int, arg: *const c_char) {
    let mut table = Adapters;
    table.set(key, None);
    if let Some(arg) = NonNull::new(arg.cast_mut()) {
        let copy = table.duplicate(arg);
        table.set(key, copy);
    }
    let _ = table.get(key);
}

unsafe extern "C" fn copies_other_text(key: c_int, arg: *const c_char) {
    let mut table = Adapters;
    let old = table.get(key);
    table.release(old);
    table.set(key, None);
    if !arg.is_null() {
        let copy = table.duplicate(NonNull::from(c"other").cast());
        table.set(key, copy);
    }
}

unsafe extern "C" fn copies_before_free(key: c_int, arg: *const c_char) {
    let mut table = Adapters;
    let copy = NonNull::new(arg.cast_mut()).map(|arg| table.duplicate(arg));
    let old = table.get(key);
    table.release(old);
    table.set(key, None);
    if let Some(copy) = copy {
        table.set(key, copy);
    }
}

unsafe extern "C" fn null_keeps_old(key: c_int, arg: *const c_char) {
    if arg.is_null() {
        let mut table = Adapters;
        let _ = table.get(key);
        let _ = table.get(key);
        let old = table.get(key);
        table.release(None);
        let _ = old;
        return;
    }
    // SAFETY: as the shim.
    unsafe { obs_hotkeys_store_key_translation(key, arg) };
}

unsafe extern "C" fn wrong_key(key: c_int, arg: *const c_char) {
    // SAFETY: as the shim, on the neighbouring key (in range in every case).
    unsafe { obs_hotkeys_store_key_translation(key ^ 1, arg) };
}

#[test]
fn mutants_are_caught() {
    let [_, _, _, _, _, a, _] = layout();
    let a = c_int::try_from(a).unwrap();
    let stored = some("stored");
    let def = some("default");
    let arg = some("new name");
    assert_eq!(
        query_probe(OBS_KEY_NONE, &None, &def, reads_for_none) & 1,
        1
    );
    assert_eq!(query_probe(a, &stored, &def, prefers_default) & 2, 2);
    assert_eq!(query_probe(a, &stored, &def, reads_once) & 1, 1);
    assert_eq!(store_probe(a, &stored, &arg, leaks_old) & 5, 5);
    assert_eq!(store_probe(a, &stored, &arg, copies_other_text) & 3, 3);
    assert_eq!(store_probe(a, &stored, &arg, copies_before_free) & 1, 1);
    assert_eq!(store_probe(a, &stored, &None, null_keeps_old) & 6, 6);
    assert_eq!(store_probe(a, &None, &arg, wrong_key) & 6, 6);
    // The safe core through the same adapters agrees with the shims.
    assert_eq!(
        translation(&mut Adapters, OBS_KEY_NONE, None).map(NonNull::as_ptr),
        None
    );
    unsafe extern "C" fn core_store(key: c_int, arg: *const c_char) {
        store(&mut Adapters, key, NonNull::new(arg.cast_mut()));
    }
    assert_eq!(store_probe(a, &stored, &arg, core_store), 0);
    check(a, &stored, &arg);
}
