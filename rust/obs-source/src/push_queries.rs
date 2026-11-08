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

pub fn mute_enabled<S: PushFields>(
    _source: Option<&mut S>,
    _log: impl FnOnce(&'static CStr, &'static CStr),
) -> bool {
    todo!()
}

pub fn talk_enabled<S: PushFields>(
    _source: Option<&mut S>,
    _log: impl FnOnce(&'static CStr, &'static CStr),
) -> bool {
    todo!()
}

pub fn mute_delay<S: PushFields>(
    _source: Option<&mut S>,
    _log: impl FnOnce(&'static CStr, &'static CStr),
) -> u64 {
    todo!()
}

pub fn talk_delay<S: PushFields>(
    _source: Option<&mut S>,
    _log: impl FnOnce(&'static CStr, &'static CStr),
) -> u64 {
    todo!()
}
