/* The real private header, original C dispatch and production adapter. */
#include <stdarg.h>
#include <stdio.h>
#include <string.h>
#include "graphics/graphics-internal.h"
#include "util/base.h"

void oracle_stencil_reset(bool present);
int oracle_stencil_value(unsigned index);
const char *oracle_stencil_log(void);
void oracle_gs_enable_stencil_test(bool enable);
void oracle_gs_enable_stencil_write(bool enable);
void oracle_gs_stencil_function(enum gs_stencil_side side, enum gs_depth_test test);
void oracle_gs_stencil_op(enum gs_stencil_side side, enum gs_stencil_op_type fail, enum gs_stencil_op_type zfail,
			  enum gs_stencil_op_type zpass);

static THREAD_LOCAL struct graphics_subsystem stencil_graphics;
static THREAD_LOCAL bool stencil_present;
/* calls, operation, enabled, side, comparison, fail, zfail, zpass, device matches,
 * logs, level. No C layout is duplicated by the Rust tests. */
static THREAD_LOCAL int stencil_values[11];
static THREAD_LOCAL char stencil_log[128];
static THREAD_LOCAL int stencil_device;

static graphics_t *stencil_context(void)
{
	return stencil_present ? &stencil_graphics : NULL;
}

#ifdef __GNUC__
__attribute__((format(printf, 2, 3)))
#endif
static void stencil_blog(int level, const char *format, ...)
{
	va_list args;
	va_start(args, format);
	stencil_values[9]++;
	stencil_values[10] = level;
	vsnprintf(stencil_log, sizeof(stencil_log), format, args);
	va_end(args);
}

static void stencil_record(gs_device_t *device, int operation)
{
	stencil_values[0]++;
	stencil_values[1] = operation;
	stencil_values[8] = device == (gs_device_t *)&stencil_device;
}
static void stencil_test(gs_device_t *device, bool enable)
{
	stencil_record(device, 1);
	stencil_values[2] = enable;
}
static void stencil_write(gs_device_t *device, bool enable)
{
	stencil_record(device, 2);
	stencil_values[2] = enable;
}
static void stencil_function(gs_device_t *device, enum gs_stencil_side side, enum gs_depth_test test)
{
	stencil_record(device, 3);
	stencil_values[3] = (int)side;
	stencil_values[4] = (int)test;
}
static void stencil_op(gs_device_t *device, enum gs_stencil_side side, enum gs_stencil_op_type fail,
		       enum gs_stencil_op_type zfail, enum gs_stencil_op_type zpass)
{
	stencil_record(device, 4);
	stencil_values[3] = (int)side;
	stencil_values[5] = (int)fail;
	stencil_values[6] = (int)zfail;
	stencil_values[7] = (int)zpass;
}

#define gs_get_context stencil_context
#define blog stencil_blog
#include "graphics-stencil-adapter.inc"
#undef blog
#define gs_enable_stencil_test oracle_gs_enable_stencil_test
#define gs_enable_stencil_write oracle_gs_enable_stencil_write
#define gs_stencil_function oracle_gs_stencil_function
#define gs_stencil_op oracle_gs_stencil_op
#include "graphics-stencil.inc"
#undef gs_stencil_op
#undef gs_stencil_function
#undef gs_enable_stencil_write
#undef gs_enable_stencil_test
#undef gs_get_context

void oracle_stencil_reset(bool present)
{
	stencil_present = present;
	memset(stencil_values, 0, sizeof(stencil_values));
	stencil_log[0] = '\0';
	stencil_graphics.device = (gs_device_t *)&stencil_device;
	stencil_graphics.exports.device_enable_stencil_test = stencil_test;
	stencil_graphics.exports.device_enable_stencil_write = stencil_write;
	stencil_graphics.exports.device_stencil_function = stencil_function;
	stencil_graphics.exports.device_stencil_op = stencil_op;
}
int oracle_stencil_value(unsigned index)
{
	return index < 11 ? stencil_values[index] : -1;
}
const char *oracle_stencil_log(void)
{
	return stencil_log;
}
