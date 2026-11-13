//! Legacy output-source getter; the canvas getter owns synchronization and refs.
/// Forward the borrowed current main canvas and return the acquired source.
pub fn get_output_source<C, S>(
    canvas: C,
    channel: u32,
    get_channel: impl FnOnce(C, u32) -> S,
) -> S {
    get_channel(canvas, channel)
}
