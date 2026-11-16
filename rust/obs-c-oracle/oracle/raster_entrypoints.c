/* Tier 3 for obs-graphics::raster_entrypoints: the original cull and colour
 * calls (raster-state-entrypoints.c.h, renamed oracle_gs_*) and the
 * production adapter (raster-state-entrypoints-fields.c.h, under its real
 * names, which the Rust shims call) over the real private graphics layout,
 * a thread-local stand-in context, a recording device and a recording blog. */
#include <stdarg.h>
#include <stddef.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "graphics/graphics-internal.h"
#include "util/base.h"

void oracle_gs_set_cull_mode(enum gs_cull_mode mode);
enum gs_cull_mode oracle_gs_get_cull_mode(void);
void oracle_gs_enable_color(bool red, bool green, bool blue, bool alpha);

/* One device call; `call` is 0..2 in the order of the C file. */
struct oracle_raster_event {
	int call;
	const void *device;
	int cull_mode;
	bool rgba[4];
};

#define RE_MAX_EVENTS 4

void oracle_raster_reset(bool context, void *device, int cull_answer);
size_t oracle_raster_events(struct oracle_raster_event *out, size_t max);
unsigned oracle_raster_logs(void);
int oracle_raster_level(void);
const char *oracle_raster_log(void);

static THREAD_LOCAL struct graphics_subsystem re_graphics;
static THREAD_LOCAL bool re_has_context;
static THREAD_LOCAL int re_cull_answer;
static THREAD_LOCAL struct oracle_raster_event re_events[RE_MAX_EVENTS];
static THREAD_LOCAL size_t re_num_events;
static THREAD_LOCAL unsigned re_logs;
static THREAD_LOCAL int re_level;
static THREAD_LOCAL char re_log[160];

static graphics_t *re_context(void)
{
	return re_has_context ? &re_graphics : NULL;
}

#ifdef __GNUC__
__attribute__((format(printf, 2, 3)))
#endif
static void re_blog(int level, const char *format, ...)
{
	va_list args;
	va_start(args, format);
	re_logs++;
	re_level = level;
	vsnprintf(re_log, sizeof(re_log), format, args);
	va_end(args);
}

#define gs_get_context re_context
#define blog re_blog
#include "raster-state-entrypoints-fields.inc"
#define gs_set_cull_mode oracle_gs_set_cull_mode
#define gs_get_cull_mode oracle_gs_get_cull_mode
#define gs_enable_color oracle_gs_enable_color
#include "raster-state-entrypoints.inc"
#undef gs_enable_color
#undef gs_get_cull_mode
#undef gs_set_cull_mode
#undef blog
#undef gs_get_context

static struct oracle_raster_event *re_record(int call, const gs_device_t *device)
{
	/* An overflow is a test bug: one call never makes more than one event. */
	if (re_num_events >= RE_MAX_EVENTS)
		abort();
	struct oracle_raster_event *event = &re_events[re_num_events++];
	memset(event, 0, sizeof(*event));
	event->call = call;
	event->device = device;
	return event;
}

static void re_set_cull_mode(gs_device_t *device, enum gs_cull_mode mode)
{
	re_record(0, device)->cull_mode = (int)mode;
}

static enum gs_cull_mode re_get_cull_mode(const gs_device_t *device)
{
	re_record(1, device);
	return (enum gs_cull_mode)re_cull_answer;
}

static void re_enable_color(gs_device_t *device, bool red, bool green, bool blue, bool alpha)
{
	struct oracle_raster_event *event = re_record(2, device);
	event->rgba[0] = red;
	event->rgba[1] = green;
	event->rgba[2] = blue;
	event->rgba[3] = alpha;
}

void oracle_raster_reset(bool context, void *device, int cull_answer)
{
	memset(&re_graphics, 0, sizeof(re_graphics));
	re_graphics.device = (gs_device_t *)device;
	re_graphics.exports.device_set_cull_mode = re_set_cull_mode;
	re_graphics.exports.device_get_cull_mode = re_get_cull_mode;
	re_graphics.exports.device_enable_color = re_enable_color;
	re_has_context = context;
	re_cull_answer = cull_answer;
	re_num_events = 0;
	re_logs = 0;
	re_level = 0;
	re_log[0] = '\0';
}

size_t oracle_raster_events(struct oracle_raster_event *out, size_t max)
{
	size_t n = re_num_events < max ? re_num_events : max;
	memcpy(out, re_events, n * sizeof(*out));
	return n;
}

unsigned oracle_raster_logs(void)
{
	return re_logs;
}

int oracle_raster_level(void)
{
	return re_level;
}

const char *oracle_raster_log(void)
{
	return re_log;
}
