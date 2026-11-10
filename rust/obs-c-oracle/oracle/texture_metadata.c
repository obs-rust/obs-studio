/* Original extracted getters and real-header adapter, isolated per thread. */
#include <stdarg.h>
#include <stddef.h>
#include <stdio.h>
#include <string.h>
#define gs_texture_get_width oracle_gs_texture_get_width
#define gs_texture_get_height oracle_gs_texture_get_height
#define gs_texture_get_color_format oracle_gs_texture_get_color_format
#include "graphics/graphics-internal.h"
#include "util/base.h"

uint32_t oracle_gs_texture_get_width(const gs_texture_t *surface);
uint32_t oracle_gs_texture_get_height(const gs_texture_t *surface);
enum gs_color_format oracle_gs_texture_get_color_format(const gs_texture_t *surface);

static THREAD_LOCAL struct graphics_subsystem tm_graphics;
static THREAD_LOCAL bool tm_present;
static THREAD_LOCAL unsigned tm_calls[3];
static THREAD_LOCAL const gs_texture_t *tm_seen;
static THREAD_LOCAL uint32_t tm_width, tm_height;
static THREAD_LOCAL enum gs_color_format tm_format;
static THREAD_LOCAL unsigned tm_logs;
static THREAD_LOCAL int tm_level;
static THREAD_LOCAL char tm_text[128];
static uint32_t tm_get_width(const gs_texture_t *surface)
{
	tm_calls[0]++;
	tm_seen = surface;
	return tm_width;
}
static uint32_t tm_get_height(const gs_texture_t *surface)
{
	tm_calls[1]++;
	tm_seen = surface;
	return tm_height;
}
static enum gs_color_format tm_get_format(const gs_texture_t *surface)
{
	tm_calls[2]++;
	tm_seen = surface;
	return tm_format;
}
static graphics_t *tm_context(void)
{
	return tm_present ? &tm_graphics : NULL;
}
#ifdef __GNUC__
__attribute__((format(printf, 2, 3)))
#endif
static void tm_blog(int level, const char *format, ...)
{
	va_list args;
	va_start(args, format);
	tm_logs++;
	tm_level = level;
	vsnprintf(tm_text, sizeof(tm_text), format, args);
	va_end(args);
}
#define gs_get_context tm_context
#define blog tm_blog
#include "texture-metadata-fields.inc"
#undef blog
#undef gs_get_context
#define gs_get_context tm_context
#define blog tm_blog
#include "texture-metadata.inc"
#undef blog
#undef gs_get_context

void oracle_tm_reset(bool present, uint32_t width, uint32_t height, unsigned format);
void oracle_tm_reset(bool present, uint32_t width, uint32_t height, unsigned format)
{
	memset(&tm_graphics, 0, sizeof(tm_graphics));
	tm_graphics.exports.gs_texture_get_width = tm_get_width;
	tm_graphics.exports.gs_texture_get_height = tm_get_height;
	tm_graphics.exports.gs_texture_get_color_format = tm_get_format;
	tm_graphics.ref = present ? 1 : 0;
	tm_present = present;
	tm_width = width;
	tm_height = height;
	tm_format = (enum gs_color_format)format;
	memset(tm_calls, 0, sizeof(tm_calls));
	tm_seen = NULL;
	tm_logs = 0;
	tm_level = 0;
	tm_text[0] = '\0';
}
const void *oracle_tm_texture(void);
const void *oracle_tm_texture(void)
{
	return &tm_graphics;
}
unsigned oracle_tm_calls(unsigned index);
unsigned oracle_tm_calls(unsigned index)
{
	return tm_calls[index];
}
const void *oracle_tm_seen(void);
const void *oracle_tm_seen(void)
{
	return tm_seen;
}
unsigned oracle_tm_logs(void);
unsigned oracle_tm_logs(void)
{
	return tm_logs;
}
int oracle_tm_level(void);
int oracle_tm_level(void)
{
	return tm_level;
}
const char *oracle_tm_text(void);
const char *oracle_tm_text(void)
{
	return tm_text;
}
int oracle_tm_ref(void);
int oracle_tm_ref(void)
{
	return (int)tm_graphics.ref;
}
size_t oracle_tm_enum_size(void);
size_t oracle_tm_enum_size(void)
{
	return sizeof(enum gs_color_format);
}
size_t oracle_tm_enum_align(void);
size_t oracle_tm_enum_align(void)
{
	return _Alignof(enum gs_color_format);
}
unsigned oracle_tm_format(unsigned index);
unsigned oracle_tm_format(unsigned index)
{
	const enum gs_color_format formats[] = {GS_UNKNOWN, GS_RGBA, GS_RGBA16F, GS_R8};
	return (unsigned)formats[index];
}
/* Verify each returned callback against the actual private header field. */
bool oracle_tm_adapter_layout(void);
bool oracle_tm_adapter_layout(void)
{
	return obs_texture_metadata_width_field(&tm_graphics) == tm_graphics.exports.gs_texture_get_width &&
	       obs_texture_metadata_height_field(&tm_graphics) == tm_graphics.exports.gs_texture_get_height &&
	       obs_texture_metadata_format_field(&tm_graphics) == tm_graphics.exports.gs_texture_get_color_format;
}

#undef gs_texture_get_width
#undef gs_texture_get_height
#undef gs_texture_get_color_format
