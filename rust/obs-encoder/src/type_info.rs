//! Borrowed encoder-type metadata; registry storage and lookup belong to the host.

pub trait Info {
    type Text: Copy;
    fn name(&self) -> Self::Text;
    fn codec(&self) -> Self::Text;
}

pub fn display_name<I: Info>(_info: Option<&I>) -> Option<I::Text> {
    todo!("encoder display name")
}

pub fn codec<I: Info>(_info: Option<&I>) -> Option<I::Text> {
    todo!("encoder codec")
}
