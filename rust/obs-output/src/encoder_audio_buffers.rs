//! Encoder audio-buffer lifecycle. The host supplies libobs-owned storage.

/// The fixed number of input and output slots, including inactive planes.
pub const PLANES: usize = 8;

/// Operations on an encoder borrowed for one lifecycle call.
pub trait Host {
    /// Free and zero an input deque, then free and null its output allocation.
    fn free_plane(&mut self, plane: usize);
    /// Allocate an output plane with the encoder's allocator.
    fn allocate_plane(&mut self, plane: usize, bytes: usize);
    /// Current active planes and bytes per output plane.
    fn geometry(&self) -> (usize, usize);
    /// Publish negotiated geometry before the plugin's frame-size callback.
    fn set_format(&mut self, rate: u32, planes: usize, blocksize: usize);
    /// Call the plugin once with its original context.
    fn frame_size(&mut self) -> usize;
    /// Publish the callback result and its byte count.
    fn set_frame_size(&mut self, frames: usize, bytes: usize);
}

/// Release every slot, preserving geometry.
pub fn free(_host: &mut impl Host) { todo!() }
/// Release every slot and allocate the active output planes.
pub fn reset(_host: &mut impl Host) { todo!() }
/// Initialize from the final negotiated audio format.
pub fn initialize(_host: &mut impl Host, _rate: u32, _format: i32, _speakers: i32) { todo!() }
