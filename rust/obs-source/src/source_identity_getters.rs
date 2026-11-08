//! Borrowed source identity queries; no string decoding or ownership transfer.
use core::ffi::CStr;

#[derive(Clone, Copy)]
pub struct Identity<T> {
    pub uuid: T,
    pub unversioned_id: T,
}

pub fn uuid<T: Copy + Default>(source: Option<Identity<T>>, log: impl FnOnce(&'static CStr)) -> T {
    match source {
        Some(fields) => fields.uuid,
        None => {
            log(c"obs_source_get_uuid");
            T::default()
        }
    }
}

pub fn unversioned_id<T: Copy + Default>(
    source: Option<Identity<T>>,
    log: impl FnOnce(&'static CStr),
) -> T {
    match source {
        Some(fields) => fields.unversioned_id,
        None => {
            log(c"obs_source_get_unversioned_id");
            T::default()
        }
    }
}
