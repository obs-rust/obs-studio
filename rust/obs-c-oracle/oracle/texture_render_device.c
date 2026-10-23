/* Test-only stand-in for the graphics.c functions texture-render.c.h calls.
 *
 * graphics.c stays C and libobs links the real functions. Cargo tests have
 * no graphics context, so this file defines the same public symbols as a
 * recording fake: every call, null destroys included, is appended to one
 * log in order, with its arguments. The Rust shim and the oracle build of
 * texture-render.c.h both call these, so Tier 3 compares their logs.
 *
 * Textures and zstencils are addresses in two static pools. Index 0 of each
 * is the "screen" target the device starts with; created handles count up
 * from 1 after every reset. */

#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>

#include "graphics/graphics.h"

/* printf-checked, so clang's -Wformat-nonliteral accepts the vsnprintf. */
#if defined(__GNUC__) || defined(__clang__)
#define RECORD_FORMAT __attribute__((format(printf, 1, 2)))
#else
#define RECORD_FORMAT
#endif

#define POOL_SIZE 4096

static char textures[POOL_SIZE];
static char zstencils[POOL_SIZE];
static size_t next_texture;
static size_t next_zstencil;
static bool fail_texture;
static bool fail_zstencil;
static gs_texture_t *device_target;
static gs_zstencil_t *device_zs;
static enum gs_color_space device_space;
static char call_log[1 << 16];
static size_t log_len;
static bool quiet;

void oracle_texrender_device_reset(void);
void oracle_texrender_device_fail(bool texture, bool zstencil);
void oracle_texrender_device_set_state(int target, int zstencil, int space);
int oracle_texrender_device_handle(const void *ptr);
const char *oracle_texrender_device_log(void);
void oracle_texrender_device_clear_log(void);
void oracle_texrender_device_quiet(bool on);

RECORD_FORMAT static void record(const char *format, ...)
{
	va_list args;
	int n;

	if (quiet)
		return;
	if (log_len + 1 >= sizeof(call_log))
		return;
	if (log_len)
		call_log[log_len++] = ';';

	va_start(args, format);
	n = vsnprintf(call_log + log_len, sizeof(call_log) - log_len, format, args);
	va_end(args);
	if (n > 0)
		log_len += (size_t)n < sizeof(call_log) - log_len ? (size_t)n : sizeof(call_log) - log_len - 1;
}

/* -1 for null, the pool index for a texture, 10000 + index for a zstencil,
 * -2 for anything else. */
int oracle_texrender_device_handle(const void *ptr)
{
	const char *p = ptr;

	if (!p)
		return -1;
	if (p >= textures && p < textures + POOL_SIZE)
		return (int)(p - textures);
	if (p >= zstencils && p < zstencils + POOL_SIZE)
		return 10000 + (int)(p - zstencils);
	return -2;
}

void oracle_texrender_device_reset(void)
{
	next_texture = 0;
	next_zstencil = 0;
	fail_texture = false;
	fail_zstencil = false;
	quiet = false;
	device_target = (gs_texture_t *)&textures[0];
	device_zs = (gs_zstencil_t *)&zstencils[0];
	device_space = GS_CS_709_EXTENDED;
	oracle_texrender_device_clear_log();
}

void oracle_texrender_device_fail(bool texture, bool zstencil)
{
	fail_texture = texture;
	fail_zstencil = zstencil;
}

/* Sets what the device reports as current: an index into each pool, or a
 * negative value for null. */
void oracle_texrender_device_set_state(int target, int zstencil, int space)
{
	device_target = target < 0 ? NULL : (gs_texture_t *)&textures[target % POOL_SIZE];
	device_zs = zstencil < 0 ? NULL : (gs_zstencil_t *)&zstencils[zstencil % POOL_SIZE];
	device_space = (enum gs_color_space)space;
}

const char *oracle_texrender_device_log(void)
{
	return call_log;
}

/* Stops recording, for port_comparison: timing the log is not the point. */
void oracle_texrender_device_quiet(bool on)
{
	quiet = on;
}

void oracle_texrender_device_clear_log(void)
{
	log_len = 0;
	call_log[0] = 0;
}

gs_texture_t *gs_texture_create(uint32_t width, uint32_t height, enum gs_color_format color_format, uint32_t levels,
				const uint8_t **data, uint32_t flags)
{
	record("tex_create %u %u f%d l%u %s 0x%x", width, height, (int)color_format, levels, data ? "data" : "null",
	       flags);
	if (fail_texture)
		return NULL;
	next_texture = next_texture % (POOL_SIZE - 1) + 1;
	return (gs_texture_t *)&textures[next_texture];
}

gs_zstencil_t *gs_zstencil_create(uint32_t width, uint32_t height, enum gs_zstencil_format format)
{
	record("zs_create %u %u f%d", width, height, (int)format);
	if (fail_zstencil)
		return NULL;
	next_zstencil = next_zstencil % (POOL_SIZE - 1) + 1;
	return (gs_zstencil_t *)&zstencils[next_zstencil];
}

void gs_texture_destroy(gs_texture_t *tex)
{
	record("tex_destroy %d", oracle_texrender_device_handle(tex));
}

void gs_zstencil_destroy(gs_zstencil_t *zstencil)
{
	record("zs_destroy %d", oracle_texrender_device_handle(zstencil));
}

void gs_viewport_push(void)
{
	record("vp_push");
}

void gs_viewport_pop(void)
{
	record("vp_pop");
}

void gs_projection_push(void)
{
	record("proj_push");
}

void gs_projection_pop(void)
{
	record("proj_pop");
}

void gs_matrix_push(void)
{
	record("mat_push");
}

void gs_matrix_pop(void)
{
	record("mat_pop");
}

void gs_matrix_identity(void)
{
	record("mat_identity");
}

gs_texture_t *gs_get_render_target(void)
{
	record("rt_get %d", oracle_texrender_device_handle(device_target));
	return device_target;
}

gs_zstencil_t *gs_get_zstencil_target(void)
{
	record("zs_get %d", oracle_texrender_device_handle(device_zs));
	return device_zs;
}

enum gs_color_space gs_get_color_space(void)
{
	record("cs_get %d", (int)device_space);
	return device_space;
}

void gs_set_render_target_with_color_space(gs_texture_t *tex, gs_zstencil_t *zstencil, enum gs_color_space space)
{
	record("rt_set %d %d cs%d", oracle_texrender_device_handle(tex), oracle_texrender_device_handle(zstencil),
	       (int)space);
	device_target = tex;
	device_zs = zstencil;
	device_space = space;
}

void gs_set_viewport(int x, int y, int width, int height)
{
	record("vp_set %d %d %d %d", x, y, width, height);
}
