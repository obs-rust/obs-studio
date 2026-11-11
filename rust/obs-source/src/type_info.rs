//! Borrowed source-type metadata; the host owns registry lookup and storage.

pub trait Info {
    type Text: Copy;
    fn name(&self) -> Self::Text;
    fn flags(&self) -> u32;
    fn has_properties(&self) -> bool;
    fn has_properties2(&self) -> bool;
}

pub fn display_name<I: Info>(_info: Option<&I>) -> Option<I::Text> {
    todo!()
}

pub fn output_flags<I: Info>(_info: Option<&I>) -> u32 {
    todo!()
}

pub fn configurable<I: Info>(_info: Option<&I>) -> bool {
    todo!()
}
