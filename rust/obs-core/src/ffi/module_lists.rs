//! C ABI for `libobs/obs-module-lists.c.h`.
//!
//! The ten module list calls are `EXPORT` in `obs.h`, so the shims keep
//! their names and libobs exports them (`libobs/cmake/rust-exports.d`).
//! Every field and list of the core and of a module is reached through the
//! module list seam (`libobs/obs-module-lists-seam.h`, live C in libobs), so
//! nothing here mirrors `struct obs_core` or `struct obs_module`; the one
//! structure mirrored is the search path the seam's list holds,
//! [`obs_module_path`].
//!
//! The lists own their strings, and `free_module()` and the core's shutdown
//! `bfree()` them, so each copy is a `bmalloc`'d `bstrdup()` (one
//! allocation, as the C's) and each push is `darray_push_back()` with the
//! typed list's element size (obs-util's port of `darray.h`, allocating
//! through bmem as the C does). A NULL id pushes a NULL entry, as the C's
//! `bstrdup(NULL)`. The paths are `bmalloc`'d too, and the caller `bfree`s
//! them. No call takes a lock, as the C takes none.
//!
//! The file and config paths come from the Rust cores that libobs's own
//! `obs_module_paths_find_file()` and `obs_module_names_config_path()` run
//! (libobs's `os_file_exists()` for the file system), and the log lines go
//! to `blog()` with the C's format strings, the module's file read for its
//! line.
//!
//! Intentional difference: `obs_log_loaded_modules()` with no core logs
//! nothing (the C logs the heading, then crashes reading the core's first
//! module).

use core::ffi::{CStr, c_char, c_int, c_void};
use core::marker::PhantomData;
use core::mem::size_of;
use core::ptr::{self, NonNull};

use obs_util::ffi::darray::{darray, darray_push_back};

use super::module_paths::{LibobsFs, bstrdup, bytes};
use crate::module_lists::{self, Line, Lists, NameList, TypeList};

/// `struct obs_module_path` (`obs-internal.h`): an entry of the core's
/// search paths.
#[repr(C)]
#[allow(non_camel_case_types)]
#[derive(Debug)]
pub struct obs_module_path {
    pub bin: *mut c_char,
    pub data: *mut c_char,
    /// `enum obs_runtime_module_type`.
    pub module_type: c_int,
}

/// `OBS_MODULE_TYPE_UNDEFINED`: the type of a search path from
/// `obs_add_module_path()`, which predates module types.
pub const OBS_MODULE_TYPE_UNDEFINED: c_int = 0;

/// `LOG_INFO`.
pub const LOG_INFO: c_int = 300;

/// The heading's format, [`module_lists::HEADING`].
pub const HEADING_FORMAT: &CStr = c"  Loaded Modules:";

/// A module's line's format, [`module_lists::MODULE_FORMAT`].
pub const MODULE_FORMAT: &CStr = c"    %s";

/// `blog()`'s type.
pub type Blog = unsafe extern "C" fn(level: c_int, format: *const c_char, ...);

/// The libobs call the shims make besides the seam: `blog()`. The parity
/// test passes the C oracle's stand-in, which the oracle's own copy of the
/// original file logs through too.
pub trait Calls {
    /// `blog`.
    fn blog() -> Blog;
}

/// [`Calls`] through libobs's own `blog()`.
pub enum Libobs {}

unsafe extern "C" {
    fn blog(level: c_int, format: *const c_char, ...);

    fn obs_module_lists_seam_has_core() -> bool;
    fn obs_module_lists_seam_first_module() -> *mut c_void;
    fn obs_module_lists_seam_module_config_path() -> *const c_char;
    fn obs_module_lists_seam_module_paths() -> *mut darray;
    fn obs_module_lists_seam_safe_modules() -> *mut darray;
    fn obs_module_lists_seam_disabled_modules() -> *mut darray;

    fn obs_module_lists_seam_module_next(module: *const c_void) -> *mut c_void;
    fn obs_module_lists_seam_module_file(module: *const c_void) -> *const c_char;
    fn obs_module_lists_seam_module_data_path(module: *const c_void) -> *const c_char;
    fn obs_module_lists_seam_module_name(module: *const c_void) -> *const c_char;
    fn obs_module_lists_seam_module_sources(module: *mut c_void) -> *mut darray;
    fn obs_module_lists_seam_module_outputs(module: *mut c_void) -> *mut darray;
    fn obs_module_lists_seam_module_encoders(module: *mut c_void) -> *mut darray;
    fn obs_module_lists_seam_module_services(module: *mut c_void) -> *mut darray;
}

impl Calls for Libobs {
    fn blog() -> Blog {
        blog
    }
}

/// A non-NULL `obs_module_t *`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Module(NonNull<c_void>);

impl Module {
    fn from_ptr(module: *mut c_void) -> Option<Self> {
        NonNull::new(module).map(Self)
    }
}

