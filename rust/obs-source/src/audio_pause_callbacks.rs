//! The paired subscriptions used to pause source audio.
use core::ffi::CStr;

/// Validate before acquiring the borrowed handler, then update both signals.
pub fn update<H>(
    present: bool,
    adding: bool,
    handler: impl FnOnce() -> H,
    mut change: impl FnMut(&H, &'static CStr),
    log: impl FnOnce(&'static CStr),
) {
    if !present {
        log(if adding {
            c"obs_source_add_audio_pause_callback"
        } else {
            c"obs_source_remove_audio_pause_callback"
        });
        return;
    }
    let handler = handler();
    change(&handler, c"media_pause");
    change(&handler, c"media_stopped");
}
