//! Async source decoupling, preserving the enable-only audio reset sequence.

/// Operations on a live source. Audio operations run between lock and unlock.
pub trait Source {
    fn set_decoupled(&mut self, value: bool);
    fn lock_audio(&mut self);
    fn invalidate_timing(&mut self);
    fn reset_audio(&mut self, timestamp: u64);
    fn unlock_audio(&mut self);
}

/// Set the flag unconditionally; every enable invalidates and resets audio.
pub fn set<S: Source>(source: Option<&mut S>, decouple: bool, null_log: impl FnOnce()) {
    let _ = (source, decouple, null_log);
    todo!()
}
