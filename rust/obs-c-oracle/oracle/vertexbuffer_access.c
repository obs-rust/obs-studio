/* Real private graphics layout, real adapter and extracted original wrappers. */
#include <stdarg.h>
#include <stddef.h>
#include <stdio.h>
#define gs_vertexbuffer_flush oracle_gs_vertexbuffer_flush
#define gs_vertexbuffer_get_data oracle_gs_vertexbuffer_get_data
#include "graphics/graphics-internal.h"
#include "util/base.h"

void oracle_vertexbuffer_access_reset(bool present, bool result_present);
void oracle_vertexbuffer_access_leave(void);
void *oracle_vertexbuffer_access_buffer(void);
void *oracle_vertexbuffer_access_result(void);
unsigned oracle_vertexbuffer_access_calls(void);
const void *oracle_vertexbuffer_access_seen(void);
unsigned oracle_vertexbuffer_access_logs(void);
int oracle_vertexbuffer_access_level(void);
const char *oracle_vertexbuffer_access_log(void);
size_t oracle_vertexbuffer_access_pointer_size(void);
size_t oracle_vertexbuffer_access_pointer_align(void);
void oracle_gs_vertexbuffer_flush(gs_vertbuffer_t *buffer);
struct gs_vb_data *oracle_gs_vertexbuffer_get_data(const gs_vertbuffer_t *buffer);

static THREAD_LOCAL struct graphics_subsystem vba_graphics;
static THREAD_LOCAL bool vba_present;
static THREAD_LOCAL bool vba_result_present;
static THREAD_LOCAL unsigned vba_calls;
static THREAD_LOCAL const void *vba_seen;
static THREAD_LOCAL unsigned vba_logs;
static THREAD_LOCAL int vba_level;
static THREAD_LOCAL char vba_log[128];
static THREAD_LOCAL unsigned char vba_buffer;
static THREAD_LOCAL struct gs_vb_data vba_result;

static graphics_t *vba_context(void)
{
	return vba_present ? &vba_graphics : NULL;
}

#ifdef __GNUC__
__attribute__((format(printf, 2, 3)))
#endif
static void vba_blog(int level, const char *format, ...)
{
	va_list args;
	va_start(args, format);
	vba_logs++;
	vba_level = level;
	vsnprintf(vba_log, sizeof(vba_log), format, args);
	va_end(args);
}

static void vba_flush(gs_vertbuffer_t *buffer)
{
	vba_calls++;
	vba_seen = buffer;
}

static struct gs_vb_data *vba_get_data(const gs_vertbuffer_t *buffer)
{
	vba_calls++;
	vba_seen = buffer;
	return vba_result_present ? &vba_result : NULL;
}

#define gs_get_context vba_context
#define blog vba_blog
#include "vertexbuffer-access-fields.inc"
#include "vertexbuffer-access.inc"
#undef blog
#undef gs_get_context

void oracle_vertexbuffer_access_reset(bool present, bool result_present)
{
	vba_present = present;
	vba_result_present = result_present;
	vba_graphics.exports.gs_vertexbuffer_flush = vba_flush;
	vba_graphics.exports.gs_vertexbuffer_get_data = vba_get_data;
	vba_calls = 0;
	vba_seen = NULL;
	vba_logs = 0;
	vba_level = 0;
	vba_log[0] = '\0';
}

void oracle_vertexbuffer_access_leave(void)
{
	vba_present = false;
}

void *oracle_vertexbuffer_access_buffer(void)
{
	return &vba_buffer;
}
void *oracle_vertexbuffer_access_result(void)
{
	return vba_result_present ? &vba_result : NULL;
}
unsigned oracle_vertexbuffer_access_calls(void)
{
	return vba_calls;
}
const void *oracle_vertexbuffer_access_seen(void)
{
	return vba_seen;
}
unsigned oracle_vertexbuffer_access_logs(void)
{
	return vba_logs;
}
int oracle_vertexbuffer_access_level(void)
{
	return vba_level;
}
const char *oracle_vertexbuffer_access_log(void)
{
	return vba_log;
}
size_t oracle_vertexbuffer_access_pointer_size(void)
{
	return sizeof(void *);
}
size_t oracle_vertexbuffer_access_pointer_align(void)
{
	return _Alignof(void *);
}

#undef gs_vertexbuffer_get_data
#undef gs_vertexbuffer_flush
