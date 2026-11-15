/* Tier 3 for obs-graphics::perspective: the original gs_perspective
 * (graphics-perspective.c.h, renamed oracle_gs_perspective, with its own
 * copy of gs_valid) and the production adapter (perspective-fields.c.h,
 * under its real names, which the Rust shim calls) over the real private
 * graphics layout, a thread-local stand-in context, a recording
 * device_frustum and a recording blog. */
#include <stdarg.h>
#include <stddef.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "graphics/graphics-internal.h"
#include "util/base.h"

void oracle_gs_perspective(float angle, float aspect, float near_z, float far_z);

/* One device_frustum call: the device and its six float arguments. */
struct oracle_perspective_event {
	const void *device;
	float args[6];
};

#define PC_MAX_EVENTS 4

void oracle_perspective_reset(bool context, void *device);
size_t oracle_perspective_events(struct oracle_perspective_event *out, size_t max);
unsigned oracle_perspective_logs(void);
int oracle_perspective_level(void);
const char *oracle_perspective_log(void);

static THREAD_LOCAL struct graphics_subsystem pc_graphics;
static THREAD_LOCAL bool pc_has_context;
static THREAD_LOCAL struct oracle_perspective_event pc_events[PC_MAX_EVENTS];
static THREAD_LOCAL size_t pc_num_events;
static THREAD_LOCAL unsigned pc_logs;
static THREAD_LOCAL int pc_level;
static THREAD_LOCAL char pc_log[160];

static graphics_t *pc_context(void)
{
	return pc_has_context ? &pc_graphics : NULL;
}

#ifdef __GNUC__
__attribute__((format(printf, 2, 3)))
#endif
static void pc_blog(int level, const char *format, ...)
{
	va_list args;
	va_start(args, format);
	pc_logs++;
	pc_level = level;
	vsnprintf(pc_log, sizeof(pc_log), format, args);
	va_end(args);
}

#define gs_get_context pc_context
#define blog pc_blog
#include "perspective-fields.inc"
#define gs_perspective oracle_gs_perspective
#include "graphics-perspective.inc"
#undef gs_perspective
#undef blog
#undef gs_get_context

static void pc_frustum(gs_device_t *device, float left, float right, float top, float bottom, float znear, float zfar)
{
	/* An overflow is a test bug: one call never makes more than one event. */
	if (pc_num_events >= PC_MAX_EVENTS)
		abort();
	struct oracle_perspective_event *event = &pc_events[pc_num_events++];
	event->device = device;
	event->args[0] = left;
	event->args[1] = right;
	event->args[2] = top;
	event->args[3] = bottom;
	event->args[4] = znear;
	event->args[5] = zfar;
}

void oracle_perspective_reset(bool context, void *device)
{
	memset(&pc_graphics, 0, sizeof(pc_graphics));
	pc_graphics.device = (gs_device_t *)device;
	pc_graphics.exports.device_frustum = pc_frustum;
	pc_has_context = context;
	pc_num_events = 0;
	pc_logs = 0;
	pc_level = 0;
	pc_log[0] = '\0';
}

size_t oracle_perspective_events(struct oracle_perspective_event *out, size_t max)
{
	size_t n = pc_num_events < max ? pc_num_events : max;
	memcpy(out, pc_events, n * sizeof(*out));
	return n;
}

unsigned oracle_perspective_logs(void)
{
	return pc_logs;
}

int oracle_perspective_level(void)
{
	return pc_level;
}

const char *oracle_perspective_log(void)
{
	return pc_log;
}
