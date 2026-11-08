//! Synchronized stored push-to-mute/talk queries.
use core::ffi::CStr;

/// Borrowed source state; reads are valid only between lock and unlock.
pub trait PushFields {
    fn lock(&mut self);
    fn unlock(&mut self);
    fn mute_enabled(&self) -> bool;
    fn talk_enabled(&self) -> bool;
    fn mute_delay(&self) -> u64;
    fn talk_delay(&self) -> u64;
}

fn query<S: PushFields, T>(
    source: Option<&mut S>,
    name: &'static CStr,
    default: T,
    read: impl FnOnce(&S) -> T,
    log: impl FnOnce(&'static CStr, &'static CStr),
) -> T {
    match source {
        None => {
            log(name, c"source");
            default
        }
        Some(source) => {
            source.lock();
            let value = read(source);
            source.unlock();
            value
        }
    }
}

pub fn mute_enabled<S: PushFields>(
    source: Option<&mut S>,
    log: impl FnOnce(&'static CStr, &'static CStr),
) -> bool {
    query(
        source,
        c"obs_source_push_to_mute_enabled",
        false,
        S::mute_enabled,
        log,
    )
}

pub fn talk_enabled<S: PushFields>(
    source: Option<&mut S>,
    log: impl FnOnce(&'static CStr, &'static CStr),
) -> bool {
    query(
        source,
        c"obs_source_push_to_talk_enabled",
        false,
        S::talk_enabled,
        log,
    )
}

pub fn mute_delay<S: PushFields>(
    source: Option<&mut S>,
    log: impl FnOnce(&'static CStr, &'static CStr),
) -> u64 {
    query(
        source,
        c"obs_source_get_push_to_mute_delay",
        0,
        S::mute_delay,
        log,
    )
}

pub fn talk_delay<S: PushFields>(
    source: Option<&mut S>,
    log: impl FnOnce(&'static CStr, &'static CStr),
) -> u64 {
    query(
        source,
        c"obs_source_get_push_to_talk_delay",
        0,
        S::talk_delay,
        log,
    )
}
