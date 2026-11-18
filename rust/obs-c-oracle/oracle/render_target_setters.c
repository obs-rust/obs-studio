/* Original C and real private adapters over one thread-local recording backend. */
#include <stdarg.h>
#include <stddef.h>
#include <stdio.h>
#include <string.h>
#include "graphics/graphics-internal.h"
#include "util/base.h"

#define gs_set_render_target oracle_gs_set_render_target
#define gs_set_cube_render_target oracle_gs_set_cube_render_target
void oracle_gs_set_render_target(gs_texture_t *texture, gs_zstencil_t *depth);
void oracle_gs_set_cube_render_target(gs_texture_t *texture, int side, gs_zstencil_t *depth);

static THREAD_LOCAL struct graphics_subsystem rts_graphics;
static THREAD_LOCAL unsigned rts_calls, rts_logs;
static THREAD_LOCAL int rts_kind, rts_level, rts_side;
static THREAD_LOCAL const void *rts_device, *rts_texture, *rts_depth;
static THREAD_LOCAL char rts_log[160];

#ifdef __GNUC__
__attribute__((format(printf, 2, 3)))
#endif
static void rts_blog(int level, const char *format, ...)
{
	va_list args;
	va_start(args, format);
	rts_logs++;
	rts_level = level;
	vsnprintf(rts_log, sizeof(rts_log), format, args);
	va_end(args);
}

#define blog rts_blog
#include "graphics-render-target-setters.inc"

/* Original graphics.c.h validator, verbatim. */
static inline bool gs_valid(const char *f)
{
	if (!thread_graphics) {
		blog(LOG_DEBUG, "%s: called while not in a graphics context", f);
		return false;
	}

	return true;
}

static graphics_t *rts_context(void)
{
	return thread_graphics;
}
#define gs_get_context rts_context
#include "graphics-render-target-setters-fields.inc"
#undef gs_get_context
#undef blog

static void rts_record(int kind, gs_device_t *device, gs_texture_t *texture, gs_zstencil_t *depth, int side)
{
	rts_calls++;
	rts_kind = kind;
	rts_device = device;
	rts_texture = texture;
	rts_depth = depth;
	rts_side = side;
}

static void rts_target(gs_device_t *device, gs_texture_t *texture, gs_zstencil_t *depth)
{
	rts_record(0, device, texture, depth, 0);
}

static void rts_cube(gs_device_t *device, gs_texture_t *texture, int side, gs_zstencil_t *depth)
{
	rts_record(1, device, texture, depth, side);
}

void oracle_render_target_setters_reset(bool context, void *device);
void oracle_render_target_setters_reset(bool context, void *device)
{
	memset(&rts_graphics, 0, sizeof(rts_graphics));
	rts_graphics.device = device;
	rts_graphics.exports.device_set_render_target = rts_target;
	rts_graphics.exports.device_set_cube_render_target = rts_cube;
	thread_graphics = context ? &rts_graphics : NULL;
	rts_calls = rts_logs = 0;
	rts_kind = -1;
	rts_level = rts_side = 0;
	rts_device = rts_texture = rts_depth = NULL;
	rts_log[0] = '\0';
}

unsigned oracle_render_target_setters_calls(void);
unsigned oracle_render_target_setters_calls(void)
{
	return rts_calls;
}

unsigned oracle_render_target_setters_logs(void);
unsigned oracle_render_target_setters_logs(void)
{
	return rts_logs;
}

int oracle_render_target_setters_kind(void);
int oracle_render_target_setters_kind(void)
{
	return rts_kind;
}

int oracle_render_target_setters_level(void);
int oracle_render_target_setters_level(void)
{
	return rts_level;
}

int oracle_render_target_setters_side(void);
int oracle_render_target_setters_side(void)
{
	return rts_side;
}

const void *oracle_render_target_setters_device(void);
const void *oracle_render_target_setters_device(void)
{
	return rts_device;
}

const void *oracle_render_target_setters_texture(void);
const void *oracle_render_target_setters_texture(void)
{
	return rts_texture;
}

const void *oracle_render_target_setters_depth(void);
const void *oracle_render_target_setters_depth(void)
{
	return rts_depth;
}

const char *oracle_render_target_setters_log(void);
const char *oracle_render_target_setters_log(void)
{
	return rts_log;
}

#undef gs_set_render_target
#undef gs_set_cube_render_target