/// The core and its modules through the seam.
///
/// Its reads are valid only for a core and modules libobs keeps alive
/// during the call, which is the module list calls' own contract.
pub struct Seam<C>(PhantomData<C>);

impl<C> Seam<C> {
    /// # Safety
    ///
    /// For the life of the value, the seam's reads of the core are valid
    /// whenever [`Lists::has_core`] says there is one (and
    /// [`Lists::module_config_path`] whenever it is called, as the C's
    /// read), every module handed to it is a live `obs_module_t`, and
    /// libobs changes none of the lists it reads or pushes to.
    pub unsafe fn new() -> Self {
        Self(PhantomData)
    }
}

/// Pushes `item` onto the list at `list` of `T`s.
///
/// # Safety
///
/// `list` is a valid `struct darray` of `T`s.
unsafe fn push<T>(list: *mut darray, item: &T) {
    // SAFETY: the caller guarantees the list; `item` is one readable `T`.
    unsafe { darray_push_back(size_of::<T>(), list, ptr::from_ref(item).cast()) };
}

impl<C: Calls> Lists for Seam<C> {
    type Module = Module;

    fn has_core(&self) -> bool {
        // SAFETY: reads only libobs's global pointer.
        unsafe { obs_module_lists_seam_has_core() }
    }

    fn first_module(&self) -> Option<Module> {
        // SAFETY: the core is live (see `new`).
        Module::from_ptr(unsafe { obs_module_lists_seam_first_module() })
    }

    fn module_config_path(&self) -> Option<&[u8]> {
        // SAFETY: the core is live (see `new`), and its config path is
        // NULL or a NUL-terminated string it keeps.
        unsafe { bytes(obs_module_lists_seam_module_config_path()) }
    }

    fn next(&self, module: Module) -> Option<Module> {
        // SAFETY: a live module (see `new`).
        Module::from_ptr(unsafe { obs_module_lists_seam_module_next(module.0.as_ptr()) })
    }

    fn data_path(&self, module: Module) -> Option<&[u8]> {
        // SAFETY: a live module, whose data path is NULL or a
        // NUL-terminated string it keeps.
        unsafe { bytes(obs_module_lists_seam_module_data_path(module.0.as_ptr())) }
    }

    fn module_name(&self, module: Module) -> Option<&[u8]> {
        // SAFETY: a live module, whose name is NULL or a NUL-terminated
        // string it keeps.
        unsafe { bytes(obs_module_lists_seam_module_name(module.0.as_ptr())) }
    }

    fn log(&mut self, line: Line<Module>) {
        let blog = C::blog();
        match line {
            // SAFETY: a format with no conversion.
            Line::Heading => unsafe { blog(LOG_INFO, HEADING_FORMAT.as_ptr()) },
            Line::Module(module) => {
                // SAFETY: a live module; its file goes to "%s" as the C's
                // does, NULL included.
                unsafe {
                    let file = obs_module_lists_seam_module_file(module.0.as_ptr());
                    blog(LOG_INFO, MODULE_FORMAT.as_ptr(), file);
                }
            }
        }
    }

    fn push_type(&mut self, module: Module, list: TypeList, id: Option<&[u8]>) {
        let module = module.0.as_ptr();
        // SAFETY: a live module; each is its `DARRAY(char *)`.
        let list = unsafe {
            match list {
                TypeList::Sources => obs_module_lists_seam_module_sources(module),
                TypeList::Outputs => obs_module_lists_seam_module_outputs(module),
                TypeList::Encoders => obs_module_lists_seam_module_encoders(module),
                TypeList::Services => obs_module_lists_seam_module_services(module),
            }
        };
        let copy = id.map_or(ptr::null_mut(), bstrdup);
        // SAFETY: a list of `char *`.
        unsafe { push(list, &copy) };
    }

    fn push_name(&mut self, list: NameList, name: &[u8]) {
        // SAFETY: the core is live (`has_core`); each is its
        // `DARRAY(char *)`.
        let list = unsafe {
            match list {
                NameList::Safe => obs_module_lists_seam_safe_modules(),
                NameList::Disabled => obs_module_lists_seam_disabled_modules(),
            }
        };
        let copy = bstrdup(name);
        // SAFETY: a list of `char *`.
        unsafe { push(list, &copy) };
    }

    fn push_path(&mut self, bin: &[u8], data: &[u8]) {
        let omp = obs_module_path {
            bin: bstrdup(bin),
            data: bstrdup(data),
            module_type: OBS_MODULE_TYPE_UNDEFINED,
        };
        // SAFETY: the core is live (`has_core`); its search paths are a
        // `DARRAY(struct obs_module_path)`.
        unsafe { push(obs_module_lists_seam_module_paths(), &omp) };
    }
}

fn module(module: *mut c_void) -> Option<Module> {
    Module::from_ptr(module)
}

fn into_c(path: Option<Vec<u8>>) -> *mut c_char {
    path.map_or(ptr::null_mut(), |p| bstrdup(&p))
}

