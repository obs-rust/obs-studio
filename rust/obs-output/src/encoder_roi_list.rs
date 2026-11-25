//! ROI list policy; storage and the encoder's existing mutex are borrowed.
use crate::encoder_roi::Roi;

/// Operations available only while the shared ROI mutex is held.
pub trait Entries {
    fn append(&mut self, roi: &Roi);
    fn increment(&mut self) -> &mut u32;
    fn count(&self) -> usize;
    fn visit(&mut self, index: usize, callback: &mut dyn FnMut(&mut Roi));
}

/// Private encoder fields, with video dimensions read only when scaling.
pub trait RoiList {
    fn caps(&self) -> u32;
    fn scaled_size(&self) -> (u32, u32);
    fn video_size(&self) -> (u32, u32);
    fn with_locked(&mut self, action: impl FnOnce(&mut dyn Entries));
}

/// Admit and append one ROI, incrementing only on success.
pub fn add_roi(
    _encoder: Option<&mut impl RoiList>,
    _roi: Option<&Roi>,
    _invalid: impl FnOnce(),
) -> bool {
    todo!()
}

/// Visit newest first under the mutex, with temporary copies when scaled.
pub fn enum_roi(
    _encoder: Option<&mut impl RoiList>,
    _callback: impl FnMut(&mut Roi),
    _invalid: impl FnOnce(),
) {
    todo!()
}
