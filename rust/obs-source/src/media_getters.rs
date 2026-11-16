//! Synchronous media queries with lazy private-field access.
use crate::media_admission::{self, Admit, Call};
use core::ffi::CStr;

/// Reads occur only after each preceding admission step succeeds.
pub trait Source {
    fn has_data(&self) -> bool;
    fn output_flags(&self) -> u32;
    fn duration(&self) -> Option<i64>;
    fn time(&self) -> Option<i64>;
    fn state(&self) -> Option<u32>;
}

fn query<S: Source, T: Default>(
    source: Option<&S>,
    name: &'static CStr,
    call: Call,
    log: impl FnOnce(&'static CStr),
    callback: impl FnOnce(&S) -> Option<T>,
) -> T {
    let Some(source) = source else {
        log(name);
        return T::default();
    };
    if media_admission::admit(source.has_data()) != Admit::ReadFlags {
        return T::default();
    }
    if media_admission::admit_flags(Some(call), source.output_flags()) != Admit::ReadCallback {
        return T::default();
    }
    callback(source).unwrap_or_default()
}

pub fn duration(source: Option<&impl Source>, log: impl FnOnce(&'static CStr)) -> i64 {
    query(
        source,
        c"obs_source_media_get_duration",
        Call::GetDuration,
        log,
        Source::duration,
    )
}
pub fn time(source: Option<&impl Source>, log: impl FnOnce(&'static CStr)) -> i64 {
    query(
        source,
        c"obs_source_media_get_time",
        Call::GetTime,
        log,
        Source::time,
    )
}
pub fn state(source: Option<&impl Source>, log: impl FnOnce(&'static CStr)) -> u32 {
    query(
        source,
        c"obs_source_media_get_state",
        Call::GetState,
        log,
        Source::state,
    )
}
