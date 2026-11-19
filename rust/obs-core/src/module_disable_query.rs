//! `obs_get_module_allow_disable` (`libobs/obs-module-disable-query.c.h`,
//! #5183): whether the module with a given name may be disabled.
//!
//! The query asks this build's core-module list whether the name is a core
//! module's, then asks the existing module load policy
//! ([`crate::module_load_rules::allow_disable`]) with that answer, and
//! returns the policy's answer unchanged. Each is asked exactly once,
//! membership first. The name is a borrowed token passed through as it
//! came: it is not read, copied or normalized here, so matching stays the
//! generated list's case-sensitive `strcmp()`. Nothing is allocated, no
//! module is loaded, and the safe and disabled lists are not consulted.

/// The two calls the query makes, in this order.
pub trait Dependencies<N> {
    /// `obs_in_core_module_list(name)`: whether `name` is in this build's
    /// generated core-module list.
    fn in_core_module_list(&mut self, name: N) -> bool;
    /// `obs_module_load_rules_allow_disable(core_name)`: whether a module
    /// so classified may be disabled.
    fn allow_disable(&mut self, core_name: bool) -> bool;
}

/// `obs_get_module_allow_disable(name)` over its two dependencies.
pub fn get_module_allow_disable<N, D: Dependencies<N>>(deps: &mut D, name: N) -> bool {
    let core_name = deps.in_core_module_list(name);
    deps.allow_disable(core_name)
}
