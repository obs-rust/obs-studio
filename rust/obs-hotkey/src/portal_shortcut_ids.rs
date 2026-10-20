//! Safe core of `libobs/obs-nix-portal-shortcut-ids.c.h`: the
//! xdg-desktop-portal GlobalShortcuts shortcut id and preferred trigger of a
//! key combination.
//!
//! One portal shortcut is bound per key combination. Its id names the
//! combination (`"ctrl+shift+OBS_KEY_F13"`) and parses back to it; its
//! preferred trigger is the same combination in the XDG shortcuts format
//! (`"CTRL+SHIFT+F13"`). [`bindable`] is the `static inline` of
//! `obs-nix-portal-shortcuts.h`, which stays C; this is the same check.
//!
//! The text the C builds is a `struct dstr`, so the functions are generic
//! over [`Text`]: the C ABI shim runs them on the caller's `dstr` with the
//! same `dstr` calls, in the same order, as the C.

use std::ffi::CStr;

use crate::name_map::{KEY_NONE, OBS_KEY_LAST_VALUE, OBS_KEY_MOUSE1, OBS_KEY_MOUSE29};
use crate::nix_hotkey_state::{
    INTERACT_ALT_KEY, INTERACT_COMMAND_KEY, INTERACT_CONTROL_KEY, INTERACT_SHIFT_KEY,
    KeyCombination,
};

/// One entry of the C's `modifier_names`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModifierName {
    /// The `INTERACT_*_KEY` bit.
    pub modifier: u32,
    /// Its name in a shortcut id.
    pub id: &'static CStr,
    /// Its name in a preferred trigger.
    pub trigger: &'static CStr,
}

/// The modifiers the codec knows, in id and trigger order. Every other
/// `INTERACT_*` bit is left out of ids and triggers.
pub const MODIFIER_NAMES: [ModifierName; 4] = [
    ModifierName {
        modifier: INTERACT_CONTROL_KEY,
        id: c"ctrl",
        trigger: c"CTRL",
    },
    ModifierName {
        modifier: INTERACT_ALT_KEY,
        id: c"alt",
        trigger: c"ALT",
    },
    ModifierName {
        modifier: INTERACT_SHIFT_KEY,
        id: c"shift",
        trigger: c"SHIFT",
    },
    ModifierName {
        modifier: INTERACT_COMMAND_KEY,
        id: c"super",
        trigger: c"LOGO",
    },
];

/// The text an id or trigger is written to: a `struct dstr` in C.
pub trait Text {
    /// `dstr_free`: empty, with no storage.
    fn free(&mut self);
    /// `dstr_cat`: appends `s` (nothing for an empty `s`).
    fn cat(&mut self, s: &CStr);
    /// `dstr_cat_dstr`: appends `other`'s bytes (nothing when it has none).
    fn cat_text(&mut self, other: &Self);
    /// `dstr_is_empty`: no bytes, or a first byte of NUL.
    fn is_empty(&self) -> bool;
}

/// Plain bytes, without the terminator.
impl Text for Vec<u8> {
    fn free(&mut self) {
        todo!()
    }
    fn cat(&mut self, _s: &CStr) {
        todo!()
    }
    fn cat_text(&mut self, _other: &Self) {
        todo!()
    }
    fn is_empty(&self) -> bool {
        todo!()
    }
}

/// `bindable` (`obs-nix-portal-shortcuts.h`): whether `combo` can be a
/// portal shortcut. It needs a keyboard key: not `OBS_KEY_NONE`, not out of
/// range and not a mouse button. Modifiers do not matter.
pub fn bindable(_combo: KeyCombination) -> bool {
    todo!()
}

/// `obs_nix_portal_shortcut_id`: replaces `id` with the shortcut id of
/// `combo` and returns true, or returns false and leaves `id` alone when
/// `combo` is not [`bindable`].
pub fn shortcut_id(_combo: KeyCombination, _id: &mut impl Text) -> bool {
    todo!()
}

/// `obs_nix_portal_shortcut_combo`: the key combination a shortcut id names,
/// or `None` for anything else. Known modifiers may come in any order and
/// repeat; the key is an `OBS_KEY_*` name of a [`bindable`] key.
pub fn shortcut_combo(_id: &CStr) -> Option<KeyCombination> {
    todo!()
}

/// `obs_nix_portal_shortcut_trigger`: replaces `trigger` with the preferred
/// trigger of `combo` and returns true. The key's name comes from
/// `key_name`, which starts from an empty `T` and returns whether there is
/// one. Returns false and leaves `trigger` alone when `combo` is not
/// [`bindable`] (then `key_name` is not called), when there is no
/// `key_name`, or when it has no name or an empty one.
pub fn shortcut_trigger<T, F>(
    _combo: KeyCombination,
    _key_name: Option<F>,
    _trigger: &mut T,
) -> bool
where
    T: Text + Default,
    F: FnOnce(i32, &mut T) -> bool,
{
    todo!()
}

#[allow(dead_code)]
const _USED: (i32, usize, i32, i32) = (
    KEY_NONE,
    OBS_KEY_LAST_VALUE,
    OBS_KEY_MOUSE1,
    OBS_KEY_MOUSE29,
);
