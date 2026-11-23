use crate::Error;
use libloading::os::unix::Library;
use std::ffi::c_void;
use std::path::Path;
use vst3::ComPtr;
use vst3::Steinberg::IPluginFactory;

type Exit = unsafe extern "system" fn() -> bool;
type Entry = unsafe extern "system" fn(*mut c_void) -> bool;
type GetFactory = unsafe extern "system" fn() -> *mut IPluginFactory;

pub(crate) const WINDOWS_COM_ORDER: bool = false;

pub(crate) struct Module {
    factory: ComPtr<IPluginFactory>,
    _library: ActiveLibrary,
}

struct ActiveLibrary {
    _library: Library,
    exit: Exit,
}

impl Drop for ActiveLibrary {
    fn drop(&mut self) {
        // SAFETY: ModuleEntry succeeded and the owning library is still live.
        // Module declares its factory before this guard so COM interfaces are
        // released before ModuleExit or dlclose.
        unsafe { (self.exit)() };
    }
}

impl Module {
    pub(crate) fn open(bundle: &Path) -> Result<Self, Error> {
        let mut binary = bundle
            .file_stem()
            .ok_or(Error("invalid VST3 bundle"))?
            .to_owned();
        binary.push(".so");
        let file = bundle
            .join("Contents")
            .join(format!("{}-linux", std::env::consts::ARCH))
            .join(binary);
        // SAFETY: The caller explicitly loads a VST3 plug-in. These symbols
        // match the pinned SDK loader and are used only while its handle lives.
        unsafe {
            let library = Library::new(file).map_err(|_| Error("unable to open VST3 module"))?;
            let handle = library.into_raw();
            let library = Library::from_raw(handle);
            let entry = *library
                .get::<Entry>(b"ModuleEntry\0")
                .map_err(|_| Error("missing ModuleEntry"))?;
            let exit = *library
                .get::<Exit>(b"ModuleExit\0")
                .map_err(|_| Error("missing ModuleExit"))?;
            let get_factory = *library
                .get::<GetFactory>(b"GetPluginFactory\0")
                .map_err(|_| Error("missing GetPluginFactory"))?;
            if !entry(handle) {
                return Err(Error("ModuleEntry rejected initialization"));
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
