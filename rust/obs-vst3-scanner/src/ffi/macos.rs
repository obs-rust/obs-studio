use crate::cache::Class;
use crate::ffi::{self, GetFactory};
use std::ffi::{CStr, c_char, c_void};
use std::mem;
use std::path::{Path, PathBuf};
use std::ptr::{self, NonNull};

pub(crate) const WINDOWS_COM_ORDER: bool = false;

type CFRef = *const c_void;

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFURLCreateFromFileSystemRepresentation(
        allocator: CFRef,
        bytes: *const u8,
        length: isize,
        is_directory: u8,
    ) -> CFRef;
    fn CFStringCreateWithCString(allocator: CFRef, bytes: *const c_char, encoding: u32) -> CFRef;
    fn CFBundleCreate(allocator: CFRef, url: CFRef) -> CFRef;
    fn CFBundleLoadExecutable(bundle: CFRef) -> u8;
    fn CFBundleUnloadExecutable(bundle: CFRef);
    fn CFBundleGetFunctionPointerForName(bundle: CFRef, name: CFRef) -> CFRef;
    fn CFRelease(value: CFRef);
}

struct OwnedCf(NonNull<c_void>);

impl OwnedCf {
    fn from_created(value: CFRef) -> Option<Self> {
        NonNull::new(value.cast_mut()).map(Self)
    }

    fn ptr(&self) -> CFRef {
        self.0.as_ptr()
    }
}

impl Drop for OwnedCf {
    fn drop(&mut self) {
        // SAFETY: This value owns a non-null reference returned by a CF Create call.
        unsafe { CFRelease(self.ptr()) };
    }
}

struct Bundle(OwnedCf);

impl Drop for Bundle {
    fn drop(&mut self) {
        // SAFETY: The bundle loaded successfully and all factory interfaces dropped.
        unsafe { CFBundleUnloadExecutable(self.0.ptr()) };
    }
}

impl Bundle {
    fn load(path: &Path) -> Option<Self> {
        let bytes = path.as_os_str().as_encoded_bytes();
        let length = isize::try_from(bytes.len()).ok()?;
        // SAFETY: CoreFoundation reads length bytes from the live path and
        // all returned Create references remain owned for their use.
        unsafe {
            let url = OwnedCf::from_created(CFURLCreateFromFileSystemRepresentation(
                ptr::null(),
                bytes.as_ptr(),
                length,
                1,
            ))?;
            let bundle = OwnedCf::from_created(CFBundleCreate(ptr::null(), url.ptr()))?;
            if CFBundleLoadExecutable(bundle.ptr()) == 0 {
                return None;
            }
            Some(Self(bundle))
        }
    }

    fn symbol(&self, name: &CStr) -> Option<NonNull<c_void>> {
        const UTF8_ENCODING: u32 = 0x08000100;
        // SAFETY: name is a live NUL-terminated string and the bundle is loaded.
        unsafe {
            let name = OwnedCf::from_created(CFStringCreateWithCString(
                ptr::null(),
                name.as_ptr(),
                UTF8_ENCODING,
            ))?;
            NonNull::new(CFBundleGetFunctionPointerForName(self.0.ptr(), name.ptr()).cast_mut())
        }
    }
}

pub(crate) fn prepare_process() {}

pub(crate) fn search_roots() -> Vec<PathBuf> {
    let mut roots = vec![PathBuf::from("/Library/Audio/Plug-Ins/VST3")];
    roots.extend(
        std::env::var_os("HOME")
            .map(|home| PathBuf::from(home).join("Library/Audio/Plug-Ins/VST3")),
    );
    roots
}

pub(crate) fn is_candidate(path: &Path) -> bool {
    path.is_dir()
}

pub(crate) fn metadata_root(path: &Path) -> Option<&Path> {
    Some(path)
}

pub(crate) fn enumerate(path: &Path) -> Vec<Class> {
    let Some(bundle) = Bundle::load(path) else {
        return Vec::new();
    };
    let (Some(entry), Some(exit), Some(factory)) = (
        bundle.symbol(c"bundleEntry"),
        bundle.symbol(c"bundleExit"),
        bundle.symbol(c"GetPluginFactory"),
    ) else {
        return Vec::new();
    };
    // SAFETY: These named symbols follow the SDK's module_mac.mm declarations.
    // The loaded CoreFoundation bundle outlives these calls and the factory.
    unsafe {
        let entry: unsafe extern "C" fn(CFRef) -> bool = mem::transmute(entry.as_ptr());
        let exit: unsafe extern "C" fn() -> bool = mem::transmute(exit.as_ptr());
        let factory: GetFactory = mem::transmute(factory.as_ptr());
        if !entry(bundle.0.ptr()) {
            return Vec::new();
        }
        let classes = ffi::classes(factory(), path);
        exit();
        classes
    }
}
