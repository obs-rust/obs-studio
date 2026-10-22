//! C ABI for `libobs/obs-hotkey-translations.c.h`:
//! `obs_hotkeys_set_translations_s`, `EXPORT` in `libobs/obs-hotkey.h`.
//!
//! It is public API, so the symbol is exported
//! (`rust-exports.d/obs-hotkey-translations.txt`). The frontend calls it
//! once at startup with its locale's names; a plugin can call it too.
//!
//! As in C, the shim copies `min(size, sizeof)` bytes of the caller's
//! struct into a zeroed one, so an older, shorter struct reads as `NULL`
//! tails, and a larger `size` reads only the struct. A `NULL` struct
//! returns. The safe core (`translations::for_each_store`) then gives each
//! name, and the shim hands it to libobs's hidden store,
//! `obs_hotkeys_store_key_translation()` (`obs-internal.h`, still C in
//! `obs-hotkey.c.h`), which `bstrdup`s it. A named key's string is passed
//! on as the caller's pointer, as the C does; an expanded name lives until
//! the store call returns.
//!
//! The C's `#ifdef __APPLE__` keypad block is [`KEYPAD`] here.
//! `obs_key_t` is a C enum, passed as `int`.
//!
//! No intentional differences from C.

use std::ffi::{CStr, c_char, c_int};

use crate::translations::{self as core, Keypad, MEMBER_COUNT, Translations};

unsafe extern "C" {
    fn obs_hotkeys_store_key_translation(key: c_int, translation: *const c_char);
}

/// The keypad members this host applies (`#ifdef __APPLE__` in the C).
#[cfg(target_os = "macos")]
pub const KEYPAD: Keypad = Keypad::Apple;
/// The keypad members this host applies (`#ifdef __APPLE__` in the C).
#[cfg(not(target_os = "macos"))]
pub const KEYPAD: Keypad = Keypad::Standard;

/// `struct obs_hotkeys_translations` in `obs-hotkey.h`: 38 `const char *`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
#[allow(non_camel_case_types)]
pub struct obs_hotkeys_translations {
    pub insert: *const c_char,
    pub del: *const c_char,
    pub home: *const c_char,
    pub end: *const c_char,
    pub page_up: *const c_char,
    pub page_down: *const c_char,
    pub num_lock: *const c_char,
    pub scroll_lock: *const c_char,
    pub caps_lock: *const c_char,
    pub backspace: *const c_char,
    pub tab: *const c_char,
    pub print: *const c_char,
    pub pause: *const c_char,
    pub left: *const c_char,
    pub right: *const c_char,
    pub up: *const c_char,
    pub down: *const c_char,
    pub shift: *const c_char,
    pub alt: *const c_char,
    pub control: *const c_char,
    pub meta: *const c_char,
    pub menu: *const c_char,
    pub space: *const c_char,
    pub numpad_num: *const c_char,
    pub numpad_divide: *const c_char,
    pub numpad_multiply: *const c_char,
    pub numpad_minus: *const c_char,
    pub numpad_plus: *const c_char,
    pub numpad_decimal: *const c_char,
    pub apple_keypad_num: *const c_char,
    pub apple_keypad_divide: *const c_char,
    pub apple_keypad_multiply: *const c_char,
    pub apple_keypad_minus: *const c_char,
    pub apple_keypad_plus: *const c_char,
    pub apple_keypad_decimal: *const c_char,
    pub apple_keypad_equal: *const c_char,
    pub mouse_num: *const c_char,
    pub escape: *const c_char,
}

impl obs_hotkeys_translations {
    /// Every member `NULL`: `struct obs_hotkeys_translations t = {0}`.
    pub const ZEROED: Self = Self::from_members([std::ptr::null(); MEMBER_COUNT]);

