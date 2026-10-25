/* Test-only oracle: the original libobs/graphics/effect-passes.c.h,
 * unmodified, with its exports renamed to oracle_* so it can link next to
 * the Rust implementation. The device functions it calls are the recording
 * stubs of oracle/effect_passes_device.c, which the Rust shim calls too;
 * gs_effect_get_technique is the Rust lookup of the test binary. */
#define gs_effect_loop oracle_gs_effect_loop
#define gs_technique_begin oracle_gs_technique_begin
#define gs_technique_end oracle_gs_technique_end
#define gs_effect_update_params oracle_gs_effect_update_params
#define gs_technique_begin_pass oracle_gs_technique_begin_pass
#define gs_technique_begin_pass_by_name oracle_gs_technique_begin_pass_by_name
#define gs_technique_end_pass oracle_gs_technique_end_pass

#include "graphics/effect-passes.c.h"

#undef gs_effect_loop
#undef gs_technique_begin
#undef gs_technique_end
#undef gs_effect_update_params
#undef gs_technique_begin_pass
#undef gs_technique_begin_pass_by_name
#undef gs_technique_end_pass
