use crate::Error;
use crate::platform::ModuleUse;
use libloading::os::windows::Library;
use std::path::Path;
use vst3::{ComPtr, Steinberg::IPluginFactory};

pub(crate) const WINDOWS_COM_ORDER: bool = true;

type Entry = unsafe extern "system" fn() -> bool;
type Exit = unsafe extern "system" fn() -> bool;
type GetFactory = unsafe extern "system" fn() -> *mut IPluginFactory;

pub(crate) struct Module {
    factory: ComPtr<IPluginFactory>,
    _library: ActiveLibrary,
}

struct ActiveLibrary {
    _library: Library,
    exit: Option<Exit>,
}

impl Drop for ActiveLibrary {
    fn drop(&mut self) {
        if let Some(exit) = self.exit {
            // SAFETY: The module is loaded, all COM references are gone,
            // and its entry point was successful or absent.
            unsafe { exit() };
        }
    }
}

impl Module {
    pub(crate) fn open(path: &Path, _usage: ModuleUse) -> Result<Self, Error> {
        if !path.is_dir() {
            return Self::open_binary(path);
        }
        let name = path.file_name().ok_or(Error("invalid VST3 bundle"))?;
        let contents = path.join("Contents");
        if std::env::consts::ARCH == "aarch64" {
            return Self::open_binary(&contents.join("arm64-win").join(name))
                .or_else(|_| Self::open_binary(&contents.join("arm64x-win").join(name)));
        }
        Self::open_binary(
            &contents
                .join(format!("{}-win", std::env::consts::ARCH))
                .join(name),
        )
    }

    fn open_binary(path: &Path) -> Result<Self, Error> {
        // SAFETY: The caller explicitly loads a VST3 DLL. The mandatory
        // factory and optional lifecycle exports use the SDK's Windows ABI.
        unsafe {
            let library = Library::new(path).map_err(|_| Error("unable to open VST3 DLL"))?;
            let get_factory = *library
                .get::<GetFactory>(b"GetPluginFactory\0")
                .map_err(|_| Error("missing GetPluginFactory"))?;
            let entry = library
                .get::<Entry>(b"InitDll\0")
                .ok()
                .map(|symbol| *symbol);
            let exit = library.get::<Exit>(b"ExitDll\0").ok().map(|symbol| *symbol);
            if let Some(entry) = entry
                && !entry()
            {
                return Err(Error("InitDll rejected initialization"));
            }
            let library = ActiveLibrary {
                _library: library,
                exit,
            };
            let factory = ComPtr::from_raw(get_factory()).ok_or(Error("null VST3 factory"))?;
            Ok(Self {
                factory,
                _library: library,
            })
        }
    }

    pub(crate) fn factory(&self) -> &ComPtr<IPluginFactory> {
        &self.factory
    }
}
