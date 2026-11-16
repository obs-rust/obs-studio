//! Ordered background shortcut enumeration under the existing hotkey mutex.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::todo,
    clippy::unimplemented,
    clippy::unreachable
)]

/// The existing hotkey lock and the live background shortcut list.
///
/// `count` and `get` are called only while the lock is held, and both read
/// the list live: a callback may change the list, and later entries and the
/// count are read again, as in the original C.
pub trait BackgroundShortcuts {
    type Shortcut;
    /// Acquire the hotkey mutex; false when no core is available.
    fn lock(&mut self) -> bool;
    /// The current number of entries.
    fn count(&self) -> usize;
    /// The current entry at `index`, which is below [`Self::count`].
    fn get(&self, index: usize) -> Self::Shortcut;
    /// Release the successfully acquired mutex.
    fn unlock(&mut self);
}

/// Call `func` for each shortcut in order, under the mutex, until it returns
/// false. A missing callback returns before locking; an unavailable core
/// returns without calling it.
#[expect(clippy::todo, reason = "RED stub")]
pub fn enumerate<B, F>(list: &mut B, func: Option<F>)
where
    B: BackgroundShortcuts,
    F: FnMut(B::Shortcut) -> bool,
{
    let _ = (list, func);
    todo!("port obs_hotkeys_enum_background_shortcuts")
}
