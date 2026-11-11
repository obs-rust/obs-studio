//! Index-buffer input duplication and ownership handoff to a graphics backend.

pub const DUP_BUFFER: u32 = 1 << 4;
pub const UNSIGNED_SHORT: i32 = 0;

/// The backend owns every input passed to `create`, including on failure.
/// `duplicate` must leave the caller's original input alive and independent.
pub trait Backend {
    type Input: Copy;
    type Buffer;

    fn duplicate(&mut self, input: Self::Input, bytes: usize) -> Self::Input;
    fn create(
        &mut self,
        kind: i32,
        input: Option<Self::Input>,
        count: usize,
        flags: u32,
    ) -> Self::Buffer;
}

/// No context means no duplication or dispatch. Null input and zero counts
/// still dispatch when a context exists, matching the original C wrapper.
pub fn create<B: Backend>(
    backend: Option<&mut B>,
    kind: i32,
    input: Option<B::Input>,
    count: usize,
    flags: u32,
) -> Option<B::Buffer> {
    let backend = backend?;
    let input = input.map(|input| {
        if count != 0 && flags & DUP_BUFFER != 0 {
            let width = if kind == UNSIGNED_SHORT { 2usize } else { 4 };
            // Match unsigned C size_t multiplication, including wraparound.
            backend.duplicate(input, count.wrapping_mul(width))
        } else {
            input
        }
    });
    Some(backend.create(kind, input, count, flags))
}
