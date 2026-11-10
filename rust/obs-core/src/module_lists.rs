//! Safe core of `libobs/obs-module-lists.c.h`: the module list calls.
//!
//! The ten calls are the module lists' public API: [`log_loaded_modules`]
//! for `obs_log_loaded_modules()`, [`add_type`] for
//! `obs_module_add_source()`, `_add_output()`, `_add_encoder()` and
//! `_add_service()`, [`find_file`] for `obs_find_module_file()`,
//! [`config_path`] for `obs_module_get_config_path()`, [`add_module_path`]
//! for `obs_add_module_path()`, and [`add_name`] for `obs_add_safe_module()`
//! and `obs_add_disabled_module()`.
//!
//! The core and its modules are the caller's [`Lists`]: libobs's own
//! through the module list seam (`obs-module-lists-seam.h`), a model in the
//! tests. Each read, each log line and each push is one call of it, made in
//! the C's order and under the C's conditions; none takes a lock, as the C
//! takes none. The paths come from the cores that are already Rust:
//! [`crate::module_paths::find_file`] and
//! [`crate::module_names::config_path`].
//!
//! Kept as the C has it, characterized and not endorsed: a NULL id is
//! pushed as a NULL entry ([`add_type`]), and [`config_path`] reads the
//! core's config path without checking the core.
//!
//! Intentional difference: [`log_loaded_modules`] with no core logs nothing
//! and returns. The C logs the heading and then reads the core's first
//! module without checking the core, so it crashes.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::unreachable
)]

use crate::module_paths::ModuleFs;

/// One of a module's type lists, the one each `obs_module_add_*()` pushes
/// to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TypeList {
    /// `module->sources`, of `obs_module_add_source()`.
    Sources,
    /// `module->outputs`, of `obs_module_add_output()`.
    Outputs,
    /// `module->encoders`, of `obs_module_add_encoder()`.
    Encoders,
    /// `module->services`, of `obs_module_add_service()`.
    Services,
}

/// One of the core's module name lists.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum NameList {
    /// `obs->safe_modules`, of `obs_add_safe_module()`.
    Safe,
    /// `obs->disabled_modules`, of `obs_add_disabled_module()`.
    Disabled,
}

/// A line [`log_loaded_modules`] logs, at `LOG_INFO`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Line<M> {
    /// [`HEADING`].
    Heading,
    /// `"    %s"` with the module's file.
    Module(M),
}

/// The first line of `obs_log_loaded_modules()`.
pub const HEADING: &str = "  Loaded Modules:";

/// The format of each module's line, with its file.
pub const MODULE_FORMAT: &str = "    %s";

/// The core and its modules, as the module list calls see them. Strings are
/// bytes with no NUL, `None` for NULL.
pub trait Lists {
    /// A module (a non-NULL `obs_module_t *`).
    type Module: Copy;

    /// Whether there is a core (`obs` is set).
    fn has_core(&self) -> bool;
    /// `obs->first_module`. Needs a core.
    fn first_module(&self) -> Option<Self::Module>;
    /// `obs->module_config_path`. Needs a core.
    fn module_config_path(&self) -> Option<&[u8]>;
    /// `module->next`.
    fn next(&self, module: Self::Module) -> Option<Self::Module>;
    /// `module->data_path`.
    fn data_path(&self, module: Self::Module) -> Option<&[u8]>;
    /// `module->mod_name`.
    fn module_name(&self, module: Self::Module) -> Option<&[u8]>;

    /// `blog(LOG_INFO, ...)` of one line.
    fn log(&mut self, line: Line<Self::Module>);
    /// `da_push_back(module-><list>, &copy)` with a copy of `id`, a NULL
    /// entry for `None`.
    fn push_type(&mut self, module: Self::Module, list: TypeList, id: Option<&[u8]>);
    /// `da_push_back(obs-><list>, &copy)` with a copy of `name`. Needs a
    /// core.
    fn push_name(&mut self, list: NameList, name: &[u8]);
    /// `da_push_back(obs->module_paths, &omp)` with a copy of each path and
    /// the type `OBS_MODULE_TYPE_UNDEFINED`. Needs a core.
    fn push_path(&mut self, bin: &[u8], data: &[u8]);
}

/// Same contract as C `obs_log_loaded_modules`, but with no core it logs
/// nothing (the intentional difference): the heading, then a line for each
/// module from the first, in order.
pub fn log_loaded_modules<L: Lists>(lists: &mut L) {
    let _ = lists;
    todo!()
}

/// Same contract as C `obs_module_add_source` and its siblings: a copy of
/// `id` (NULL for `None`) on the module's `list`; nothing with no module.
pub fn add_type<L: Lists>(
    lists: &mut L,
    module: Option<L::Module>,
    list: TypeList,
    id: Option<&[u8]>,
) {
    let _ = (lists, module, list, id);
    todo!()
}

/// Same contract as C `obs_find_module_file`: the file under the module's
/// data path ([`crate::module_paths::find_file`]), `None` with no module.
pub fn find_file<L: Lists>(
    lists: &L,
    fs: &impl ModuleFs,
    module: Option<L::Module>,
    file: Option<&[u8]>,
) -> Option<Vec<u8>> {
    let _ = (lists, fs, module, file);
    todo!()
}

/// Same contract as C `obs_module_get_config_path`: the core's module
/// config path, the module's name and `file`
/// ([`crate::module_names::config_path`]), `None` with no module. It reads the
/// core's config path whether or not there is a core, as the C does.
pub fn config_path<L: Lists>(
    lists: &L,
    module: Option<L::Module>,
    file: Option<&[u8]>,
) -> Option<Vec<u8>> {
    let _ = (lists, module, file);
    todo!()
}

/// Same contract as C `obs_add_module_path`: a copy of both paths on the
/// core's search paths; nothing with no core or without both.
pub fn add_module_path<L: Lists>(lists: &mut L, bin: Option<&[u8]>, data: Option<&[u8]>) {
    let _ = (lists, bin, data);
    todo!()
}

/// Same contract as C `obs_add_safe_module` and `obs_add_disabled_module`:
/// a copy of `name` on the core's `list`; nothing with no core or no name.
pub fn add_name<L: Lists>(lists: &mut L, list: NameList, name: Option<&[u8]>) {
    let _ = (lists, list, name);
    todo!()
}
