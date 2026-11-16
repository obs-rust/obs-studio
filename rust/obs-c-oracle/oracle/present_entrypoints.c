/* Tier 3 for obs-graphics::present_entrypoints: the original frame and present
 * calls (present-entrypoints.c.h, renamed oracle_gs_*) and the production
 * adapter (present-entrypoints-fields.c.h, under its real names, which the
 * Rust shims call) over the real private graphics layout, a thread-local
 * stand-in context, a recording device and a recording blog. */
#include <stdarg.h>
#include <stddef.h>
#include <stdio.h>
#include <string.h>
#include "graphics/graphics-internal.h"
#include "util/base.h"

void oracle_gs_begin_frame(void);
void oracle_gs_begin_scene(void);
void oracle_gs_draw(enum gs_draw_mode draw_mode, uint32_t start_vert, uint32_t num_verts);
void oracle_gs_end_scene(void);
void oracle_gs_load_swapchain(gs_swapchain_t *swapchain);
void oracle_gs_clear(uint32_t clear_flags, const struct vec4 *color, float depth, uint8_t stencil);
bool oracle_gs_is_present_ready(void);
void oracle_gs_present(void);
void oracle_gs_flush(void);

/* One device call; `call` is 0..8 in the order of the C file. */
struct oracle_present_event {
	int call;
	const void *device;
	int draw_mode;
	uint32_t start_vert;
	uint32_t num_verts;
	const void *pointer;
	uint32_t clear_flags;
	uint32_t depth_bits;
	uint8_t stencil;
};

#define PE_MAX_EVENTS 8

void oracle_present_reset(bool context, void *device, bool present_ready);
size_t oracle_present_events(struct oracle_present_event *out, size_t max);
unsigned oracle_present_logs(void);
int oracle_present_level(void);
const char *oracle_present_log(void);

static THREAD_LOCAL struct graphics_subsystem pe_graphics;
static THREAD_LOCAL bool pe_has_context;
static THREAD_LOCAL bool pe_present_ready;
static THREAD_LOCAL struct oracle_present_event pe_events[PE_MAX_EVENTS];
static THREAD_LOCAL size_t pe_num_events;
static THREAD_LOCAL unsigned pe_logs;
static THREAD_LOCAL int pe_level;
static THREAD_LOCAL char pe_log[160];

static graphics_t *pe_context(void)
{
	return pe_has_context ? &pe_graphics : NULL;
}

#ifdef __GNUC__
__attribute__((format(printf, 2, 3)))
#endif
static void pe_blog(int level, const char *format, ...)
{
	va_list args;
	va_start(args, format);
	pe_logs++;
	pe_level = level;
	vsnprintf(pe_log, sizeof(pe_log), format, args);
	va_end(args);
}

#define gs_get_context pe_context
#define blog pe_blog
#include "present-entrypoints-fields.inc"
#define gs_begin_frame oracle_gs_begin_frame
#define gs_begin_scene oracle_gs_begin_scene
#define gs_draw oracle_gs_draw
#define gs_end_scene oracle_gs_end_scene
#define gs_load_swapchain oracle_gs_load_swapchain
#define gs_clear oracle_gs_clear
#define gs_is_present_ready oracle_gs_is_present_ready
#define gs_present oracle_gs_present
#define gs_flush oracle_gs_flush
#include "present-entrypoints.inc"
#undef gs_flush
#undef gs_present
#undef gs_is_present_ready
#undef gs_clear
#undef gs_load_swapchain
#undef gs_end_scene
#undef gs_draw
#undef gs_begin_scene
#undef gs_begin_frame
#undef blog
#undef gs_get_context

static struct oracle_present_event *pe_record(int call, gs_device_t *device)
{
	/* An overflow is a test bug: one call never makes more than one event. */
	if (pe_num_events >= PE_MAX_EVENTS)
		abort();
	struct oracle_present_event *event = &pe_events[pe_num_events++];
	memset(event, 0, sizeof(*event));
	event->call = call;
	event->device = device;
	return event;
}

static void pe_begin_frame(gs_device_t *device)
{
	pe_record(0, device);
}

static void pe_begin_scene(gs_device_t *device)
{
	pe_record(1, device);
}

static void pe_draw(gs_device_t *device, enum gs_draw_mode draw_mode, uint32_t start_vert, uint32_t num_verts)
{
	struct oracle_present_event *event = pe_record(2, device);
	event->draw_mode = (int)draw_mode;
	event->start_vert = start_vert;
	event->num_verts = num_verts;
}

static void pe_end_scene(gs_device_t *device)
{
	pe_record(3, device);
}

static void pe_load_swapchain(gs_device_t *device, gs_swapchain_t *swapchain)
{
	pe_record(4, device)->pointer = swapchain;
}

static void pe_clear(gs_device_t *device, uint32_t clear_flags, const struct vec4 *color, float depth, uint8_t stencil)
{
	struct oracle_present_event *event = pe_record(5, device);
	event->clear_flags = clear_flags;
	event->pointer = color;
	memcpy(&event->depth_bits, &depth, sizeof(event->depth_bits));
	event->stencil = stencil;
}

static bool pe_is_present_ready(gs_device_t *device)
{
	pe_record(6, device);
	return pe_present_ready;
}

static void pe_present(gs_device_t *device)
{
	pe_record(7, device);
}

static void pe_flush(gs_device_t *device)
{
	pe_record(8, device);
}

void oracle_present_reset(bool context, void *device, bool present_ready)
{
	memset(&pe_graphics, 0, sizeof(pe_graphics));
	pe_graphics.device = (gs_device_t *)device;
	pe_graphics.exports.device_begin_frame = pe_begin_frame;
	pe_graphics.exports.device_begin_scene = pe_begin_scene;
	pe_graphics.exports.device_draw = pe_draw;
	pe_graphics.exports.device_end_scene = pe_end_scene;
	pe_graphics.exports.device_load_swapchain = pe_load_swapchain;
	pe_graphics.exports.device_clear = pe_clear;
	pe_graphics.exports.device_is_present_ready = pe_is_present_ready;
	pe_graphics.exports.device_present = pe_present;
	pe_graphics.exports.device_flush = pe_flush;
	pe_has_context = context;
	pe_present_ready = present_ready;
	pe_num_events = 0;
	pe_logs = 0;
	pe_level = 0;
	pe_log[0] = '\0';
}

size_t oracle_present_events(struct oracle_present_event *out, size_t max)
{
	size_t n = pe_num_events < max ? pe_num_events : max;
	memcpy(out, pe_events, n * sizeof(*out));
	return n;
}

unsigned oracle_present_logs(void)
{
	return pe_logs;
}

int oracle_present_level(void)
{
	return pe_level;
}

const char *oracle_present_log(void)
{
	return pe_log;
}