/// `obs_log_loaded_modules()`, logging through `C`.
///
/// # Safety
///
/// The core, when there is one, and its modules live for the call.
pub unsafe fn log_loaded_modules_with<C: Calls>() {
    // SAFETY: the caller's contract.
    let mut seam = unsafe { Seam::<C>::new() };
    module_lists::log_loaded_modules(&mut seam);
}

/// The `obs_module_add_*()` call of `list`.
///
/// # Safety
///
/// `module` is null or a live `obs_module_t`; `id` is null or
/// NUL-terminated.
unsafe fn add_type(module: *mut c_void, list: TypeList, id: *const c_char) {
    // SAFETY: the caller's contract.
    let (mut seam, id) = unsafe { (Seam::<Libobs>::new(), bytes(id)) };
    module_lists::add_type(&mut seam, self::module(module), list, id);
}

/// `obs_add_safe_module()` or `obs_add_disabled_module()`.
///
/// # Safety
///
/// `name` is null or NUL-terminated.
unsafe fn add_name(list: NameList, name: *const c_char) {
    // SAFETY: the caller's contract.
    let (mut seam, name) = unsafe { (Seam::<Libobs>::new(), bytes(name)) };
    module_lists::add_name(&mut seam, list, name);
}

/// Same contract as C `obs_log_loaded_modules`, but with no core it logs
/// nothing.
///
/// # Safety
///
/// Called as libobs's: the core, when there is one, and its modules live
/// for the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_log_loaded_modules() {
    // SAFETY: the caller's contract.
    unsafe { log_loaded_modules_with::<Libobs>() }
}

/// Same contract as C `obs_module_add_source`.
///
/// # Safety
///
/// `module` is null or a live `obs_module_t`; `id` is null or
/// NUL-terminated.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_module_add_source(module: *mut c_void, id: *const c_char) {
    // SAFETY: the caller's contract.
    unsafe { add_type(module, TypeList::Sources, id) }
}

/// Same contract as C `obs_module_add_output`.
///
/// # Safety
///
/// As [`obs_module_add_source`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_module_add_output(module: *mut c_void, id: *const c_char) {
    // SAFETY: the caller's contract.
    unsafe { add_type(module, TypeList::Outputs, id) }
}

/// Same contract as C `obs_module_add_encoder`.
///
/// # Safety
///
/// As [`obs_module_add_source`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_module_add_encoder(module: *mut c_void, id: *const c_char) {
    // SAFETY: the caller's contract.
    unsafe { add_type(module, TypeList::Encoders, id) }
}

/// Same contract as C `obs_module_add_service`.
///
/// # Safety
///
/// As [`obs_module_add_source`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_module_add_service(module: *mut c_void, id: *const c_char) {
    // SAFETY: the caller's contract.
    unsafe { add_type(module, TypeList::Services, id) }
}

/// Same contract as C `obs_find_module_file`.
///
/// # Safety
///
/// `module` is null or a live `obs_module_t`; `file` is null or
/// NUL-terminated.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_find_module_file(
    module: *mut c_void,
    file: *const c_char,
) -> *mut c_char {
    // SAFETY: the caller's contract.
    let (seam, file) = unsafe { (Seam::<Libobs>::new(), bytes(file)) };
    into_c(module_lists::find_file(
        &seam,
        &LibobsFs,
        self::module(module),
        file,
    ))
}

/// Same contract as C `obs_module_get_config_path`.
///
/// # Safety
///
/// `module` is null or a live `obs_module_t` of a live core; `file` is
/// null or NUL-terminated.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_module_get_config_path(
    module: *mut c_void,
    file: *const c_char,
) -> *mut c_char {
    // SAFETY: the caller's contract.
    let (seam, file) = unsafe { (Seam::<Libobs>::new(), bytes(file)) };
    into_c(module_lists::config_path(&seam, self::module(module), file))
}

/// Same contract as C `obs_add_module_path`.
///
/// # Safety
///
/// `bin` and `data` are null or NUL-terminated.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_add_module_path(bin: *const c_char, data: *const c_char) {
    // SAFETY: the caller's contract.
    let (mut seam, bin, data) = unsafe { (Seam::<Libobs>::new(), bytes(bin), bytes(data)) };
    module_lists::add_module_path(&mut seam, bin, data);
}

/// Same contract as C `obs_add_safe_module`.
///
/// # Safety
///
/// `name` is null or NUL-terminated.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_add_safe_module(name: *const c_char) {
    // SAFETY: the caller's contract.
    unsafe { add_name(NameList::Safe, name) }
}

/// Same contract as C `obs_add_disabled_module`.
///
/// # Safety
///
/// `name` is null or NUL-terminated.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_add_disabled_module(name: *const c_char) {
    // SAFETY: the caller's contract.
    unsafe { add_name(NameList::Disabled, name) }
}
