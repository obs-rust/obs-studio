//! Legacy output-source wrapper; the existing canvas setter owns all effects.
/// Forward the borrowed current main canvas and the unchanged caller arguments.
pub fn set_output_source<C, S>(
    canvas: C,
    channel: u32,
    source: S,
    set_channel: impl FnOnce(C, u32, S),
) {
    let _ = (canvas, channel, source, set_channel);
    todo!("obs_set_output_source Rust swap")
}
