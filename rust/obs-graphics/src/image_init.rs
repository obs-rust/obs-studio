//! Image decoder lifecycle; FFmpeg ownership and layouts stay in the private C adapter.

/// Private adapter operations. Values are shared with graphics-ffmpeg-init-adapter.c.h.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Operation {
    Reset, Open, StreamCount, FirstVideo, FirstKnown, Probe, BestStream,
    FindDecoder, TooLarge, Allocate, Configure, OpenCodec, FreeCodec, CloseFormat,
    WarnOpen, WarnProbe, WarnStream, WarnDecoder, WarnCodec, BufferTooLarge, DefaultBuffer,
}

/// A decoder session. Each operation is one FFmpeg call or private field access.
pub trait Backend {
    fn run(&mut self, operation: Operation, value: i32) -> i32;
}

/// Initialize a nonempty local image path, retaining successful handles until free.
pub fn init(backend: &mut impl Backend, nonempty: bool) -> bool {
    let _ = (backend, nonempty);
    todo!()
}

/// Release the codec before its owning input format.
pub fn free(backend: &mut impl Backend) {
    let _ = backend;
    todo!()
}

/// Reject oversized buffers before FFmpeg allocates their pixels.
pub fn get_buffer(backend: &mut impl Backend, flags: i32) -> i32 {
    let _ = (backend, flags);
    todo!()
}
