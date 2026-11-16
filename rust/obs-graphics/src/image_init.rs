//! Image decoder lifecycle; FFmpeg ownership and layouts stay in the private C adapter.

/// Private adapter operations. Values are shared with graphics-ffmpeg-init-adapter.c.h.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Operation {
    Reset,
    Open,
    StreamCount,
    FirstVideo,
    FirstKnown,
    Probe,
    BestStream,
    FindDecoder,
    TooLarge,
    Allocate,
    Configure,
    OpenCodec,
    FreeCodec,
    CloseFormat,
    WarnOpen,
    WarnProbe,
    WarnStream,
    WarnDecoder,
    WarnCodec,
    BufferTooLarge,
    DefaultBuffer,
}

/// A decoder session. Each operation is one FFmpeg call or private field access.
pub trait Backend {
    fn run(&mut self, operation: Operation, value: i32) -> i32;
}

/// Initialize a nonempty local image path, retaining successful handles until free.
pub fn init(backend: &mut impl Backend, nonempty: bool) -> bool {
    use Operation::{FirstKnown, FirstVideo, Open, Probe, Reset, StreamCount, WarnOpen, WarnProbe};
    if !nonempty {
        return false;
    }
    backend.run(Reset, 0);
    let ret = backend.run(Open, 0);
    if ret < 0 {
        backend.run(WarnOpen, ret);
        return false;
    }
    let known = backend.run(StreamCount, 0) == 1
        && backend.run(FirstVideo, 0) != 0
        && backend.run(FirstKnown, 0) != 0;
    if !known {
        let ret = backend.run(Probe, 0);
        if ret < 0 {
            backend.run(WarnProbe, ret);
            free(backend);
            return false;
        }
    }
    let success = open_decoder(backend);
    if !success {
        free(backend);
    }
    success
}

fn open_decoder(backend: &mut impl Backend) -> bool {
    use Operation::{
        Allocate, BestStream, Configure, FindDecoder, OpenCodec, TooLarge, WarnCodec, WarnDecoder,
        WarnStream,
    };
    let stream = backend.run(BestStream, 0);
    if stream < 0 {
        backend.run(WarnStream, stream);
        return false;
    }
    if backend.run(FindDecoder, stream) == 0 {
        backend.run(WarnDecoder, 0);
        return false;
    }
    if backend.run(TooLarge, stream) != 0 {
        return false;
    }
    backend.run(Allocate, 0);
    backend.run(Configure, stream);
    let ret = backend.run(OpenCodec, 0);
    if ret < 0 {
        backend.run(WarnCodec, ret);
        return false;
    }
    true
}

/// Release the codec before its owning input format.
pub fn free(backend: &mut impl Backend) {
    backend.run(Operation::FreeCodec, 0);
    backend.run(Operation::CloseFormat, 0);
}

/// Reject oversized buffers before FFmpeg allocates their pixels.
pub fn get_buffer(backend: &mut impl Backend, flags: i32) -> i32 {
    let ret = backend.run(Operation::BufferTooLarge, 0);
    if ret < 0 {
        ret
    } else {
        backend.run(Operation::DefaultBuffer, flags)
    }
}
