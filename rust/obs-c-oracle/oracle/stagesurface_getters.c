/* Original extracted getters and real-header adapter, isolated per thread. */
#include <stdarg.h>
#include <stddef.h>
#include <stdio.h>
#include <string.h>
#define gs_stagesurface_get_width oracle_gs_stagesurface_get_width
#define gs_stagesurface_get_height oracle_gs_stagesurface_get_height
#define gs_stagesurface_get_color_format oracle_gs_stagesurface_get_color_format
#include "graphics/graphics-internal.h"
#include "util/base.h"

uint32_t oracle_gs_stagesurface_get_width(const gs_stagesurf_t *surface);
uint32_t oracle_gs_stagesurface_get_height(const gs_stagesurf_t *surface);
enum gs_color_format oracle_gs_stagesurface_get_color_format(const gs_stagesurf_t *surface);

static THREAD_LOCAL struct graphics_subsystem ss_graphics;
static THREAD_LOCAL bool ss_present;
static THREAD_LOCAL unsigned ss_calls[3];
static THREAD_LOCAL const gs_stagesurf_t *ss_seen;
static THREAD_LOCAL uint32_t ss_width, ss_height;
static THREAD_LOCAL enum gs_color_format ss_format;
static THREAD_LOCAL unsigned ss_logs;
static THREAD_LOCAL int ss_level;
static THREAD_LOCAL char ss_text[128];
static uint32_t ss_get_width(const gs_stagesurf_t *surface)
{
	ss_calls[0]++;
	ss_seen = surface;
	return ss_width;
}
static uint32_t ss_get_height(const gs_stagesurf_t *surface)
{
	ss_calls[1]++;
	ss_seen = surface;
	return ss_height;
}
static enum gs_color_format ss_get_format(const gs_stagesurf_t *surface)
{
	ss_calls[2]++;
	ss_seen = surface;
	return ss_format;
}
static graphics_t *ss_context(void)
{
	return ss_present ? &ss_graphics : NULL;
}
#ifdef __GNUC__
__attribute__((format(printf, 2, 3)))
#endif
static void ss_blog(int level, const char *format, ...)
{
	va_list args;
	va_start(args, format);
	ss_logs++;
	ss_level = level;
	vsnprintf(ss_text, sizeof(ss_text), format, args);
	va_end(args);
}
#define gs_get_context ss_context
#define blog ss_blog
#define obs_graphics_linear_srgb_field oracle_ss_unused_linear_srgb_field
#include "graphics-linear-srgb-fields.inc"
#undef obs_graphics_linear_srgb_field
#undef blog
#undef gs_get_context
#define gs_get_context ss_context
#define blog ss_blog
#include "stagesurface-getters.inc"
#undef blog
#undef gs_get_context

void oracle_ss_reset(bool present, uint32_t width, uint32_t height, unsigned format);
void oracle_ss_reset(bool present, uint32_t width, uint32_t height, unsigned format)
{
	memset(&ss_graphics, 0, sizeof(ss_graphics));
	ss_graphics.exports.gs_stagesurface_get_width = ss_get_width;
	ss_graphics.exports.gs_stagesurface_get_height = ss_get_height;
	ss_graphics.exports.gs_stagesurface_get_color_format = ss_get_format;
	ss_graphics.ref = present ? 1 : 0;
	ss_present = present;
	ss_width = width;
	ss_height = height;
	ss_format = (enum gs_color_format)format;
	memset(ss_calls, 0, sizeof(ss_calls));
	ss_seen = NULL;
	ss_logs = 0;
	ss_level = 0;
	ss_text[0] = '\0';
}
const void *oracle_ss_surface(void);
const void *oracle_ss_surface(void)
{
	return &ss_graphics;
}
unsigned oracle_ss_calls(unsigned index);
unsigned oracle_ss_calls(unsigned index)
{
	return ss_calls[index];
}
const void *oracle_ss_seen(void);
const void *oracle_ss_seen(void)
{
	return ss_seen;
}
unsigned oracle_ss_logs(void);
unsigned oracle_ss_logs(void)
{
	return ss_logs;
}
int oracle_ss_level(void);
int oracle_ss_level(void)
{
	return ss_level;
}
const char *oracle_ss_text(void);
const char *oracle_ss_text(void)
{
	return ss_text;
}
int oracle_ss_ref(void);
int oracle_ss_ref(void)
{
	return (int)ss_graphics.ref;
}
size_t oracle_ss_enum_size(void);
size_t oracle_ss_enum_size(void)
{
	return sizeof(enum gs_color_format);
}
size_t oracle_ss_enum_align(void);
size_t oracle_ss_enum_align(void)
{
	return _Alignof(enum gs_color_format);
}
unsigned oracle_ss_format(unsigned index);
unsigned oracle_ss_format(unsigned index)
{
	const enum gs_color_format formats[] = {GS_UNKNOWN, GS_RGBA, GS_RGBA16F, GS_R8};
	return (unsigned)formats[index];
}
/* Verify each returned callback against the actual private header field. */
bool oracle_ss_adapter_layout(void);
bool oracle_ss_adapter_layout(void)
{
	return obs_stagesurface_width_field(&ss_graphics) == ss_graphics.exports.gs_stagesurface_get_width &&
	       obs_stagesurface_height_field(&ss_graphics) == ss_graphics.exports.gs_stagesurface_get_height &&
	       obs_stagesurface_format_field(&ss_graphics) == ss_graphics.exports.gs_stagesurface_get_color_format;
}

#undef gs_stagesurface_get_width
#undef gs_stagesurface_get_height
#undef gs_stagesurface_get_color_format
