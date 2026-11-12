//! Legacy output-source getter; the canvas getter owns synchronization and refs.
/// Forward the borrowed current main canvas and return the acquired source.
pub fn get_output_source<C, S>(
    _canvas: C,
    _channel: u32,
    _get_channel: impl FnOnce(C, u32) -> S,
) -> S {
    todo!("port obs_get_output_source")
}
