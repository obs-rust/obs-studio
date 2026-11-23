use crate::Error;
use std::ffi::{CStr, c_char, c_void};
use std::mem;
use std::path::Path;
use std::ptr::{self, NonNull};
use vst3::{ComPtr, Steinberg::IPluginFactory};

pub(crate) const WINDOWS_COM_ORDER: bool = false;

type CfRef = *const c_void;
type Entry = unsafe extern "C" fn(CfRef) -> bool;
type Exit = unsafe extern "C" fn() -> bool;
type GetFactory = unsafe extern "system" fn() -> *mut IPluginFactory;

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFURLCreateFromFileSystemRepresentation(
        allocator: CfRef,
        bytes: *const u8,
        length: isize,
        is_directory: u8,
    ) -> CfRef;
    fn CFStringCreateWithCString(allocator: CfRef, bytes: *const c_char, encoding: u32) -> CfRef;
    fn CFBundleCreate(allocator: CfRef, url: CfRef) -> CfRef;
    fn CFBundleLoadExecutable(bundle: CfRef) -> u8;
    fn CFBundleGetFunctionPointerForName(bundle: CfRef, name: CfRef) -> CfRef;
    fn CFRelease(value: CfRef);
}

struct OwnedCf(NonNull<c_void>);

impl OwnedCf {
    fn from_created(value: CfRef) -> Option<Self> {
        NonNull::new(value.cast_mut()).map(Self)
    }

    fn as_ptr(&self) -> CfRef {
        self.0.as_ptr()
    }
}

impl Drop for OwnedCf {
    fn drop(&mut self) {
        // SAFETY: The wrapper owns exactly one non-null Core Foundation reference.
        unsafe { CFRelease(self.as_ptr()) };
    }
}

pub(crate) struct Module {
    factory: ComPtr<IPluginFactory>,
    _library: ActiveBundle,
}

struct ActiveBundle {
    _bundle: OwnedCf,
    exit: Exit,
}

impl Drop for ActiveBundle {
    fn drop(&mut self) {
        // SAFETY: bundleEntry succeeded. SDK interfaces released first and
        // the owning Core Foundation bundle remains live through bundleExit.
        unsafe { (self.exit)() };
    }
}

impl Module {
    pub(crate) fn open(path: &Path) -> Result<Self, Error> {
        let bytes = path.as_os_str().as_encoded_bytes();
        let length = isize::try_from(bytes.len()).map_err(|_| Error("invalid VST3 bundle path"))?;
        // SAFETY: The CF create functions borrow valid path bytes and return
        // owned references. All SDK entry points are resolved from this bundle.
        unsafe {
            let url = OwnedCf::from_created(CFURLCreateFromFileSystemRepresentation(
                ptr::null(),
                bytes.as_ptr(),
                length,
                1,
            ))
            .ok_or(Error("could not construct VST3 bundle URL"))?;
            let bundle = OwnedCf::from_created(CFBundleCreate(ptr::null(), url.as_ptr()))
                .ok_or(Error("could not create VST3 bundle"))?;
            if CFBundleLoadExecutable(bundle.as_ptr()) == 0 {
                return Err(Error("could not load the VST3 bundle executable"));
            }
            let entry: Entry = mem::transmute(
                symbol(&bundle, c"bundleEntry")
                    .ok_or(Error("missing bundleEntry"))?
                    .as_ptr(),
            );
            let exit: Exit = mem::transmute(
                symbol(&bundle, c"bundleExit")
                    .ok_or(Error("missing bundleExit"))?
                    .as_ptr(),
            );
            let get_factory: GetFactory = mem::transmute(
                symbol(&bundle, c"GetPluginFactory")
                    .ok_or(Error("missing GetPluginFactory"))?
                    .as_ptr(),
            );
            if !entry(bundle.as_ptr()) {
                return Err(Error("bundleEntry rejected initialization"));
            }
            let library = ActiveBundle {
                _bundle: bundle,
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

fn symbol(bundle: &OwnedCf, name: &CStr) -> Option<NonNull<c_void>> {
    const UTF8: u32 = 0x08000100;
    // SAFETY: The live bundle and NUL-terminated symbol name outlive this call.
    unsafe {
        let name =
            OwnedCf::from_created(CFStringCreateWithCString(ptr::null(), name.as_ptr(), UTF8))?;
        NonNull::new(CFBundleGetFunctionPointerForName(bundle.as_ptr(), name.as_ptr()).cast_mut())
    }
}
