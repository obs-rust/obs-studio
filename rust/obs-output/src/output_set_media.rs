//! Media assignment rules, preserving the C output's flag gates and diagnostics.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Diagnostic {
    NullOutput,
    EncodedOutput,
}

pub struct Media<'a, T> {
    pub flags: u32,
    pub video: &'a mut T,
    pub audio: &'a mut T,
}

pub fn set_media<T>(output: Option<Media<'_, T>>, video: T, audio: T, log: impl FnOnce(Diagnostic)) {
    todo!()
}
