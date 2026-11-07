//! ROI-state reads retain the C getters' unsynchronized access and NULL logging.

pub fn has_roi(_count: Option<usize>, _invalid: impl FnOnce()) -> bool {
    todo!()
}

pub fn roi_increment(_increment: Option<u32>, _invalid: impl FnOnce()) -> u32 {
    todo!()
}
