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
    encoder: Option<&mut impl RoiList>,
    roi: Option<&Roi>,
    invalid: impl FnOnce(),
) -> bool {
    let Some(encoder) = encoder else {
        invalid();
        return false;
    };
    let Some(roi) = roi else { return false };
    if !crate::encoder_roi::admit(encoder.caps(), roi) {
        return false;
    }
    encoder.with_locked(|entries| {
        entries.append(roi);
        let increment = entries.increment();
        *increment = increment.wrapping_add(1);
    });
    true
}

/// Visit newest first under the mutex, with temporary copies when scaled.
pub fn enum_roi(
    encoder: Option<&mut impl RoiList>,
    mut callback: impl FnMut(&mut Roi),
    invalid: impl FnOnce(),
) {
    let Some(encoder) = encoder else {
        invalid();
        return;
    };
    let (sw, sh) = encoder.scaled_size();
    let factors = if sw != 0 && sh != 0 {
        let (w, h) = encoder.video_size();
        let Some(factors) = crate::encoder_roi::scale_factors(sw, sh, w, h) else {
            return;
        };
        Some(factors)
    } else {
        None
    };
    encoder.with_locked(|entries| {
        for index in (0..entries.count()).rev() {
            entries.visit(index, &mut |roi| {
                if let Some((sx, sy)) = factors {
                    callback(&mut crate::encoder_roi::scale(roi, sx, sy));
                } else {
                    callback(roi);
                }
            });
        }
    });
}
