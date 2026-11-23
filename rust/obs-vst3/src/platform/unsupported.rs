use crate::Error;
use crate::platform::ModuleUse;
use std::path::Path;
use vst3::{ComPtr, Steinberg::IPluginFactory};

pub(crate) const WINDOWS_COM_ORDER: bool = false;

pub(crate) enum Module {}

impl Module {
    pub(crate) fn open(_path: &Path, _usage: ModuleUse) -> Result<Self, Error> {
        Err(Error("VST3 modules are unavailable on this host"))
    }

    pub(crate) fn factory(&self) -> &ComPtr<IPluginFactory> {
        match *self {}
    }
}
