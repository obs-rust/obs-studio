//! Device-query decisions, independent of native graphics layouts.
use std::ffi::CStr;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Entry {
    DeviceType,
    AdapterCount,
}

impl Entry {
    pub const fn name(self) -> &'static str {
        match self {
            Self::DeviceType => "gs_get_device_type",
            Self::AdapterCount => "gs_get_adapter_count",
        }
    }

    pub const fn c_name(self) -> &'static CStr {
        match self {
            Self::DeviceType => c"gs_get_device_type",
            Self::AdapterCount => c"gs_get_adapter_count",
        }
    }
}

/// Native field access and dispatch are reached only with a current context.
pub trait Host {
    fn has_context(&self) -> bool;
    fn debug(&mut self, entry: Entry);
    fn device_type(&mut self) -> i32;
    fn has_adapter_count(&self) -> bool;
    fn adapter_count(&mut self) -> u32;
}

/// `None` means no context; `Some(-1)` is an actual backend result.
pub fn get_device_type<H: Host>(_host: &mut H) -> Option<i32> {
    todo!("#5070: device type")
}

/// `None` means no context; a missing optional callback yields `Some(0)`.
pub fn get_adapter_count<H: Host>(_host: &mut H) -> Option<u32> {
    todo!("#5070: adapter count")
}
