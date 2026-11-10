/* Compile the extracted C body and the real adapter against the real header. */
#include <stdarg.h>
#include <stdint.h>
#include <stdio.h>
#define gs_vertexbuffer_destroy oracle_gs_vertexbuffer_destroy
#include "graphics/graphics-internal.h"
#include "util/base.h"

void oracle_gs_vertexbuffer_destroy(gs_vertbuffer_t *buffer);
void oracle_vertexbuffer_reset(bool present, bool second);
unsigned oracle_vertexbuffer_calls(void);
uintptr_t oracle_vertexbuffer_pointer(void);
unsigned oracle_vertexbuffer_device(void);
unsigned oracle_vertexbuffer_logs(void);
int oracle_vertexbuffer_level(void);
const char *oracle_vertexbuffer_log(void);

static THREAD_LOCAL struct graphics_subsystem vb_graphics;
static THREAD_LOCAL bool vb_present;
static THREAD_LOCAL unsigned vb_calls;
static THREAD_LOCAL uintptr_t vb_pointer;
static THREAD_LOCAL unsigned vb_device;
static THREAD_LOCAL unsigned vb_logs;
static THREAD_LOCAL int vb_level;
static THREAD_LOCAL char vb_log[128];

static void vb_first(gs_vertbuffer_t *buffer)
{
	vb_calls++;
	vb_pointer = (uintptr_t)buffer;
	vb_device = 1;
}

static void vb_second(gs_vertbuffer_t *buffer)
{
	vb_calls++;
	vb_pointer = (uintptr_t)buffer;
	vb_device = 2;
}

static graphics_t *vb_context(void)
{
	return vb_present ? &vb_graphics : NULL;
}

#ifdef __GNUC__
__attribute__((format(printf, 2, 3)))
#endif
static void vb_blog(int level, const char *format, ...)
{
	va_list args;
	va_start(args, format);
	vb_logs++;
	vb_level = level;
	vsnprintf(vb_log, sizeof(vb_log), format, args);
	va_end(args);
}

#define gs_get_context vb_context
#define blog vb_blog
#include "vertexbuffer-destroy-fields.inc"
#undef blog
#include "vertexbuffer-destroy.inc"
#undef gs_get_context

void oracle_vertexbuffer_reset(bool present, bool second)
{
	vb_present = present;
	vb_graphics.exports.gs_vertexbuffer_destroy = second ? vb_second : vb_first;
	vb_calls = 0;
	vb_pointer = 0;
	vb_device = 0;
	vb_logs = 0;
	vb_level = 0;
	vb_log[0] = '\0';
}

unsigned oracle_vertexbuffer_calls(void)
{
	return vb_calls;
}
uintptr_t oracle_vertexbuffer_pointer(void)
{
	return vb_pointer;
}
unsigned oracle_vertexbuffer_device(void)
{
	return vb_device;
}
unsigned oracle_vertexbuffer_logs(void)
{
	return vb_logs;
}
int oracle_vertexbuffer_level(void)
{
	return vb_level;
}
const char *oracle_vertexbuffer_log(void)
{
	return vb_log;
}

#undef gs_vertexbuffer_destroy
