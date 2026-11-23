//! Registration delegates to the existing audio output; the duplicate check,
//! the mix range check and locking stay there.

/// Access to the main output and its existing connect operation.
pub trait Audio {
    type Output;
    type Conversion;
    type Callback;
    type Param;

    fn main_audio(&mut self) -> Self::Output;
    fn connect(
        &mut self,
        output: Self::Output,
        mix: usize,
        conversion: Self::Conversion,
        callback: Self::Callback,
        param: Self::Param,
    );
}

/// Forward the full mix index, the conversion and unchanged identities exactly
/// once, even when the initialized core has no audio output, the mix is out
/// of range or the tuple is already registered. The connect result is ignored.
pub fn add<A: Audio>(
    audio: &mut A,
    mix: usize,
    conversion: A::Conversion,
    callback: A::Callback,
    param: A::Param,
) {
    let output = audio.main_audio();
    audio.connect(output, mix, conversion, callback, param);
}
