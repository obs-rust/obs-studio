//! Locale replacement and ordered module notification over host-owned storage.

/// Access to C-owned storage without exposing its layout to the safe core.
pub trait Host {
    type Locale: Copy;
    type Owned: Copy;
    type Module: Copy;
    type Callback;
    fn owned_locale(&mut self) -> Option<Self::Owned>;
    fn free_locale(&mut self, owned: Self::Owned);
    fn copy_locale(&mut self, locale: Self::Locale) -> Self::Owned;
    fn store_locale(&mut self, owned: Self::Owned);
    fn first_module(&mut self) -> Option<Self::Module>;
    fn callback(&mut self, module: Self::Module) -> Option<Self::Callback>;
    fn notify(&mut self, callback: Self::Callback, locale: Self::Locale);
    fn next_module(&mut self, module: Self::Module) -> Option<Self::Module>;
}

/// Free before copying, store before notifying, and read each next link after
/// its callback. Notifications receive the original argument, including NULL.
pub fn set_locale<H: Host>(_host: &mut H, _locale: H::Locale) {
    todo!("Refs #5042: locale swap")
}
