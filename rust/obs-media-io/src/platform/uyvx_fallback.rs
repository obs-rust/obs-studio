//! Unsupported architectures retain the scalar conversion.
use super::uyvx::Rows;
pub(crate) struct Native;
impl Native {
    pub(super) fn detect() -> Option<Self> {
        None
    }
    pub(super) fn i420(&self, _rows: Rows<'_>, _u: &mut [u8], _v: &mut [u8]) {
        unreachable!()
    }
    pub(super) fn nv12(&self, _rows: Rows<'_>, _chroma: &mut [u8]) {
        unreachable!()
    }
    pub(super) fn i444(&self, _rows: Rows<'_>, _u: &mut [u8], _v: &mut [u8]) {
        unreachable!()
    }
}
