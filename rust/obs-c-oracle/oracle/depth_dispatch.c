/* GPU-free, per-thread recording of the original extracted C bodies. */
#include <stdarg.h>
#include <stdio.h>
#include <string.h>
#define gs_enable_depth_test oracle_gs_enable_depth_test
#define gs_depth_function oracle_gs_depth_function
#include "graphics/graphics-internal.h"
#include "util/base.h"
static THREAD_LOCAL struct graphics_subsystem dd_graphics;
static THREAD_LOCAL graphics_t *dd_context;
static THREAD_LOCAL unsigned char dd_devices[2];
static THREAD_LOCAL unsigned dd_calls, dd_logs;
static THREAD_LOCAL unsigned dd_kinds[256], dd_values[256], dd_identity[256];
static THREAD_LOCAL int dd_level;
static THREAD_LOCAL char dd_message[160];
static graphics_t *dd_get_context(void)
{
	return dd_context;
}
#ifdef __GNUC__
__attribute__((format(printf, 2, 3)))
#endif
static void dd_blog(int level, const char *format, ...)
{
	va_list args;
	va_start(args, format);
	dd_level = level;
	dd_logs++;
	vsnprintf(dd_message, sizeof(dd_message), format, args);
	va_end(args);
}
#define gs_get_context dd_get_context
#define blog dd_blog
#include "state-depth-entrypoints.inc"
#include "depth-dispatch-fields.inc"
#undef blog
#undef gs_get_context
#undef gs_enable_depth_test
#undef gs_depth_function
static void dd_record(gs_device_t *device, unsigned kind, unsigned value)
{
	unsigned n = dd_calls++;
	if (n >= 256)
		return;
	dd_kinds[n] = kind;
	dd_values[n] = value;
	dd_identity[n] = device == (gs_device_t *)&dd_devices[0] ? 0 : device == (gs_device_t *)&dd_devices[1] ? 1 : 2;
}
static void dd_enable(gs_device_t *device, bool enable)
{
	dd_record(device, 0, enable);
}
static void dd_function(gs_device_t *device, enum gs_depth_test test)
{
	dd_record(device, 1, (unsigned)test);
}
void oracle_depth_begin(void);
void oracle_depth_begin(void)
{
	memset(&dd_graphics, 0, sizeof(dd_graphics));
	dd_graphics.exports.device_enable_depth_test = dd_enable;
	dd_graphics.exports.device_depth_function = dd_function;
	dd_context = NULL;
	dd_calls = dd_logs = 0;
	dd_level = 0;
	dd_message[0] = 0;
}
void oracle_depth_context(bool present, unsigned device);
void oracle_depth_context(bool present, unsigned device)
{
	dd_graphics.device = (gs_device_t *)&dd_devices[device];
	dd_context = present ? &dd_graphics : NULL;
}
unsigned oracle_depth_calls(void);
unsigned oracle_depth_calls(void)
{
	return dd_calls;
}
unsigned oracle_depth_logs(void);
unsigned oracle_depth_logs(void)
{
	return dd_logs;
}
int oracle_depth_level(void);
int oracle_depth_level(void)
{
	return dd_level;
}
const char *oracle_depth_message(void);
const char *oracle_depth_message(void)
{
	return dd_message;
}
unsigned oracle_depth_kind(unsigned n);
unsigned oracle_depth_kind(unsigned n)
{
	return dd_kinds[n];
}
unsigned oracle_depth_value(unsigned n);
unsigned oracle_depth_value(unsigned n)
{
	return dd_values[n];
}
unsigned oracle_depth_device(unsigned n);
unsigned oracle_depth_device(unsigned n)
{
	return dd_identity[n];
}
unsigned oracle_depth_enum(unsigned n);
unsigned oracle_depth_enum(unsigned n)
{
	const enum gs_depth_test values[] = {GS_NEVER,  GS_LESS,    GS_LEQUAL,   GS_EQUAL,
					     GS_GEQUAL, GS_GREATER, GS_NOTEQUAL, GS_ALWAYS};
	return (unsigned)values[n];
}
size_t oracle_depth_size(void);
size_t oracle_depth_size(void)
{
	return sizeof(enum gs_depth_test);
}
size_t oracle_depth_align(void);
size_t oracle_depth_align(void)
{
	return _Alignof(enum gs_depth_test);
}
