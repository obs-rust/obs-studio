//! Align a grouped encoder without changing its registration or ownership.

pub fn align<M, I>(
    encoder: Option<(Option<M>, u32, I)>,
    timestamp: u64,
    forward: impl FnOnce(M, u64, I),
) {
    if let Some((Some(media), divisor, identity)) = encoder
        && divisor >= 2
    {
        forward(media, timestamp, identity);
    }
}
