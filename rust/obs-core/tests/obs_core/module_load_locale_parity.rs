//! Tier 3: unchanged C orchestration versus the safe Rust core, including
//! service call order, null paths, warnings, and failed create/add operations.
#![allow(unsafe_code)]
use obs_c_oracle as _;
use obs_core::module_load_locale::{LocaleLoader, load};
use proptest::prelude::*;
use std::ffi::{CStr, CString, c_char, c_void};

unsafe extern "C" {
    fn oracle_module_load_locale(
        module: bool,
        def: *const c_char,
        locale: *const c_char,
        event: unsafe extern "C" fn(*mut c_void, u32, *const c_char) -> bool,
        data: *mut c_void,
    ) -> bool;
}
#[derive(Clone, Default, Debug)]
struct Host {
    flags: [bool; 4],
    finds: usize,
    events: Vec<(u32, Option<Vec<u8>>)>,
}
impl Host {
    fn event(&mut self, kind: u32, text: Option<&[u8]>) -> bool {
        self.events.push((kind, text.map(<[u8]>::to_vec)));
        match kind {
            0 => {
                let found = if self.finds == 0 {
                    self.flags[0]
                } else {
                    self.flags[2]
                };
                self.finds += 1;
                found
            }
            1 => self.flags[1],
            2 => text.is_some() && self.flags[3],
            _ => false,
        }
    }
}
impl LocaleLoader for Host {
    type Lookup = ();
    fn create(&mut self, name: &[u8]) -> Option<()> {
        (self.event(0, Some(name)) && self.event(1, Some(name))).then_some(())
    }
    fn add(&mut self, _: &mut (), name: &[u8]) -> bool {
        let found = self.event(0, Some(name));
        self.event(2, found.then_some(name))
    }
    fn invalid(&mut self) {
        self.event(3, None);
    }
    fn failed(&mut self, locale: &[u8]) {
        self.event(4, Some(locale));
    }
}
unsafe extern "C" fn event(data: *mut c_void, kind: u32, text: *const c_char) -> bool {
    // SAFETY: the synchronous oracle passes our exclusive Host and nullable C strings.
    unsafe {
        let host = &mut *data.cast::<Host>();
        host.event(
            kind,
            (!text.is_null()).then(|| CStr::from_ptr(text).to_bytes()),
        )
    }
}
fn check(module: bool, def: Option<&[u8]>, locale: Option<&[u8]>, flags: [bool; 4]) {
    let mut rust = Host {
        flags,
        ..Host::default()
    };
    let mut c = rust.clone();
    let def_c = def.map(|s| CString::new(s).unwrap());
    let locale_c = locale.map(|s| CString::new(s).unwrap());
    // SAFETY: strings and context outlive this synchronous call; the callback
    // does not retain pointers, change logging state, or start threads.
    let expected = unsafe {
        oracle_module_load_locale(
            module,
            def_c.as_ref().map_or(std::ptr::null(), |s| s.as_ptr()),
            locale_c.as_ref().map_or(std::ptr::null(), |s| s.as_ptr()),
            event,
            (&mut c as *mut Host).cast(),
        )
    };
    assert_eq!(load(&mut rust, module, def, locale).is_some(), expected);
    assert_eq!(rust.events, c.events);
}
#[test]
fn module_locale_orchestration_edges() {
    for module in [false, true] {
        for def in [None, Some(&b""[..]), Some(&b"en-US"[..])] {
            for locale in [
                None,
                Some(&b""[..]),
                Some(&b"en-US"[..]),
                Some(&b"EN-us"[..]),
                Some(&b"de-DE"[..]),
            ] {
                for bits in 0u8..16 {
                    check(
                        module,
                        def,
                        locale,
                        [bits & 1 != 0, bits & 2 != 0, bits & 4 != 0, bits & 8 != 0],
                    );
                }
            }
        }
    }
}
proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(128)))]
    #[test]
    fn arbitrary_locales(
        module in any::<bool>(),
        def in prop::option::of(prop::collection::vec(1u8..=255, 0..48)),
        locale in prop::option::of(prop::collection::vec(1u8..=255, 0..48)),
        flags in any::<[bool;4]>(),
    ) {
        check(module, def.as_deref(), locale.as_deref(), flags);
    }
}
