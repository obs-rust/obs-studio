/* Original C and real private adapters over one thread-local recording backend. */
#include <stdarg.h>
#include <stddef.h>
#include <stdio.h>
#include <string.h>
/* Rename the same-named vtable members together with the public functions. */
#define gs_indexbuffer_destroy oracle_gs_indexbuffer_destroy
#define gs_indexbuffer_flush oracle_gs_indexbuffer_flush
#define gs_indexbuffer_flush_direct oracle_gs_indexbuffer_flush_direct
#define gs_indexbuffer_get_num_indices oracle_gs_indexbuffer_get_num_indices
#include "graphics/graphics-internal.h"
#include "util/base.h"

void oracle_gs_indexbuffer_destroy(gs_indexbuffer_t *buffer);
void oracle_gs_indexbuffer_flush(gs_indexbuffer_t *buffer);
void oracle_gs_indexbuffer_flush_direct(gs_indexbuffer_t *buffer, const void *data);
size_t oracle_gs_indexbuffer_get_num_indices(const gs_indexbuffer_t *buffer);

static THREAD_LOCAL struct graphics_subsystem ibm_graphics;
static THREAD_LOCAL unsigned ibm_calls, ibm_logs;
static THREAD_LOCAL int ibm_kind, ibm_level;
static THREAD_LOCAL const void *ibm_buffer, *ibm_data;
static THREAD_LOCAL size_t ibm_count;
static THREAD_LOCAL char ibm_log[160];

#ifdef __GNUC__
__attribute__((format(printf, 2, 3)))
#endif
static void ibm_blog(int level, const char *format, ...)
{
	va_list args;
	va_start(args, format);
	ibm_logs++;
	ibm_level = level;
	vsnprintf(ibm_log, sizeof(ibm_log), format, args);
	va_end(args);
}

#define blog ibm_blog
#include "graphics-indexbuffer-maintenance.inc"

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

static graphics_t *ibm_context(void)
{
	return thread_graphics;
}
#define gs_get_context ibm_context
#include "graphics-indexbuffer-maintenance-fields.inc"
#undef gs_get_context
#undef blog

static void ibm_record(int kind, const gs_indexbuffer_t *buffer, const void *data)
{
	ibm_calls++;
	ibm_kind = kind;
	ibm_buffer = buffer;
	ibm_data = data;
}

static void ibm_destroy(gs_indexbuffer_t *buffer)
{
	ibm_record(0, buffer, NULL);
}

static void ibm_flush(gs_indexbuffer_t *buffer)
{
	ibm_record(1, buffer, NULL);
}

static void ibm_direct(gs_indexbuffer_t *buffer, const void *data)
{
	ibm_record(2, buffer, data);
}

static size_t ibm_num_indices(const gs_indexbuffer_t *buffer)
{
	ibm_record(3, buffer, NULL);
	return ibm_count;
}

void oracle_indexbuffer_maintenance_reset(bool context, size_t count);
void oracle_indexbuffer_maintenance_reset(bool context, size_t count)
{
	memset(&ibm_graphics, 0, sizeof(ibm_graphics));
	ibm_graphics.exports.gs_indexbuffer_destroy = ibm_destroy;
	ibm_graphics.exports.gs_indexbuffer_flush = ibm_flush;
	ibm_graphics.exports.gs_indexbuffer_flush_direct = ibm_direct;
	ibm_graphics.exports.gs_indexbuffer_get_num_indices = ibm_num_indices;
	thread_graphics = context ? &ibm_graphics : NULL;
	ibm_calls = ibm_logs = 0;
	ibm_kind = -1;
	ibm_level = 0;
	ibm_buffer = ibm_data = NULL;
	ibm_count = count;
	ibm_log[0] = '\0';
}

unsigned oracle_indexbuffer_maintenance_calls(void);
unsigned oracle_indexbuffer_maintenance_calls(void)
{
	return ibm_calls;
}

unsigned oracle_indexbuffer_maintenance_logs(void);
unsigned oracle_indexbuffer_maintenance_logs(void)
{
	return ibm_logs;
}

int oracle_indexbuffer_maintenance_kind(void);
int oracle_indexbuffer_maintenance_kind(void)
{
	return ibm_kind;
}

int oracle_indexbuffer_maintenance_level(void);
int oracle_indexbuffer_maintenance_level(void)
{
	return ibm_level;
}

const void *oracle_indexbuffer_maintenance_buffer(void);
const void *oracle_indexbuffer_maintenance_buffer(void)
{
	return ibm_buffer;
}

const void *oracle_indexbuffer_maintenance_data(void);
const void *oracle_indexbuffer_maintenance_data(void)
{
	return ibm_data;
}

const char *oracle_indexbuffer_maintenance_log(void);
const char *oracle_indexbuffer_maintenance_log(void)
{
	return ibm_log;
}

#undef gs_indexbuffer_destroy
#undef gs_indexbuffer_flush
#undef gs_indexbuffer_flush_direct
#undef gs_indexbuffer_get_num_indices
