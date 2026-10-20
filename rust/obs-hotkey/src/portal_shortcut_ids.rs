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

use crate::name_map::{
    KEY_NONE, OBS_KEY_LAST_VALUE, OBS_KEY_MOUSE1, OBS_KEY_MOUSE29, key_from_name, key_name,
};
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
        *self = Vec::new();
    }
    fn cat(&mut self, s: &CStr) {
        self.extend_from_slice(s.to_bytes());
    }
    fn cat_text(&mut self, other: &Self) {
        self.extend_from_slice(other);
    }
    fn is_empty(&self) -> bool {
        self.first().is_none_or(|&b| b == 0)
    }
}

/// `bindable` (`obs-nix-portal-shortcuts.h`): whether `combo` can be a
/// portal shortcut. It needs a keyboard key: not `OBS_KEY_NONE`, not out of
/// range and not a mouse button. Modifiers do not matter.
pub fn bindable(combo: KeyCombination) -> bool {
    let key = combo.key;
    // Portal triggers are keyboard shortcuts; modifier-only and mouse
    // combinations keep working only while OBS is focused.
    if key <= KEY_NONE || usize::try_from(key).is_ok_and(|k| k >= OBS_KEY_LAST_VALUE) {
        return false;
    }
    !(OBS_KEY_MOUSE1..=OBS_KEY_MOUSE29).contains(&key)
}

/// Appends each known modifier of `modifiers` as `name(m)` and `"+"`, one
/// `dstr_cat` each, in [`MODIFIER_NAMES`] order.
fn cat_modifiers(
    modifiers: u32,
    name: impl Fn(&ModifierName) -> &'static CStr,
    out: &mut impl Text,
) {
    for m in MODIFIER_NAMES
        .iter()
        .filter(|m| modifiers & m.modifier != 0)
    {
        out.cat(name(m));
        out.cat(c"+");
    }
}

/// `obs_nix_portal_shortcut_id`: replaces `id` with the shortcut id of
/// `combo` and returns true, or returns false and leaves `id` alone when
/// `combo` is not [`bindable`].
pub fn shortcut_id(combo: KeyCombination, id: &mut impl Text) -> bool {
    if !bindable(combo) {
        return false;
    }

    id.free();
    cat_modifiers(combo.modifiers, |m| m.id, id);
    // A bindable key always has a name.
    if let Some(name) = key_name(combo.key) {
        id.cat(name);
    }
    true
}

/// `obs_nix_portal_shortcut_combo`: the key combination a shortcut id names,
/// or `None` for anything else. Known modifiers may come in any order and
/// repeat; the key is an `OBS_KEY_*` name of a [`bindable`] key.
pub fn shortcut_combo(id: &CStr) -> Option<KeyCombination> {
    let bytes = id.to_bytes_with_nul();
    let mut modifiers = 0;
    let mut part = 0;

    // Every segment before a '+' is a known modifier id, matched exactly.
    while let Some(len) = bytes[part..].iter().position(|&b| b == b'+') {
        let segment = &bytes[part..part + len];
        let known = MODIFIER_NAMES.iter().find(|m| m.id.to_bytes() == segment)?;
        modifiers |= known.modifier;
        part += len + 1;
    }

    // The rest is the key's name.
    let rest = CStr::from_bytes_with_nul(&bytes[part..]).ok()?;
    if !rest.to_bytes().starts_with(b"OBS_KEY_") {
        return None;
    }
    let result = KeyCombination {
        modifiers,
        key: key_from_name(rest),
    };
    bindable(result).then_some(result)
}

/// `obs_nix_portal_shortcut_trigger`: replaces `trigger` with the preferred
/// trigger of `combo` and returns true. The key's name comes from
/// `key_name`, which starts from an empty `T` and returns whether there is
/// one. Returns false and leaves `trigger` alone when `combo` is not
/// [`bindable`] (then `key_name` is not called), when there is no
/// `key_name`, or when it has no name or an empty one.
pub fn shortcut_trigger<T, F>(combo: KeyCombination, key_name: Option<F>, trigger: &mut T) -> bool
where
    T: Text + Default,
    F: FnOnce(i32, &mut T) -> bool,
{
    let mut name = T::default();
    if !bindable(combo) || !key_name.is_some_and(|f| f(combo.key, &mut name)) || name.is_empty() {
        name.free();
        return false;
    }

    trigger.free();
    cat_modifiers(combo.modifiers, |m| m.trigger, trigger);
    trigger.cat_text(&name);
    name.free();
    true
}
