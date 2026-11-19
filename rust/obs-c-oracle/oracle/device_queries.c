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

#define gs_nv12_available oracle_gs_nv12_available
#define gs_p010_available oracle_gs_p010_available
bool oracle_gs_nv12_available(void);
bool oracle_gs_p010_available(void);
#include "graphics-planar-capability-queries.inc"
#undef gs_p010_available
#undef gs_nv12_available

#define gs_get_vertex_shader oracle_gs_get_vertex_shader
#define gs_get_pixel_shader oracle_gs_get_pixel_shader
gs_shader_t *oracle_gs_get_vertex_shader(void);
gs_shader_t *oracle_gs_get_pixel_shader(void);
#include "graphics-active-shader-queries.inc"
#undef gs_get_pixel_shader
#undef gs_get_vertex_shader

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

/* Planar queries share the real device-query TLS/logging adapters above. */
static THREAD_LOCAL int dq_planar_devices[2];
static THREAD_LOCAL bool dq_planar_results[2];
static THREAD_LOCAL unsigned dq_planar_calls[2], dq_planar_last_device;

static bool dq_planar_nv12(gs_device_t *device)
{
	assert(thread_graphics == &dq_graphics);
	assert(device == dq_graphics.device);
	assert(device == (gs_device_t *)&dq_planar_devices[0] || device == (gs_device_t *)&dq_planar_devices[1]);
	dq_planar_last_device = device == (gs_device_t *)&dq_planar_devices[1] ? 1u : 0u;
	dq_planar_calls[0]++;
	return dq_planar_results[0];
}

static bool dq_planar_p010(gs_device_t *device)
{
	assert(thread_graphics == &dq_graphics);
	assert(device == dq_graphics.device);
	assert(device == (gs_device_t *)&dq_planar_devices[0] || device == (gs_device_t *)&dq_planar_devices[1]);
	dq_planar_last_device = device == (gs_device_t *)&dq_planar_devices[1] ? 1u : 0u;
	dq_planar_calls[1]++;
	return dq_planar_results[1];
}

void oracle_planar_capability_reset(bool context, bool nv12, bool p010, bool nv12_result, bool p010_result,
				    bool device);
void oracle_planar_capability_reset(bool context, bool nv12, bool p010, bool nv12_result, bool p010_result, bool device)
{
	oracle_device_queries_reset(context, false, false, 0, 0);
	dq_graphics.device = (gs_device_t *)&dq_planar_devices[device ? 1 : 0];
	dq_graphics.exports.device_nv12_available = nv12 ? dq_planar_nv12 : NULL;
	dq_graphics.exports.device_p010_available = p010 ? dq_planar_p010 : NULL;
	dq_planar_results[0] = nv12_result;
	dq_planar_results[1] = p010_result;
	dq_planar_calls[0] = dq_planar_calls[1] = 0;
	dq_planar_last_device = 2;
}

unsigned oracle_planar_capability_calls(bool p010);
unsigned oracle_planar_capability_calls(bool p010)
{
	return dq_planar_calls[p010 ? 1 : 0];
}

unsigned oracle_planar_capability_last_device(void);
unsigned oracle_planar_capability_last_device(void)
{
	return dq_planar_last_device;
}

/* Active shader callbacks use the export table's const gs_device_t signature. */
static THREAD_LOCAL int dq_shader_devices[2], dq_shader_tokens[2];
static THREAD_LOCAL gs_shader_t *dq_shader_results[2];
static THREAD_LOCAL unsigned dq_shader_calls[2], dq_shader_last_device;

static gs_shader_t *dq_vertex_shader(const gs_device_t *device)
{
	assert(thread_graphics == &dq_graphics);
	assert(dq_graphics.ref == 1);
	assert(device == dq_graphics.device);
	assert(device == (gs_device_t *)&dq_shader_devices[0] || device == (gs_device_t *)&dq_shader_devices[1]);
	dq_shader_last_device = device == (gs_device_t *)&dq_shader_devices[1] ? 1u : 0u;
	dq_shader_calls[0]++;
	return dq_shader_results[0];
}

static gs_shader_t *dq_pixel_shader(const gs_device_t *device)
{
	assert(thread_graphics == &dq_graphics);
	assert(dq_graphics.ref == 1);
	assert(device == dq_graphics.device);
	assert(device == (gs_device_t *)&dq_shader_devices[0] || device == (gs_device_t *)&dq_shader_devices[1]);
	dq_shader_last_device = device == (gs_device_t *)&dq_shader_devices[1] ? 1u : 0u;
	dq_shader_calls[1]++;
	return dq_shader_results[1];
}

gs_shader_t *oracle_active_shader_token(unsigned token);
gs_shader_t *oracle_active_shader_token(unsigned token)
{
	assert(token <= 2);
	return token ? (gs_shader_t *)&dq_shader_tokens[token - 1] : NULL;
}

void oracle_active_shader_context(bool context);
void oracle_active_shader_context(bool context)
{
	thread_graphics = context ? &dq_graphics : NULL;
	dq_graphics.ref = context ? 1 : 0;
}

void oracle_active_shader_reset(bool vertex, bool pixel, unsigned vertex_result, unsigned pixel_result, bool device);
void oracle_active_shader_reset(bool vertex, bool pixel, unsigned vertex_result, unsigned pixel_result, bool device)
{
	oracle_device_queries_reset(false, false, false, 0, 0);
	dq_graphics.device = (gs_device_t *)&dq_shader_devices[device ? 1 : 0];
	dq_graphics.exports.device_get_vertex_shader = vertex ? dq_vertex_shader : NULL;
	dq_graphics.exports.device_get_pixel_shader = pixel ? dq_pixel_shader : NULL;
	dq_shader_tokens[0] = 17;
	dq_shader_tokens[1] = 29;
	dq_shader_results[0] = oracle_active_shader_token(vertex_result);
	dq_shader_results[1] = oracle_active_shader_token(pixel_result);
	dq_shader_calls[0] = dq_shader_calls[1] = 0;
	dq_shader_last_device = 2;
}

unsigned oracle_active_shader_calls(bool pixel);
unsigned oracle_active_shader_calls(bool pixel)
{
	return dq_shader_calls[pixel ? 1 : 0];
}

unsigned oracle_active_shader_last_device(void);
unsigned oracle_active_shader_last_device(void)
{
	return dq_shader_last_device;
}

int oracle_active_shader_ref(void);
int oracle_active_shader_ref(void)
{
	assert(dq_shader_tokens[0] == 17 && dq_shader_tokens[1] == 29);
	assert(dq_graphics.ref == 0 || dq_graphics.ref == 1);
	return dq_graphics.ref == 1 ? 1 : 0;
}
