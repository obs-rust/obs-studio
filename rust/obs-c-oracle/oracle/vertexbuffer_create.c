/* Extracted C implementation and real adapter against a thread-local backend. */
#include <stdarg.h>
#include <stddef.h>
#include <stdio.h>
#include "graphics/graphics-internal.h"
#include "util/base.h"

gs_vertbuffer_t *oracle_gs_vertexbuffer_create(struct gs_vb_data *data, uint32_t flags);
void oracle_vertexbuffer_create_reset(bool present, bool reject);
size_t oracle_vertexbuffer_create_calls(void);
struct gs_vb_data *oracle_vertexbuffer_create_input(void);
uint32_t oracle_vertexbuffer_create_flags(void);
bool oracle_vertexbuffer_create_device_matches(void);
void *oracle_vertexbuffer_create_result(void);
unsigned oracle_vertexbuffer_create_logs(void);
int oracle_vertexbuffer_create_level(void);
const char *oracle_vertexbuffer_create_log(void);
void oracle_vertexbuffer_create_free(struct gs_vb_data *data);
size_t oracle_vertexbuffer_create_layout(size_t key);

static THREAD_LOCAL struct graphics_subsystem vb_graphics;
static THREAD_LOCAL bool vb_present, vb_reject, vb_device_matches;
static THREAD_LOCAL size_t vb_calls;
static THREAD_LOCAL struct gs_vb_data *vb_input;
static THREAD_LOCAL uint32_t vb_flags;
static THREAD_LOCAL unsigned vb_logs;
static THREAD_LOCAL int vb_level;
static THREAD_LOCAL char vb_log[128];

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
#include "vertexbuffer-create-fields.inc"
#define gs_vertexbuffer_create oracle_gs_vertexbuffer_create
#include "vertexbuffer-create.inc"
#undef gs_vertexbuffer_create
#undef blog
#undef gs_get_context

static gs_vertbuffer_t *vb_create(gs_device_t *device, struct gs_vb_data *data, uint32_t flags)
{
	vb_calls++;
	vb_device_matches = (void *)device == (void *)&vb_graphics;
	vb_input = data;
	vb_flags = flags;
	return vb_reject ? NULL : (gs_vertbuffer_t *)&vb_graphics;
}

void oracle_vertexbuffer_create_reset(bool present, bool reject)
{
	vb_present = present;
	vb_reject = reject;
	vb_calls = 0;
	vb_input = NULL;
	vb_flags = 0;
	vb_device_matches = false;
	vb_logs = 0;
	vb_level = 0;
	vb_log[0] = '\0';
	vb_graphics.device = (gs_device_t *)&vb_graphics;
	vb_graphics.exports.device_vertexbuffer_create = vb_create;
}

size_t oracle_vertexbuffer_create_calls(void)
{
	return vb_calls;
}
struct gs_vb_data *oracle_vertexbuffer_create_input(void)
{
	return vb_input;
}
uint32_t oracle_vertexbuffer_create_flags(void)
{
	return vb_flags;
}
bool oracle_vertexbuffer_create_device_matches(void)
{
	return vb_device_matches;
}
void *oracle_vertexbuffer_create_result(void)
{
	return vb_reject ? NULL : &vb_graphics;
}
unsigned oracle_vertexbuffer_create_logs(void)
{
	return vb_logs;
}
int oracle_vertexbuffer_create_level(void)
{
	return vb_level;
}
const char *oracle_vertexbuffer_create_log(void)
{
	return vb_log;
}
void oracle_vertexbuffer_create_free(struct gs_vb_data *data)
{
	gs_vbdata_destroy(data);
}

size_t oracle_vertexbuffer_create_layout(size_t key)
{
	const size_t values[] = {
		sizeof(struct gs_vb_data),
		_Alignof(struct gs_vb_data),
		offsetof(struct gs_vb_data, num),
		offsetof(struct gs_vb_data, points),
		offsetof(struct gs_vb_data, normals),
		offsetof(struct gs_vb_data, tangents),
		offsetof(struct gs_vb_data, colors),
		offsetof(struct gs_vb_data, num_tex),
		offsetof(struct gs_vb_data, tvarray),
		sizeof(struct gs_tvertarray),
		_Alignof(struct gs_tvertarray),
		offsetof(struct gs_tvertarray, width),
		offsetof(struct gs_tvertarray, array),
		sizeof(struct vec3),
		sizeof(float),
	};
	return values[key];
}
