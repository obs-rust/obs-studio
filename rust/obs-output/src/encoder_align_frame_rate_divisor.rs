//! Align a grouped encoder without changing its registration or ownership.

pub fn align<M>(
    encoder: Option<(Option<M>, u32)>,
    timestamp: u64,
    forward: impl FnOnce(M, u64),
) {
    let _ = (encoder, timestamp, forward);
    todo!("encoder frame-rate divisor alignment")
}
