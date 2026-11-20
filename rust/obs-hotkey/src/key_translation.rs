//! The key name store, `obs->hotkeys.translations[]`, and its query
//! (`libobs/obs-hotkey-key-translation.c.h`).
//!
//! The table, its allocator and the key range stay with the caller: the
//! query takes no lock and checks no range, as in C. A stored name is
//! borrowed by the query, never copied.

/// `OBS_KEY_NONE` in `obs-hotkeys.h`.
pub const OBS_KEY_NONE: i32 = 0;

/// Access to the caller's per-key name slots and their allocator.
pub trait KeyTranslations {
    /// A stored or caller-owned name (never null).
    type Text: Copy;
    /// The current slot of `key`; one read of the table per call.
    fn get(&mut self, key: i32) -> Option<Self::Text>;
    /// Overwrite the slot of `key`.
    fn set(&mut self, key: i32, value: Option<Self::Text>);
    /// Release a stored name; `None` is released too, as `bfree(NULL)`.
    fn release(&mut self, value: Option<Self::Text>);
    /// An owned copy of `value` in the table's allocator.
    fn duplicate(&mut self, value: Self::Text) -> Self::Text;
}

/// The stored name of `key`, else `default`. `OBS_KEY_NONE` returns `None`
/// without touching the table.
#[expect(clippy::todo, reason = "RED stub")]
pub fn translation<T: KeyTranslations>(
    table: &mut T,
    key: i32,
    default: Option<T::Text>,
) -> Option<T::Text> {
    let _ = (table, key, default);
    todo!()
}

/// Replace the stored name of `key` with a copy of `translation`, or clear
/// it for `None`. The old name is released first.
#[expect(clippy::todo, reason = "RED stub")]
pub fn store<T: KeyTranslations>(table: &mut T, key: i32, translation: Option<T::Text>) {
    let _ = (table, key, translation);
    todo!()
}