    /// The members in struct order (`translations::Member::ALL`).
    pub const fn from_members(m: [*const c_char; MEMBER_COUNT]) -> Self {
        let [
            insert,
            del,
            home,
            end,
            page_up,
            page_down,
            num_lock,
            scroll_lock,
            caps_lock,
            backspace,
            tab,
            print,
            pause,
            left,
            right,
            up,
            down,
            shift,
            alt,
            control,
            meta,
            menu,
            space,
            numpad_num,
            numpad_divide,
            numpad_multiply,
            numpad_minus,
            numpad_plus,
            numpad_decimal,
            apple_keypad_num,
            apple_keypad_divide,
            apple_keypad_multiply,
            apple_keypad_minus,
            apple_keypad_plus,
            apple_keypad_decimal,
            apple_keypad_equal,
            mouse_num,
            escape,
        ] = m;
        Self {
            insert,
            del,
            home,
            end,
            page_up,
            page_down,
            num_lock,
            scroll_lock,
            caps_lock,
            backspace,
            tab,
            print,
            pause,
            left,
            right,
            up,
            down,
            shift,
            alt,
            control,
            meta,
            menu,
            space,
            numpad_num,
            numpad_divide,
            numpad_multiply,
            numpad_minus,
            numpad_plus,
            numpad_decimal,
            apple_keypad_num,
            apple_keypad_divide,
            apple_keypad_multiply,
            apple_keypad_minus,
            apple_keypad_plus,
            apple_keypad_decimal,
            apple_keypad_equal,
            mouse_num,
            escape,
        }
    }

    /// The members in struct order (`translations::Member::ALL`).
    pub const fn members(&self) -> [*const c_char; MEMBER_COUNT] {
        [
            self.insert,
            self.del,
            self.home,
            self.end,
            self.page_up,
            self.page_down,
            self.num_lock,
            self.scroll_lock,
            self.caps_lock,
            self.backspace,
            self.tab,
            self.print,
            self.pause,
            self.left,
            self.right,
            self.up,
            self.down,
            self.shift,
            self.alt,
            self.control,
            self.meta,
            self.menu,
            self.space,
            self.numpad_num,
            self.numpad_divide,
            self.numpad_multiply,
            self.numpad_minus,
            self.numpad_plus,
            self.numpad_decimal,
            self.apple_keypad_num,
            self.apple_keypad_divide,
            self.apple_keypad_multiply,
            self.apple_keypad_minus,
            self.apple_keypad_plus,
            self.apple_keypad_decimal,
            self.apple_keypad_equal,
            self.mouse_num,
            self.escape,
        ]
    }
}

/// `obs_hotkeys_set_translations_s()`: stores the translated key names of
/// `translations`, of which `size` bytes are read.
///
/// # Safety
///
/// `translations` is null or valid for reads of `min(size,
/// sizeof(struct obs_hotkeys_translations))` bytes, and each member in
/// those bytes is null or a NUL-terminated string that lives for the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_hotkeys_set_translations_s(
    translations: *mut obs_hotkeys_translations,
    size: usize,
) {
    if translations.is_null() {
        return;
    }
    let mut t = obs_hotkeys_translations::ZEROED;
    let n = size.min(size_of::<obs_hotkeys_translations>());
    // SAFETY: the caller's struct is valid for `n` bytes (see above), `t`
    // is a local struct of at least `n` bytes, and they do not overlap.
    // Like the C's memcpy, a size that cuts a member copies part of it.
    unsafe {
        std::ptr::copy_nonoverlapping(
            translations.cast::<u8>().cast_const(),
            (&raw mut t).cast::<u8>(),
            n,
        );
    }
    let members = t.members().map(|p| {
        // SAFETY: a member that is not null is a NUL-terminated string
        // that lives for this call (see above).
        (!p.is_null()).then(|| unsafe { CStr::from_ptr(p) })
    });
    core::for_each_store(&Translations::from_members(members), KEYPAD, |key, name| {
        // SAFETY: `name` is NUL-terminated and lives for the call; the store
        // copies it.
        unsafe { obs_hotkeys_store_key_translation(key, name.as_ptr()) };
    });
}
