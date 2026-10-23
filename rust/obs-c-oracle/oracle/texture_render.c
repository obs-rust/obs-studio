/* Test-only oracle: the original libobs/graphics/texture-render.c.h,
 * unmodified, with its exports renamed to oracle_* so it can link next to
 * the Rust implementation. Its gs_* graphics calls go to the recording
 * stand-in in texture_render_device.c, the same one the Rust shim calls. */
#define gs_texrender_create oracle_gs_texrender_create
#define gs_texrender_destroy oracle_gs_texrender_destroy
#define gs_texrender_begin oracle_gs_texrender_begin
#define gs_texrender_begin_with_color_space oracle_gs_texrender_begin_with_color_space
#define gs_texrender_end oracle_gs_texrender_end
#define gs_texrender_reset oracle_gs_texrender_reset
#define gs_texrender_get_texture oracle_gs_texrender_get_texture
#define gs_texrender_get_format oracle_gs_texrender_get_format

/* The original passes its uint32_t size to gs_set_viewport's int
 * parameters without the cast the oracle's -Wsign-conversion wants. */
#if defined(__clang__) || defined(__GNUC__)
#pragma GCC diagnostic push
#pragma GCC diagnostic ignored "-Wsign-conversion"
#endif
#include "graphics/texture-render.c.h"
#if defined(__clang__) || defined(__GNUC__)
#pragma GCC diagnostic pop
#endif
