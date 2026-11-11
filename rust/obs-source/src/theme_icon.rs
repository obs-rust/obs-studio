//! Source-type theme icons use two separate registry lookups and borrowed results.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Theme {
    Dark,
    Light,
}

pub trait Registry {
    type Info;
    type Icon;
    fn icon_type(&self) -> u32;
    fn source_info(&self) -> Option<Self::Info>;
    fn call(&self, info: Self::Info, theme: Theme) -> Option<Self::Icon>;
}

pub fn get<R: Registry>(_registry: &R, _theme: Theme) -> Option<R::Icon> {
    todo!("source theme icon dispatch")
}
