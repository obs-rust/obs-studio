//! Align a grouped encoder without changing its registration or ownership.

pub fn align<M>(
    encoder: Option<(Option<M>, u32)>,
    timestamp: u64,
    forward: impl FnOnce(M, u64),
) {
    if let Some((Some(media), divisor)) = encoder
        && divisor >= 2
    {
        forward(media, timestamp);
    }
}
