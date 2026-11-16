//! Borrowed output-type metadata. Lookup and plugin storage remain with the host.

pub trait Info {
    type Text: Copy;
    fn name(&self) -> Self::Text;
    fn flags(&self) -> u32;
    fn video_codecs(&self) -> Self::Text;
    fn audio_codecs(&self) -> Self::Text;
}

pub fn display_name<I: Info>(info: Option<&I>) -> Option<I::Text> {
    info.map(Info::name)
}

pub fn flags<I: Info>(info: Option<&I>) -> u32 {
    info.map_or(0, Info::flags)
}

pub fn video_codecs<I: Info>(info: Option<&I>) -> Option<I::Text> {
    info.map(Info::video_codecs)
}

pub fn audio_codecs<I: Info>(info: Option<&I>) -> Option<I::Text> {
    info.map(Info::audio_codecs)
}
