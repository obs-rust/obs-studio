//! Borrowed output-type metadata. Lookup and plugin storage remain with the host.

pub trait Info {
    type Text: Copy;
    fn name(&self) -> Self::Text;
    fn flags(&self) -> u32;
    fn video_codecs(&self) -> Self::Text;
    fn audio_codecs(&self) -> Self::Text;
}

pub fn display_name<I: Info>(_info: Option<&I>) -> Option<I::Text> {
    todo!()
}

pub fn flags<I: Info>(_info: Option<&I>) -> u32 {
    todo!()
}

pub fn video_codecs<I: Info>(_info: Option<&I>) -> Option<I::Text> {
    todo!()
}

pub fn audio_codecs<I: Info>(_info: Option<&I>) -> Option<I::Text> {
    todo!()
}
