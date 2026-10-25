/* Test-only oracle: the original libobs/graphics/effect-params.c.h,
 * unmodified, with its exports renamed to oracle_* so it can link next to
 * the Rust implementation. blog, bzalloc and the darray inlines are the
 * C ones of the test binary; the Rust shim logs and allocates through the
 * adapters of obs-graphics-adapter.c.h, which oracle/image_file.c builds
 * into the same binary. */
#define gs_effect_set_bool oracle_gs_effect_set_bool
#define gs_effect_set_float oracle_gs_effect_set_float
#define gs_effect_set_int oracle_gs_effect_set_int
#define gs_effect_set_matrix4 oracle_gs_effect_set_matrix4
#define gs_effect_set_vec2 oracle_gs_effect_set_vec2
#define gs_effect_set_vec3 oracle_gs_effect_set_vec3
#define gs_effect_set_vec4 oracle_gs_effect_set_vec4
#define gs_effect_set_color oracle_gs_effect_set_color
#define gs_effect_set_texture oracle_gs_effect_set_texture
#define gs_effect_set_texture_srgb oracle_gs_effect_set_texture_srgb
#define gs_effect_set_val oracle_gs_effect_set_val
#define gs_effect_get_val oracle_gs_effect_get_val
#define gs_effect_get_val_size oracle_gs_effect_get_val_size
#define gs_effect_get_default_val oracle_gs_effect_get_default_val
#define gs_effect_get_default_val_size oracle_gs_effect_get_default_val_size
#define gs_effect_set_default oracle_gs_effect_set_default
#define gs_effect_set_next_sampler oracle_gs_effect_set_next_sampler

#include "graphics/effect-params.c.h"

#undef gs_effect_set_bool
#undef gs_effect_set_float
#undef gs_effect_set_int
#undef gs_effect_set_matrix4
#undef gs_effect_set_vec2
#undef gs_effect_set_vec3
#undef gs_effect_set_vec4
#undef gs_effect_set_color
#undef gs_effect_set_texture
#undef gs_effect_set_texture_srgb
#undef gs_effect_set_val
#undef gs_effect_get_val
#undef gs_effect_get_val_size
#undef gs_effect_get_default_val
#undef gs_effect_get_default_val_size
#undef gs_effect_set_default
#undef gs_effect_set_next_sampler

#include <stddef.h>

/* The layout of struct gs_shader_texture, in the order of
 * rust/obs-c-oracle/src/effect_params.rs. */
enum {
	ORACLE_EFFECT_PARAMS_LAYOUT_COUNT = 4,
};

size_t oracle_effect_params_layout(size_t which);

size_t oracle_effect_params_layout(size_t which)
{
	const size_t values[ORACLE_EFFECT_PARAMS_LAYOUT_COUNT] = {
		sizeof(struct gs_shader_texture),
		_Alignof(struct gs_shader_texture),
		offsetof(struct gs_shader_texture, tex),
		offsetof(struct gs_shader_texture, srgb),
	};
	return which < ORACLE_EFFECT_PARAMS_LAYOUT_COUNT ? values[which] : 0;
}
