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
    /// Current blocksize, which the callback may have changed.
    fn blocksize(&self) -> usize;
    /// Publish the callback result and its byte count.
    fn set_frame_size(&mut self, frames: usize, bytes: usize);
}

/// Release every slot, preserving geometry.
pub fn free(host: &mut impl Host) {
    for plane in 0..PLANES {
        host.free_plane(plane);
    }
}
/// Release every slot and allocate the active output planes.
pub fn reset(host: &mut impl Host) {
    free(host);
    let (planes, bytes) = host.geometry();
    for plane in 0..planes {
        host.allocate_plane(plane, bytes);
    }
}
/// Initialize from the final negotiated audio format.
pub fn initialize(host: &mut impl Host, rate: u32, format: i32, speakers: i32) {
    use obs_media_io::audio_io::{
        AudioFormat, SpeakerLayout, audio_format_from_c, audio_planes, audio_size,
        speaker_layout_from_c,
    };
    let format = audio_format_from_c(format).unwrap_or(AudioFormat::Unknown);
    let speakers = speaker_layout_from_c(speakers).unwrap_or(SpeakerLayout::Unknown);
    let planes = audio_planes(format, speakers);
    let blocksize = audio_size(format, speakers, 1);
    host.set_format(rate, planes, blocksize);
    let frames = host.frame_size();
    host.set_frame_size(frames, host.blocksize().wrapping_mul(frames));
    reset(host);
}
