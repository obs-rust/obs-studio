//! Source context initialization order; failed registration deliberately has no rollback.
use core::ffi::CStr;

pub fn initialize<T>(
    inputs: T,
    init: impl FnOnce(T) -> bool,
    register: impl FnOnce(&[&CStr]) -> bool,
) -> bool {
    init(inputs) && register(SIGNALS)
}

/// Original signal declarations, in registration order.
pub const SIGNALS: &[&CStr] = &[
    c"void destroy(ptr source)",
    c"void remove(ptr source)",
    c"void update(ptr source)",
    c"void save(ptr source)",
    c"void load(ptr source)",
    c"void activate(ptr source)",
    c"void deactivate(ptr source)",
    c"void show(ptr source)",
    c"void hide(ptr source)",
    c"void mute(ptr source, bool muted)",
    c"void push_to_mute_changed(ptr source, bool enabled)",
    c"void push_to_mute_delay(ptr source, int delay)",
    c"void push_to_talk_changed(ptr source, bool enabled)",
    c"void push_to_talk_delay(ptr source, int delay)",
    c"void enable(ptr source, bool enabled)",
    c"void rename(ptr source, string new_name, string prev_name)",
    c"void volume(ptr source, in out float volume)",
    c"void update_properties(ptr source)",
    c"void update_flags(ptr source, int flags)",
    c"void audio_sync(ptr source, int out int offset)",
    c"void audio_balance(ptr source, in out float balance)",
    c"void audio_mixers(ptr source, in out int mixers)",
    c"void audio_monitoring(ptr source, int type)",
    c"void monitor(ptr source, bool monitor)",
    c"void audio_activate(ptr source)",
    c"void audio_deactivate(ptr source)",
    c"void filter_add(ptr source, ptr filter)",
    c"void filter_remove(ptr source, ptr filter)",
    c"void reorder_filters(ptr source)",
    c"void transition_start(ptr source)",
    c"void transition_video_stop(ptr source)",
    c"void transition_stop(ptr source)",
    c"void media_play(ptr source)",
    c"void media_pause(ptr source)",
    c"void media_restart(ptr source)",
    c"void media_stopped(ptr source)",
    c"void media_next(ptr source)",
    c"void media_previous(ptr source)",
    c"void media_started(ptr source)",
    c"void media_ended(ptr source)",
];
