/* Test-only oracle: the original libobs/graphics/graphics-sprite.c.h,
 * unmodified, with its functions renamed to oracle_* so they can link next
 * to the Rust shims. */
#include <stddef.h>

#define gs_sprite_build oracle_gs_sprite_build
#define gs_sprite_uv_norm oracle_gs_sprite_uv_norm
#define gs_sprite_uv_rect oracle_gs_sprite_uv_rect
#define gs_sprite_uv_subregion oracle_gs_sprite_uv_subregion
#define gs_sprite_subregion_is_whole oracle_gs_sprite_subregion_is_whole
#define gs_sprite_quad_refusal oracle_gs_sprite_quad_refusal
#define gs_sprite_quad_texture_size oracle_gs_sprite_quad_texture_size
#define gs_sprite_quad_flipped_buffer oracle_gs_sprite_quad_flipped_buffer
#define gs_texture_upload_plan oracle_gs_texture_upload_plan
#define gs_texture_is_pow2 oracle_gs_texture_is_pow2
#define gs_texture_mip_plan oracle_gs_texture_mip_plan

/* libobs' own build disables -Wsign-conversion (cmake/linux/
 * compilerconfig.cmake and cmake/windows/compilerconfig.cmake), and
 * gs_texture_mip_plan() clears GS_SPRITE_BUILD_MIPMAPS with flags &= ~(int),
 * as gs_texture_create() always did. Widen only around the include, as
 * oracle/platform.c does; the C file is unchanged. */
#if defined(__GNUC__) && !defined(__clang__)
#pragma GCC diagnostic push
#pragma GCC diagnostic ignored "-Wsign-conversion"
#elif defined(_MSC_VER)
#pragma warning(push)
#pragma warning(disable : 4245)
#else
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wsign-conversion"
#endif
#include "graphics/graphics-sprite.c.h"
#if defined(__GNUC__) && !defined(__clang__)
#pragma GCC diagnostic pop
#elif defined(_MSC_VER)
#pragma warning(pop)
#else
#pragma clang diagnostic pop
#endif

#undef gs_texture_mip_plan
#undef gs_texture_is_pow2
#undef gs_texture_upload_plan
#undef gs_sprite_quad_flipped_buffer
#undef gs_sprite_quad_texture_size
#undef gs_sprite_quad_refusal
#undef gs_sprite_subregion_is_whole
#undef gs_sprite_uv_subregion
#undef gs_sprite_uv_rect
#undef gs_sprite_uv_norm
#undef gs_sprite_build

/* The constants of the header, in the order of
 * rust/obs-c-oracle/src/graphics_sprite.rs. */
enum {
	ORACLE_GRAPHICS_SPRITE_CONSTANT_COUNT = 14,
};

long long oracle_graphics_sprite_constant(size_t which);

long long oracle_graphics_sprite_constant(size_t which)
{
	const long long values[ORACLE_GRAPHICS_SPRITE_CONSTANT_COUNT] = {
		GS_SPRITE_FLIP_U,          GS_SPRITE_FLIP_V,
		GS_SPRITE_BUILD_MIPMAPS,   GS_SPRITE_RENDER_TARGET,
		GS_SPRITE_QUAD_DRAW,       GS_SPRITE_QUAD_NOT_2D,
		GS_SPRITE_QUAD_NO_SIZE,    GS_TEXTURE_UPLOAD_NONE,
		GS_TEXTURE_UPLOAD_FLIPPED, GS_TEXTURE_UPLOAD_WHOLE,
		GS_TEXTURE_UPLOAD_ROWS,    GS_TEXTURE_MIP_OK,
		GS_TEXTURE_MIP_NOT_POW2,   GS_TEXTURE_MIP_RENDER_TARGET,
	};
	return which < ORACLE_GRAPHICS_SPRITE_CONSTANT_COUNT ? values[which] : -1;
}

/* The layout of the three structs and the size of the three enums, for the
 * Rust mirrors, in the order of rust/obs-c-oracle/src/graphics_sprite.rs.
 * struct gs_texture_upload_plan, struct gs_texture_mip_plan and enum
 * gs_sprite_quad_refusal share their names with functions, so the renames
 * above renamed their tags too. */
enum {
	ORACLE_GRAPHICS_SPRITE_LAYOUT_COUNT = 17,
};

size_t oracle_graphics_sprite_layout(size_t which);

size_t oracle_graphics_sprite_layout(size_t which)
{
	const size_t values[ORACLE_GRAPHICS_SPRITE_LAYOUT_COUNT] = {
		sizeof(struct gs_sprite_uv),
		offsetof(struct gs_sprite_uv, start_u),
		offsetof(struct gs_sprite_uv, end_u),
		offsetof(struct gs_sprite_uv, start_v),
		offsetof(struct gs_sprite_uv, end_v),
		sizeof(struct oracle_gs_texture_upload_plan),
		offsetof(struct oracle_gs_texture_upload_plan, row_copy),
		offsetof(struct oracle_gs_texture_upload_plan, whole_size),
		offsetof(struct oracle_gs_texture_upload_plan, src_offset),
		offsetof(struct oracle_gs_texture_upload_plan, mode),
		sizeof(struct oracle_gs_texture_mip_plan),
		offsetof(struct oracle_gs_texture_mip_plan, flags),
		offsetof(struct oracle_gs_texture_mip_plan, levels),
		offsetof(struct oracle_gs_texture_mip_plan, warning),
		sizeof(enum oracle_gs_sprite_quad_refusal),
		sizeof(enum gs_texture_upload_mode),
		sizeof(enum gs_texture_mip_warning),
	};
	return which < ORACLE_GRAPHICS_SPRITE_LAYOUT_COUNT ? values[which] : 0;
}
