/* Original C and real adapters over a thread-local GPU-free recording backend. */
#include <assert.h>
#include <stdarg.h>
#include <stdio.h>
#include <string.h>
#include "graphics/graphics-internal.h"
#include "util/base.h"

#define gs_get_size oracle_gs_get_size
#define gs_get_width oracle_gs_get_width
#define gs_get_height oracle_gs_get_height
void oracle_gs_get_size(uint32_t *x, uint32_t *y);
uint32_t oracle_gs_get_width(void);
uint32_t oracle_gs_get_height(void);
static THREAD_LOCAL struct graphics_subsystem ts_graphics;
static THREAD_LOCAL int ts_device;
static THREAD_LOCAL uint32_t ts_width, ts_height;
static THREAD_LOCAL uint32_t *ts_x, *ts_y;
static THREAD_LOCAL unsigned ts_calls[3], ts_logs;
static THREAD_LOCAL int ts_level;
static THREAD_LOCAL char ts_log[160];

#ifdef __GNUC__
__attribute__((format(printf, 2, 3)))
#endif
static void ts_blog(int level, const char *format, ...)
{
	va_list args;
	va_start(args, format);
	ts_logs++;
	ts_level = level;
	vsnprintf(ts_log, sizeof(ts_log), format, args);
	va_end(args);
}
#define blog ts_blog
#include "graphics-target-size.inc"

/* Original graphics.c.h context validator, verbatim. */
static inline bool gs_valid(const char *f)
{
	if (!thread_graphics) {
		blog(LOG_DEBUG, "%s: called while not in a graphics context", f);
		return false;
	}

	return true;
}
static graphics_t *ts_context(void)
{
	return thread_graphics;
}
#define gs_get_context ts_context
#include "graphics-target-size-fields.inc"
#undef gs_get_context
#undef blog

static void ts_query_size(const gs_device_t *device, uint32_t *x, uint32_t *y)
{
	assert(thread_graphics == &ts_graphics);
	assert(device == (gs_device_t *)&ts_device);
	assert(x == ts_x && y == ts_y);
	ts_calls[0]++;
	if (x)
		*x = ts_width;
	if (y)
		*y = ts_height;
}
static uint32_t ts_query_width(const gs_device_t *device)
{
	assert(thread_graphics == &ts_graphics);
	assert(device == (gs_device_t *)&ts_device);
	ts_calls[1]++;
	return ts_width;
}
static uint32_t ts_query_height(const gs_device_t *device)
{
	assert(thread_graphics == &ts_graphics);
	assert(device == (gs_device_t *)&ts_device);
	ts_calls[2]++;
	return ts_height;
}
void oracle_target_size_reset(bool context, unsigned callbacks, uint32_t width, uint32_t height);
void oracle_target_size_reset(bool context, unsigned callbacks, uint32_t width, uint32_t height)
{
	memset(&ts_graphics, 0, sizeof(ts_graphics));
	ts_graphics.device = (gs_device_t *)&ts_device;
	ts_graphics.exports.device_get_size = (callbacks & 1) ? ts_query_size : NULL;
	ts_graphics.exports.device_get_width = (callbacks & 2) ? ts_query_width : NULL;
	ts_graphics.exports.device_get_height = (callbacks & 4) ? ts_query_height : NULL;
	thread_graphics = context ? &ts_graphics : NULL;
	ts_width = width;
	ts_height = height;
	memset(ts_calls, 0, sizeof(ts_calls));
	ts_logs = 0;
	ts_level = 0;
	ts_log[0] = '\0';
	ts_x = ts_y = NULL;
}
void oracle_target_size_expect(uint32_t *x, uint32_t *y);
void oracle_target_size_expect(uint32_t *x, uint32_t *y)
{
	ts_x = x;
	ts_y = y;
}
unsigned oracle_target_size_calls(unsigned index);
unsigned oracle_target_size_calls(unsigned index)
{
	assert(index < 3);
	return ts_calls[index];
}
unsigned oracle_target_size_logs(void);
unsigned oracle_target_size_logs(void)
{
	return ts_logs;
}
int oracle_target_size_level(void);
int oracle_target_size_level(void)
{
	return ts_level;
}
const char *oracle_target_size_log(void);
const char *oracle_target_size_log(void)
{
	return ts_log;
}
#undef gs_get_size
#undef gs_get_width
#undef gs_get_height
