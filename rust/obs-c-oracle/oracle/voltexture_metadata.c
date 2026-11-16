/* Original extracted getters and real-header adapter, isolated per thread. */
#include <stdarg.h>
#include <stddef.h>
#include <stdio.h>
#include <string.h>
#define gs_voltexture_get_width oracle_gs_voltexture_get_width
#define gs_voltexture_get_height oracle_gs_voltexture_get_height
#define gs_voltexture_get_depth oracle_gs_voltexture_get_depth
#define gs_voltexture_get_color_format oracle_gs_voltexture_get_color_format
#include "graphics/graphics-internal.h"
#include "util/base.h"

uint32_t oracle_gs_voltexture_get_width(const gs_texture_t *surface);
uint32_t oracle_gs_voltexture_get_height(const gs_texture_t *surface);
uint32_t oracle_gs_voltexture_get_depth(const gs_texture_t *surface);
enum gs_color_format oracle_gs_voltexture_get_color_format(const gs_texture_t *surface);

static THREAD_LOCAL struct graphics_subsystem vtm_graphics;
static THREAD_LOCAL bool vtm_present;
static THREAD_LOCAL unsigned vtm_calls[4];
static THREAD_LOCAL const gs_texture_t *vtm_seen;
static THREAD_LOCAL uint32_t vtm_width, vtm_height, vtm_depth;
static THREAD_LOCAL enum gs_color_format vtm_format;
static THREAD_LOCAL unsigned vtm_logs;
static THREAD_LOCAL int vtm_level;
static THREAD_LOCAL char vtm_text[128];
static uint32_t vtm_get_width(const gs_texture_t *surface)
{
	vtm_calls[0]++;
	vtm_seen = surface;
	return vtm_width;
}
static uint32_t vtm_get_height(const gs_texture_t *surface)
{
	vtm_calls[1]++;
	vtm_seen = surface;
	return vtm_height;
}
static uint32_t vtm_get_depth(const gs_texture_t *surface)
{
	vtm_calls[2]++;
	vtm_seen = surface;
	return vtm_depth;
}
static enum gs_color_format vtm_get_format(const gs_texture_t *surface)
{
	vtm_calls[3]++;
	vtm_seen = surface;
	return vtm_format;
}
static graphics_t *vtm_context(void)
{
	return vtm_present ? &vtm_graphics : NULL;
}
#ifdef __GNUC__
__attribute__((format(printf, 2, 3)))
#endif
static void vtm_blog(int level, const char *format, ...)
{
	va_list args;
	va_start(args, format);
	vtm_logs++;
	vtm_level = level;
	vsnprintf(vtm_text, sizeof(vtm_text), format, args);
	va_end(args);
}
#define gs_get_context vtm_context
#define blog vtm_blog
#include "voltexture-metadata-fields.inc"
#undef blog
#undef gs_get_context
#define gs_get_context vtm_context
#define blog vtm_blog
#include "voltexture-metadata.inc"
#undef blog
#undef gs_get_context

void oracle_vtm_reset(bool present, uint32_t width, uint32_t height, uint32_t depth, unsigned format);
void oracle_vtm_reset(bool present, uint32_t width, uint32_t height, uint32_t depth, unsigned format)
{
	memset(&vtm_graphics, 0, sizeof(vtm_graphics));
	vtm_graphics.exports.gs_voltexture_get_width = vtm_get_width;
	vtm_graphics.exports.gs_voltexture_get_height = vtm_get_height;
	vtm_graphics.exports.gs_voltexture_get_depth = vtm_get_depth;
	vtm_graphics.exports.gs_voltexture_get_color_format = vtm_get_format;
	vtm_graphics.ref = present ? 1 : 0;
	vtm_present = present;
	vtm_width = width;
	vtm_height = height;
	vtm_depth = depth;
	vtm_format = (enum gs_color_format)format;
	memset(vtm_calls, 0, sizeof(vtm_calls));
	vtm_seen = NULL;
	vtm_logs = 0;
	vtm_level = 0;
	vtm_text[0] = '\0';
}
const void *oracle_vtm_texture(void);
const void *oracle_vtm_texture(void)
{
	return &vtm_graphics;
}
unsigned oracle_vtm_calls(unsigned index);
unsigned oracle_vtm_calls(unsigned index)
{
	return vtm_calls[index];
}
const void *oracle_vtm_seen(void);
const void *oracle_vtm_seen(void)
{
	return vtm_seen;
}
unsigned oracle_vtm_logs(void);
unsigned oracle_vtm_logs(void)
{
	return vtm_logs;
}
int oracle_vtm_level(void);
int oracle_vtm_level(void)
{
	return vtm_level;
}
const char *oracle_vtm_text(void);
const char *oracle_vtm_text(void)
{
	return vtm_text;
}
int oracle_vtm_ref(void);
int oracle_vtm_ref(void)
{
	return (int)vtm_graphics.ref;
}
size_t oracle_vtm_enum_size(void);
size_t oracle_vtm_enum_size(void)
{
	return sizeof(enum gs_color_format);
}
size_t oracle_vtm_enum_align(void);
size_t oracle_vtm_enum_align(void)
{
	return _Alignof(enum gs_color_format);
}
unsigned oracle_vtm_format(unsigned index);
unsigned oracle_vtm_format(unsigned index)
{
	const enum gs_color_format formats[] = {GS_UNKNOWN,    GS_A8,          GS_R8,     GS_RGBA, GS_BGRX,
						GS_BGRA,       GS_R10G10B10A2, GS_RGBA16, GS_R16,  GS_RGBA16F,
						GS_RGBA32F,    GS_RG16F,       GS_RG32F,  GS_R16F, GS_R32F,
						GS_DXT1,       GS_DXT3,        GS_DXT5,   GS_R8G8, GS_RGBA_UNORM,
						GS_BGRX_UNORM, GS_BGRA_UNORM,  GS_RG16};
	return (unsigned)formats[index];
}
/* Verify each returned callback against the actual private header field. */
bool oracle_vtm_adapter_layout(void);
bool oracle_vtm_adapter_layout(void)
{
	return obs_voltexture_metadata_width_field(&vtm_graphics) == vtm_graphics.exports.gs_voltexture_get_width &&
	       obs_voltexture_metadata_height_field(&vtm_graphics) == vtm_graphics.exports.gs_voltexture_get_height &&
	       obs_voltexture_metadata_depth_field(&vtm_graphics) == vtm_graphics.exports.gs_voltexture_get_depth &&
	       obs_voltexture_metadata_format_field(&vtm_graphics) ==
		       vtm_graphics.exports.gs_voltexture_get_color_format;
}

#undef gs_voltexture_get_width
#undef gs_voltexture_get_height
#undef gs_voltexture_get_color_format

#undef gs_voltexture_get_depth
