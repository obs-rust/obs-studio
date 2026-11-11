/* Real private layout and production adapter, with a thread-local fake device. */
#include <stdarg.h>
#include <stddef.h>
#include <stdio.h>
#include "graphics/graphics-internal.h"
#include "util/base.h"
#include "util/bmem.h"

gs_indexbuffer_t *oracle_gs_indexbuffer_create(enum gs_index_type type, void *indices, size_t num, uint32_t flags);
void oracle_indexbuffer_reset(bool present, bool reject);
size_t oracle_indexbuffer_calls(void);
void *oracle_indexbuffer_input(void);
int oracle_indexbuffer_type(void);
size_t oracle_indexbuffer_count(void);
uint32_t oracle_indexbuffer_flags(void);
bool oracle_indexbuffer_device_matches(void);
unsigned oracle_indexbuffer_logs(void);
int oracle_indexbuffer_level(void);
const char *oracle_indexbuffer_log(void);
void oracle_indexbuffer_destroy(void);

static THREAD_LOCAL struct graphics_subsystem ib_graphics;
static THREAD_LOCAL bool ib_present;
static THREAD_LOCAL bool ib_reject;
static THREAD_LOCAL size_t ib_calls;
static THREAD_LOCAL void *ib_input;
static THREAD_LOCAL enum gs_index_type ib_type;
static THREAD_LOCAL size_t ib_count;
static THREAD_LOCAL uint32_t ib_flags;
static THREAD_LOCAL bool ib_device_matches;
static THREAD_LOCAL unsigned ib_logs;
static THREAD_LOCAL int ib_level;
static THREAD_LOCAL char ib_log[128];

static graphics_t *ib_context(void)
{
	return ib_present ? &ib_graphics : NULL;
}

#ifdef __GNUC__
__attribute__((format(printf, 2, 3)))
#endif
static void ib_blog(int level, const char *format, ...)
{
	va_list args;
	va_start(args, format);
	ib_logs++;
	ib_level = level;
	vsnprintf(ib_log, sizeof(ib_log), format, args);
	va_end(args);
}

#define gs_get_context ib_context
#define blog ib_blog
#include "indexbuffer-create-fields.inc"
#undef blog
#define gs_indexbuffer_create oracle_gs_indexbuffer_create
#include "indexbuffer-create.inc"
#undef gs_indexbuffer_create
#undef gs_get_context

static gs_indexbuffer_t *ib_create(gs_device_t *device, enum gs_index_type type, void *indices, size_t num,
				   uint32_t flags)
{
	ib_calls++;
	ib_device_matches = (void *)device == (void *)&ib_graphics;
	ib_input = indices;
	ib_type = type;
	ib_count = num;
	ib_flags = flags;
	if (ib_reject) {
		bfree(indices);
		ib_input = NULL;
		return NULL;
	}
	return (gs_indexbuffer_t *)&ib_graphics;
}

void oracle_indexbuffer_reset(bool present, bool reject)
{
	ib_present = present;
	ib_reject = reject;
	ib_calls = 0;
	ib_input = NULL;
	ib_type = GS_UNSIGNED_SHORT;
	ib_count = 0;
	ib_flags = 0;
	ib_device_matches = false;
	ib_logs = 0;
	ib_level = 0;
	ib_log[0] = '\0';
	ib_graphics.device = (gs_device_t *)&ib_graphics;
	ib_graphics.exports.device_indexbuffer_create = ib_create;
}

size_t oracle_indexbuffer_calls(void)
{
	return ib_calls;
}
void *oracle_indexbuffer_input(void)
{
	return ib_input;
}
int oracle_indexbuffer_type(void)
{
	return (int)ib_type;
}
size_t oracle_indexbuffer_count(void)
{
	return ib_count;
}
uint32_t oracle_indexbuffer_flags(void)
{
	return ib_flags;
}
bool oracle_indexbuffer_device_matches(void)
{
	return ib_device_matches;
}
unsigned oracle_indexbuffer_logs(void)
{
	return ib_logs;
}
int oracle_indexbuffer_level(void)
{
	return ib_level;
}
const char *oracle_indexbuffer_log(void)
{
	return ib_log;
}
void oracle_indexbuffer_destroy(void)
{
	bfree(ib_input);
	ib_input = NULL;
}
