/* The parser comparison links private Rust shims statically and public APIs
 * through obs.dll. A Rust codegen object can also contain this public lookup;
 * calling the address-only import avoids loading obs.lib's same-named
 * function thunk before that object (LNK2005, #3603). The DLL still supplies
 * the public call, and the static Rust definition is retained without /FORCE.
 * Built only into test_effect_parser_blocks_rust, never into libobs. */
#include "graphics/graphics.h"

extern gs_eparam_t *(*__imp_gs_effect_get_param_by_name)(const gs_effect_t *, const char *);

gs_eparam_t *obs_test_effect_get_param_by_name(const gs_effect_t *effect, const char *name)
{
	return __imp_gs_effect_get_param_by_name(effect, name);
}
