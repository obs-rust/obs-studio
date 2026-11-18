/* Original C and real private adapters over a thread-local recording backend. */
#include <stdarg.h>
#include <stddef.h>
#include <stdio.h>
#include <string.h>
#define gs_copy_texture oracle_gs_copy_texture
#define gs_copy_texture_region oracle_gs_copy_texture_region
#include "graphics/graphics-internal.h"
#include "util/base.h"

void oracle_gs_copy_texture(gs_texture_t *dst, gs_texture_t *src);
void oracle_gs_copy_texture_region(gs_texture_t *dst, uint32_t dst_x, uint32_t dst_y, gs_texture_t *src, uint32_t src_x,
				   uint32_t src_y, uint32_t src_w, uint32_t src_h);

static THREAD_LOCAL struct graphics_subsystem tc_graphics;
static THREAD_LOCAL int tc_device_token;
static THREAD_LOCAL unsigned tc_calls, tc_logs;
static THREAD_LOCAL int tc_kind, tc_level;
static THREAD_LOCAL const void *tc_device, *tc_dst, *tc_src;
static THREAD_LOCAL uint32_t tc_values[6];
static THREAD_LOCAL char tc_log[160];

#ifdef __GNUC__
__attribute__((format(printf, 2, 3)))
#endif
static void tc_blog(int level, const char *format, ...)
{
	va_list args;
	va_start(args, format);
	tc_logs++;
	tc_level = level;
	vsnprintf(tc_log, sizeof(tc_log), format, args);
	va_end(args);
}

#define blog tc_blog
#include "graphics-texture-copy.inc"

/* Original graphics.c.h validators, verbatim. */
static inline bool gs_valid(const char *f)
{
	if (!thread_graphics) {
		blog(LOG_DEBUG, "%s: called while not in a graphics context", f);
		return false;
	}

	return true;
}

static inline bool gs_obj_valid(const void *obj, const char *f, const char *name)
{
	if (!obj) {
		blog(LOG_DEBUG, "%s: Null '%s' parameter", f, name);
		return false;
	}
	return true;
}

static graphics_t *tc_context(void)
{
	return thread_graphics;
}
#define gs_get_context tc_context
#include "graphics-texture-copy-fields.inc"
#undef gs_get_context
#undef blog

static void tc_region(gs_device_t *device, gs_texture_t *dst, uint32_t dst_x, uint32_t dst_y, gs_texture_t *src,
		      uint32_t src_x, uint32_t src_y, uint32_t src_w, uint32_t src_h)
{
	tc_calls++;
	tc_kind = 1;
	tc_device = device;
	tc_dst = dst;
	tc_src = src;
	tc_values[0] = dst_x;
	tc_values[1] = dst_y;
	tc_values[2] = src_x;
	tc_values[3] = src_y;
	tc_values[4] = src_w;
	tc_values[5] = src_h;
}

static void tc_whole(gs_device_t *device, gs_texture_t *dst, gs_texture_t *src)
{
	tc_region(device, dst, 0, 0, src, 0, 0, 0, 0);
	tc_kind = 0;
}

void oracle_texture_copy_reset(bool context);
void oracle_texture_copy_reset(bool context)
{
	memset(&tc_graphics, 0, sizeof(tc_graphics));
	tc_graphics.device = (gs_device_t *)&tc_device_token;
	tc_graphics.exports.device_copy_texture = tc_whole;
	tc_graphics.exports.device_copy_texture_region = tc_region;
	thread_graphics = context ? &tc_graphics : NULL;
	tc_calls = tc_logs = 0;
	tc_kind = -1;
	tc_level = 0;
	tc_device = tc_dst = tc_src = NULL;
	memset(tc_values, 0, sizeof(tc_values));
	tc_log[0] = '\0';
}

unsigned oracle_texture_copy_calls(void);
unsigned oracle_texture_copy_calls(void)
{
	return tc_calls;
}
unsigned oracle_texture_copy_logs(void);
unsigned oracle_texture_copy_logs(void)
{
	return tc_logs;
}
int oracle_texture_copy_kind(void);
int oracle_texture_copy_kind(void)
{
	return tc_kind;
}
int oracle_texture_copy_level(void);
int oracle_texture_copy_level(void)
{
	return tc_level;
}
const void *oracle_texture_copy_device(void);
const void *oracle_texture_copy_device(void)
{
	return tc_device;
}
const void *oracle_texture_copy_expected_device(void);
const void *oracle_texture_copy_expected_device(void)
{
	return &tc_device_token;
}
const void *oracle_texture_copy_dst(void);
const void *oracle_texture_copy_dst(void)
{
	return tc_dst;
}
const void *oracle_texture_copy_src(void);
const void *oracle_texture_copy_src(void)
{
	return tc_src;
}
uint32_t oracle_texture_copy_value(unsigned index);
uint32_t oracle_texture_copy_value(unsigned index)
{
	return tc_values[index];
}
const char *oracle_texture_copy_log(void);
const char *oracle_texture_copy_log(void)
{
	return tc_log;
}

#undef gs_copy_texture
#undef gs_copy_texture_region
