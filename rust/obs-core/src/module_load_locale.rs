//! Locale-loading orchestration over borrowed module services.

/// Host operations used by the locale loader; paths and locales are raw bytes.
pub trait LocaleLoader {
    /// Owned lookup returned to the caller.
    type Lookup;
    /// Resolve a module-relative name and create its default lookup.
    fn create(&mut self, name: &[u8]) -> Option<Self::Lookup>;
    /// Resolve and overlay a name, returning whether the overlay loaded.
    fn add(&mut self, lookup: &mut Self::Lookup, name: &[u8]) -> bool;
    /// Report invalid parameters.
    fn invalid(&mut self);
    /// Report a failed default or overlay load.
    fn failed(&mut self, locale: &[u8]);
}

/// Load the default first, then an optional overlay, preserving C behavior.
pub fn load<L: LocaleLoader>(
    loader: &mut L,
    module: bool,
    default: Option<&[u8]>,
    locale: Option<&[u8]>,
) -> Option<L::Lookup> {
    let _ = (loader, module, default, locale);
    todo!()
}
