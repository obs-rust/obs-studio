//! Borrowed source identity queries; no string decoding or ownership transfer.
use core::ffi::CStr;

#[derive(Clone, Copy)]
pub struct Identity<T> {
    pub uuid: T,
    pub unversioned_id: T,
}

pub fn uuid<T: Copy + Default>(source: Option<Identity<T>>, log: impl FnOnce(&'static CStr)) -> T {
    let _ = (source, log);
    todo!("source UUID port")
}

pub fn unversioned_id<T: Copy + Default>(
    source: Option<Identity<T>>,
    log: impl FnOnce(&'static CStr),
) -> T {
    let _ = (source, log);
    todo!("source unversioned ID port")
}
