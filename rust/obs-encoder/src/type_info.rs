//! Borrowed encoder-type metadata; registry storage and lookup belong to the host.

pub trait Info {
    type Text: Copy;
    fn name(&self) -> Self::Text;
    fn codec(&self) -> Self::Text;
}

pub fn display_name<I: Info>(info: Option<&I>) -> Option<I::Text> {
    info.map(Info::name)
}

pub fn codec<I: Info>(info: Option<&I>) -> Option<I::Text> {
    info.map(Info::codec)
}
