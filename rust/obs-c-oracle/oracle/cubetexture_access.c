/* Original extracted getters and real-header adapter, isolated per thread. */
#include <stdarg.h>
#include <stddef.h>
#include <stdio.h>
#include <string.h>
#define gs_cubetexture_get_size oracle_gs_cubetexture_get_size
#define gs_cubetexture_get_color_format oracle_gs_cubetexture_get_color_format
#define gs_cubetexture_destroy oracle_gs_cubetexture_destroy
#include "graphics/graphics-internal.h"
#include "util/base.h"

uint32_t oracle_gs_cubetexture_get_size(const gs_texture_t *surface);
enum gs_color_format oracle_gs_cubetexture_get_color_format(const gs_texture_t *surface);

static THREAD_LOCAL struct graphics_subsystem cta_graphics;
static THREAD_LOCAL bool cta_present;
static THREAD_LOCAL unsigned cta_calls[3];
static THREAD_LOCAL const gs_texture_t *cta_seen;
static THREAD_LOCAL uint32_t cta_size;
static THREAD_LOCAL enum gs_color_format cta_format;
static THREAD_LOCAL unsigned cta_logs;
static THREAD_LOCAL int cta_level;
static THREAD_LOCAL char cta_text[128];
static uint32_t cta_get_size(const gs_texture_t *surface)
{
	cta_calls[0]++;
	cta_seen = surface;
	return cta_size;
}
static THREAD_LOCAL unsigned char cta_token;
static void cta_destroy(gs_texture_t *surface)
{
	cta_calls[1]++;
	cta_seen = surface;
	cta_token = 0xa5;
}
static enum gs_color_format cta_get_format(const gs_texture_t *surface)
{
	cta_calls[2]++;
	cta_seen = surface;
	return cta_format;
}
static graphics_t *cta_context(void)
{
	return cta_present ? &cta_graphics : NULL;
}
#ifdef __GNUC__
__attribute__((format(printf, 2, 3)))
#endif
static void cta_blog(int level, const char *format, ...)
{
	va_list args;
	va_start(args, format);
	cta_logs++;
	cta_level = level;
	vsnprintf(cta_text, sizeof(cta_text), format, args);
	va_end(args);
}
#define gs_get_context cta_context
#define blog cta_blog
#include "cubetexture-access-fields.inc"
#undef blog
#undef gs_get_context
#define gs_get_context cta_context
#define blog cta_blog
#include "texture-cube-access.inc"
#undef blog
#undef gs_get_context

void oracle_cta_reset(bool present, uint32_t size, unsigned format);
void oracle_cta_reset(bool present, uint32_t size, unsigned format)
{
	memset(&cta_graphics, 0, sizeof(cta_graphics));
	cta_graphics.exports.gs_cubetexture_get_size = cta_get_size;
	cta_graphics.exports.gs_cubetexture_destroy = cta_destroy;
	cta_graphics.exports.gs_cubetexture_get_color_format = cta_get_format;
	cta_graphics.ref = present ? 1 : 0;
	cta_present = present;
	cta_size = size;
	cta_token = 0x5a;
	cta_format = (enum gs_color_format)format;
	memset(cta_calls, 0, sizeof(cta_calls));
	cta_seen = NULL;
	cta_logs = 0;
	cta_level = 0;
	cta_text[0] = '\0';
}
const void *oracle_cta_texture(void);
const void *oracle_cta_texture(void)
{
	return &cta_token;
}
unsigned oracle_cta_calls(unsigned index);
unsigned oracle_cta_calls(unsigned index)
{
	return cta_calls[index];
}
const void *oracle_cta_seen(void);
const void *oracle_cta_seen(void)
{
	return cta_seen;
}
unsigned oracle_cta_logs(void);
unsigned oracle_cta_logs(void)
{
	return cta_logs;
}
int oracle_cta_level(void);
int oracle_cta_level(void)
{
	return cta_level;
}
const char *oracle_cta_text(void);
const char *oracle_cta_text(void)
{
	return cta_text;
}
int oracle_cta_ref(void);
int oracle_cta_ref(void)
{
	return (int)cta_graphics.ref;
}
size_t oracle_cta_enum_size(void);
size_t oracle_cta_enum_size(void)
{
	return sizeof(enum gs_color_format);
}
size_t oracle_cta_enum_align(void);
size_t oracle_cta_enum_align(void)
{
	return _Alignof(enum gs_color_format);
}
unsigned oracle_cta_format(unsigned index);
unsigned oracle_cta_format(unsigned index)
{
	const enum gs_color_format formats[] = {GS_UNKNOWN,    GS_A8,          GS_R8,     GS_RGBA, GS_BGRX,
						GS_BGRA,       GS_R10G10B10A2, GS_RGBA16, GS_R16,  GS_RGBA16F,
						GS_RGBA32F,    GS_RG16F,       GS_RG32F,  GS_R16F, GS_R32F,
						GS_DXT1,       GS_DXT3,        GS_DXT5,   GS_R8G8, GS_RGBA_UNORM,
						GS_BGRX_UNORM, GS_BGRA_UNORM,  GS_RG16};
	return (unsigned)formats[index];
}
/* Real-header callback layout check. */
bool oracle_cta_adapter_layout(void);
bool oracle_cta_adapter_layout(void)
{
	return obs_cubetexture_access_size_field(&cta_graphics) == cta_graphics.exports.gs_cubetexture_get_size &&
	       obs_cubetexture_access_destroy_field(&cta_graphics) == cta_graphics.exports.gs_cubetexture_destroy &&
	       obs_cubetexture_access_format_field(&cta_graphics) ==
		       cta_graphics.exports.gs_cubetexture_get_color_format;
}
unsigned oracle_cta_token(void);
unsigned oracle_cta_token(void)
{
	return cta_token;
}
#undef gs_cubetexture_get_size
#undef gs_cubetexture_destroy
#undef gs_cubetexture_get_color_format
