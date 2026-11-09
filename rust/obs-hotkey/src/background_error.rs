//! Owned copies of the background backend's error, under the hotkey mutex.

/// Access and copying happen only while the existing mutex is held.
pub trait BackgroundError {
    type Error: Copy;
    type Owned;
    fn lock(&mut self) -> bool;
    fn error(&self) -> Option<Self::Error>;
    fn duplicate(&mut self, error: Self::Error) -> Self::Owned;
    fn unlock(&mut self);
}

/// Return no error if locking fails or the backend has none.
pub fn query<B: BackgroundError>(_background: &mut B) -> Option<B::Owned> {
    todo!("port background error getter")
}
