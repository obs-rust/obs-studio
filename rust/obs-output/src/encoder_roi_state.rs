//! ROI-state reads retain the C getters' unsynchronized access and NULL logging.

pub fn has_roi(count: Option<usize>, invalid: impl FnOnce()) -> bool {
    match count {
        Some(count) => count > 0,
        None => {
            invalid();
            false
        }
    }
}

pub fn roi_increment(increment: Option<u32>, invalid: impl FnOnce()) -> u32 {
    match increment {
        Some(increment) => increment,
        None => {
            invalid();
            0
        }
    }
}
