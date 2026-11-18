/* Original C and real private adapters over a thread-local recording backend. */
#include <assert.h>
#include <stdarg.h>
#include <stdio.h>
#include <string.h>
/* Rename the optional callback's vtable member along with its public symbol. */
#define gs_get_device_type oracle_gs_get_device_type
#define gs_get_adapter_count oracle_gs_get_adapter_count
#include "graphics/graphics-internal.h"
#include "util/base.h"

int oracle_gs_get_device_type(void);
uint32_t oracle_gs_get_adapter_count(void);

static THREAD_LOCAL struct graphics_subsystem dq_graphics;
static THREAD_LOCAL unsigned dq_type_calls, dq_adapter_calls, dq_logs;
static THREAD_LOCAL int dq_type, dq_level;
static THREAD_LOCAL uint32_t dq_count;
static THREAD_LOCAL char dq_log[160];

#ifdef __GNUC__
__attribute__((format(printf, 2, 3)))
#endif
static void dq_blog(int level, const char *format, ...)
{
	va_list args;
	va_start(args, format);
	dq_logs++;
	dq_level = level;
	vsnprintf(dq_log, sizeof(dq_log), format, args);
	va_end(args);
}

#define blog dq_blog
#include "graphics-device-queries.inc"

/* Original graphics.c.h context validator, verbatim. */
static inline bool gs_valid(const char *f)
{
	if (!thread_graphics) {
		blog(LOG_DEBUG, "%s: called while not in a graphics context", f);
		return false;
	}

	return true;
}

static graphics_t *dq_context(void)
{
	return thread_graphics;
}
#define gs_get_context dq_context
#include "graphics-device-queries-fields.inc"
#undef gs_get_context
#undef blog

static int dq_query_type(void)
{
	assert(thread_graphics == &dq_graphics);
	dq_type_calls++;
	return dq_type;
}

static uint32_t dq_query_adapters(void)
{
	assert(thread_graphics == &dq_graphics);
	dq_adapter_calls++;
	return dq_count;
}

void oracle_device_queries_reset(bool context, bool type_callback, bool adapter_callback, int type, uint32_t count);
void oracle_device_queries_reset(bool context, bool type_callback, bool adapter_callback, int type, uint32_t count)
{
	memset(&dq_graphics, 0, sizeof(dq_graphics));
	dq_graphics.exports.device_get_type = type_callback ? dq_query_type : NULL;
	dq_graphics.exports.gs_get_adapter_count = adapter_callback ? dq_query_adapters : NULL;
	thread_graphics = context ? &dq_graphics : NULL;
	dq_type_calls = dq_adapter_calls = dq_logs = 0;
	dq_type = type;
	dq_count = count;
	dq_level = 0;
	dq_log[0] = '\0';
}

unsigned oracle_device_queries_type_calls(void);
unsigned oracle_device_queries_type_calls(void)
{
	return dq_type_calls;
}

unsigned oracle_device_queries_adapter_calls(void);
unsigned oracle_device_queries_adapter_calls(void)
{
	return dq_adapter_calls;
}

unsigned oracle_device_queries_logs(void);
unsigned oracle_device_queries_logs(void)
{
	return dq_logs;
}

int oracle_device_queries_level(void);
int oracle_device_queries_level(void)
{
	return dq_level;
}

const char *oracle_device_queries_log(void);
const char *oracle_device_queries_log(void)
{
	return dq_log;
}

#undef gs_get_adapter_count
#undef gs_get_device_type
