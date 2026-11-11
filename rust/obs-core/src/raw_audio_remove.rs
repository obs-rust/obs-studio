//! Removal delegates to the existing audio output; registration and locking stay there.

/// Access to the main output and its existing disconnect operation.
pub trait Audio {
    type Output;
    type Callback;
    type Param;

    fn main_audio(&mut self) -> Self::Output;
    fn disconnect(
        &mut self,
        output: Self::Output,
        mix: usize,
        callback: Self::Callback,
        param: Self::Param,
    );
}

/// Forward the full mix index and unchanged identities exactly once, even when
/// the initialized core has no audio output or the tuple is not registered.
pub fn remove<A: Audio>(audio: &mut A, mix: usize, callback: A::Callback, param: A::Param) {
    let output = audio.main_audio();
    audio.disconnect(output, mix, callback, param);
}
