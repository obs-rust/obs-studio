//! Ordered replacement of the six audio and two scene-item descriptions.

/// Storage owns each duplicated value and releases it before replacement.
pub trait Storage {
    type Input: Copy;
    type Owned;
    fn release(&mut self, slot: usize);
    fn duplicate(&mut self, value: Self::Input) -> Self::Owned;
    fn write(&mut self, slot: usize, value: Self::Owned);
}

/// Replace the audio descriptions, leaving scene-item descriptions alone.
pub fn set_audio<S: Storage>(_storage: &mut S, _values: [S::Input; 6]) {
    todo!()
}

/// Replace the scene-item descriptions, leaving audio descriptions alone.
pub fn set_sceneitem<S: Storage>(_storage: &mut S, _values: [S::Input; 2]) {
    todo!()
}
