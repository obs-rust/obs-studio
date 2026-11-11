//! Borrowed source-type metadata; the host owns registry lookup and storage.

pub trait Info {
    type Text: Copy;
    fn name(&self) -> Self::Text;
    fn flags(&self) -> u32;
    fn has_properties(&self) -> bool;
    fn has_properties2(&self) -> bool;
}

pub fn display_name<I: Info>(info: Option<&I>) -> Option<I::Text> {
    info.map(Info::name)
}

pub fn output_flags<I: Info>(info: Option<&I>) -> u32 {
    info.map_or(0, Info::flags)
}

pub fn configurable<I: Info>(info: Option<&I>) -> bool {
    info.is_some_and(|info| info.has_properties() || info.has_properties2())
}
