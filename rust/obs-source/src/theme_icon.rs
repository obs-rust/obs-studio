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

pub fn get<R: Registry>(registry: &R, theme: Theme) -> Option<R::Icon> {
    if registry.icon_type() != 13 {
        return None;
    }
    let info = registry.source_info()?;
    registry.call(info, theme)
}
