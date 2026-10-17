//! Opens NVML for [`crate::nvml`] (obs-rust/obs-studio#1021): the first of
//! the platform's candidates that loads and initialises, through libloading.

use std::ffi::{CStr, c_void};

use crate::nvml::{Nvml, Symbols};

struct Lib(libloading::Library);

impl Symbols for Lib {
    fn get(&self, name: &CStr) -> Option<*mut c_void> {
        // SAFETY: the address is read as an untyped pointer only;
        // crate::nvml casts it to the documented type of `name`.
        let symbol = unsafe { self.0.get::<*mut c_void>(name.to_bytes_with_nul()) }.ok()?;
        Some(*symbol)
    }
}

pub(super) fn load(candidates: &[&str]) -> Option<Nvml> {
    candidates.iter().find_map(|name| {
        // SAFETY: loading the NVIDIA driver's NVML runs its initialisers.
        // The names are fixed data (crate::nvml's candidate lists).
        let lib = unsafe { libloading::Library::new(name) }.ok()?;
        Nvml::load(Box::new(Lib(lib)))
    })
}
