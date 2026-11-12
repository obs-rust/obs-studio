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
pub fn set_audio<S: Storage>(storage: &mut S, values: [S::Input; 6]) {
    replace(storage, 0, values);
}

/// Replace the scene-item descriptions, leaving audio descriptions alone.
pub fn set_sceneitem<S: Storage>(storage: &mut S, values: [S::Input; 2]) {
    replace(storage, 6, values);
}

fn replace<S: Storage, const N: usize>(storage: &mut S, start: usize, values: [S::Input; N]) {
    for (index, value) in values.into_iter().enumerate() {
        let slot = start + index;
        storage.release(slot);
        let owned = storage.duplicate(value);
        storage.write(slot, owned);
    }
}
