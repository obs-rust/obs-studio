/* Original C and real private adapters over a thread-local recording backend. */
#include <stdarg.h>
#include <stddef.h>
#include <stdio.h>
#include <string.h>
#define gs_shader_get_viewproj_matrix oracle_gs_shader_get_viewproj_matrix
#define gs_shader_get_world_matrix oracle_gs_shader_get_world_matrix
#include "graphics/graphics-internal.h"
#include "util/base.h"

gs_sparam_t *oracle_gs_shader_get_viewproj_matrix(const gs_shader_t *shader);
gs_sparam_t *oracle_gs_shader_get_world_matrix(const gs_shader_t *shader);

static THREAD_LOCAL struct graphics_subsystem sm_graphics;
static THREAD_LOCAL int sm_device_token;
static THREAD_LOCAL unsigned sm_calls[2], sm_logs;
static THREAD_LOCAL int sm_level;
static THREAD_LOCAL const void *sm_shader;
static THREAD_LOCAL gs_sparam_t *sm_results[2];
static THREAD_LOCAL char sm_log[160], sm_events[64];
static THREAD_LOCAL size_t sm_event_count;

static void sm_record(char event)
{
	if (sm_event_count + 1 < sizeof(sm_events)) {
		sm_events[sm_event_count++] = event;
		sm_events[sm_event_count] = '\0';
	}
}

#ifdef __GNUC__
__attribute__((format(printf, 2, 3)))
#endif
static void sm_blog(int level, const char *format, ...)
{
	va_list args;
	va_start(args, format);
	sm_logs++;
	sm_level = level;
	vsnprintf(sm_log, sizeof(sm_log), format, args);
	va_end(args);
	sm_record('L');
}

#define blog sm_blog
#include "graphics-shader-matrix-queries.inc"

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

static graphics_t *sm_context(void)
{
	return thread_graphics;
}
#define gs_get_context sm_context
#include "graphics-shader-matrix-queries-fields.inc"
#undef gs_get_context
#undef blog

static gs_sparam_t *sm_query(unsigned index, const gs_shader_t *shader)
{
	sm_calls[index]++;
	sm_shader = shader;
	sm_record(index ? 'W' : 'V');
	return sm_results[index];
}
static gs_sparam_t *sm_viewproj(const gs_shader_t *shader)
{
	return sm_query(0, shader);
}
static gs_sparam_t *sm_world(const gs_shader_t *shader)
{
	return sm_query(1, shader);
}

/* callbacks: bit 0 installs viewproj, bit 1 installs world. */
void oracle_shader_matrix_reset(bool context, unsigned callbacks, gs_sparam_t *viewproj, gs_sparam_t *world);
void oracle_shader_matrix_reset(bool context, unsigned callbacks, gs_sparam_t *viewproj, gs_sparam_t *world)
{
	memset(&sm_graphics, 0, sizeof(sm_graphics));
	sm_graphics.device = (gs_device_t *)&sm_device_token;
	sm_graphics.exports.gs_shader_get_viewproj_matrix = (callbacks & 1) ? sm_viewproj : NULL;
	sm_graphics.exports.gs_shader_get_world_matrix = (callbacks & 2) ? sm_world : NULL;
	thread_graphics = context ? &sm_graphics : NULL;
	memset(sm_calls, 0, sizeof(sm_calls));
	sm_logs = 0;
	sm_level = 0;
	sm_shader = NULL;
	sm_results[0] = viewproj;
	sm_results[1] = world;
	sm_log[0] = sm_events[0] = '\0';
	sm_event_count = 0;
}
unsigned oracle_shader_matrix_calls(unsigned index);
unsigned oracle_shader_matrix_calls(unsigned index)
{
	return index < 2 ? sm_calls[index] : 0;
}
unsigned oracle_shader_matrix_logs(void);
unsigned oracle_shader_matrix_logs(void)
{
	return sm_logs;
}
int oracle_shader_matrix_level(void);
int oracle_shader_matrix_level(void)
{
	return sm_level;
}
const void *oracle_shader_matrix_shader(void);
const void *oracle_shader_matrix_shader(void)
{
	return sm_shader;
}
const char *oracle_shader_matrix_log(void);
const char *oracle_shader_matrix_log(void)
{
	return sm_log;
}
const char *oracle_shader_matrix_events(void);
const char *oracle_shader_matrix_events(void)
{
	return sm_events;
}

#undef gs_shader_get_viewproj_matrix
#undef gs_shader_get_world_matrix
