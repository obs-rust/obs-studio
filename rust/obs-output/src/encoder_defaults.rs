//! Fresh encoder defaults, independent of private C layouts and settings storage.

/// A borrowed encoder callback table; the host creates caller-owned settings.
pub trait Host {
    type Settings;
    type TypeData;
    fn create_settings(&self) -> Self::Settings;
    fn has_legacy(&self) -> bool;
    fn has_modern(&self) -> bool;
    fn legacy(&self, settings: &Self::Settings);
    fn type_data(&self) -> Self::TypeData;
    fn modern(&self, settings: &Self::Settings, type_data: Self::TypeData);
}

/// Unknown types return no settings; missing instances also emit their diagnostic.
pub fn query<H: Host>(host: Option<&H>, null_instance: impl FnOnce()) -> Option<H::Settings> {
    todo!()
}
