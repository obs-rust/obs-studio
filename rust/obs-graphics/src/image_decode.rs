//! Still-image decode orchestration. FFmpeg ownership and layouts stay in the
//! private C adapter; this core controls call order and failure handling.

/// Result of sending a packet or receiving a frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    /// The operation succeeded.
    Ready,
    /// FFmpeg returned EOF or EAGAIN. Preserve the original retry behavior.
    Retry,
    /// Another FFmpeg error.
    Error(i32),
}

/// Warning emitted by the original decoder.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Warning {
    /// Frame allocation failed.
    Allocate,
    /// Reading the first packet failed.
    Read(i32),
    /// Sending or receiving failed.
    Decode(i32),
}

/// Private decoder operations. Implementations own their frame and packet.
pub trait Decoder {
    /// Converted image data, owned by the caller after return.
    type Output;
    /// Allocate a frame and a zeroed packet.
    fn allocate(&mut self) -> bool;
    /// Read exactly one image packet.
    fn read(&mut self) -> Result<(), i32>;
    /// Send that same packet, including on retry.
    fn send(&mut self) -> Status;
    /// Receive into the allocated frame.
    fn receive(&mut self) -> Status;
    /// Whether an earlier size check already logged a rejection.
    fn too_large(&mut self) -> bool;
    /// Validate decoded dimensions, logging the size rejection if needed.
    fn check_size(&mut self) -> bool;
    /// Store the decoded dimensions and format in the image state.
    fn update(&mut self);
    /// Convert the frame using the caller's alpha mode.
    fn reformat(&mut self) -> Self::Output;
    /// Emit an original warning.
    fn warn(&mut self, warning: Warning);
    /// Unreference the packet and free the frame, in that order.
    fn cleanup(&mut self);
}

/// Decode one image, preserving the original packet/frame lifecycle.
pub fn decode<D: Decoder>(_decoder: &mut D) -> Option<D::Output> {
    todo!()
}
