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
#include "util/base.h"
#include "util/bmem.h"

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
/* Bit n fails the nth texture create since the last fail_creates call. */
static uint32_t fail_mask;
static unsigned create_index;
static gs_texture_t *device_target;
static gs_zstencil_t *device_zs;
static enum gs_color_space device_space;
static char call_log[1 << 16];
static size_t log_len;
static bool quiet;
static uint32_t texture_cx[POOL_SIZE];
static uint32_t texture_cy[POOL_SIZE];

void oracle_texrender_device_reset(void);
void oracle_texrender_device_fail(bool texture, bool zstencil);
void oracle_texrender_device_set_state(int target, int zstencil, int space);
int oracle_texrender_device_handle(const void *ptr);
const char *oracle_texrender_device_log(void);
void oracle_texrender_device_clear_log(void);
void oracle_texrender_device_quiet(bool on);
void oracle_texrender_device_fail_creates(uint32_t mask);
void oracle_texrender_device_note(const char *text);
void oracle_graphics_set_context_present(bool present);
gs_texture_t *oracle_texrender_texture_create(uint32_t width, uint32_t height, enum gs_color_format color_format,
					      uint32_t levels, const uint8_t **data, uint32_t flags);

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

/* FNV-1a over a byte range, for logging pixel data. */
static uint32_t hash_bytes(const uint8_t *data, size_t size)
{
	uint32_t h = 2166136261u;

	for (size_t i = 0; i < size; i++) {
		h ^= data[i];
		h *= 16777619u;
	}
	return h;
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
	oracle_graphics_set_context_present(true);
	next_texture = 0;
	next_zstencil = 0;
	fail_texture = false;
	fail_zstencil = false;
	fail_mask = 0;
	create_index = 0;
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

/* Makes the texture creates fail whose bit is set: bit 0 the next one, bit 1
 * the one after, and so on. */
void oracle_texrender_device_fail_creates(uint32_t mask)
{
	fail_mask = mask;
	create_index = 0;
}

/* Appends text to the call log as one more call (the planar creators' device
 * exports, oracle/texture_planar_creators.c). */
void oracle_texrender_device_note(const char *text)
{
	record("%s", text);
}

void oracle_texrender_device_clear_log(void)
{
	log_len = 0;
	call_log[0] = 0;
}

gs_texture_t *oracle_texrender_texture_create(uint32_t width, uint32_t height, enum gs_color_format color_format,
					      uint32_t levels, const uint8_t **data, uint32_t flags)
{
	/* image-file passes its pixels: record a hash of the RGBA bytes. */
	if (data && *data)
		record("tex_create %u %u f%d l%u data:%08x 0x%x", width, height, (int)color_format, levels,
		       hash_bytes(*data, (size_t)width * height * 4), flags);
	else
		record("tex_create %u %u f%d l%u %s 0x%x", width, height, (int)color_format, levels,
		       data ? "data" : "null", flags);
	create_index++;
	if (fail_texture || (create_index <= 32 && (fail_mask >> (create_index - 1)) & 1))
		return NULL;
	next_texture = next_texture % (POOL_SIZE - 1) + 1;
	texture_cx[next_texture] = width;
	texture_cy[next_texture] = height;
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

/* gs_matrix_push(), gs_matrix_pop() and gs_matrix_identity() are the Rust
 * matrix calls now: without a context they reach this log as "mat_push",
 * "mat_pop" and "mat_identity" through oracle/matrix_entrypoints.c. */

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

/* image-file.c.h's other graphics calls (obs-rust/obs-studio#1907). */

void gs_texture_set_image(gs_texture_t *tex, const uint8_t *data, uint32_t linesize, bool flip)
{
	int handle = oracle_texrender_device_handle(tex);

	if (!data || handle < 0 || handle >= POOL_SIZE) {
		record("set_image %d null %u %d", handle, linesize, (int)flip);
		return;
	}
	record("set_image %d data:%08x %u %d", handle, hash_bytes(data, (size_t)linesize * texture_cy[handle]),
	       linesize, (int)flip);
}

/* graphics-ffmpeg stays C and needs FFmpeg. The stand-in decodes nothing:
 * a file that opens is a 3x2 RGBA picture whose bytes depend on the alpha
 * mode, in a bmalloc block as FFmpeg's is. The outputs are written only on
 * success, as the original does. */
uint8_t *gs_create_texture_file_data3(const char *file, enum gs_image_alpha_mode alpha_mode,
				      enum gs_color_format *format, uint32_t *cx_out, uint32_t *cy_out,
				      enum gs_color_space *space)
{
	FILE *f = fopen(file, "rb");
	uint8_t *data;

	record("file_data a%d %s", (int)alpha_mode, f ? "ok" : "fail");
	if (!f)
		return NULL;
	fclose(f);

	data = bmalloc(3 * 2 * 4);
	for (size_t i = 0; i < 3 * 2 * 4; i++)
		data[i] = (uint8_t)(i * 7 + (size_t)alpha_mode);
	*format = GS_RGBA;
	*cx_out = 3;
	*cy_out = 2;
	*space = GS_CS_SRGB;
	return data;
}

/* Log lines, in the call log, so a parity test compares them in order. */
#if defined(__clang__) || defined(__GNUC__)
#pragma GCC diagnostic push
#pragma GCC diagnostic ignored "-Wformat-nonliteral"
#endif
static void capture_log(int level, const char *format, va_list args, void *param)
{
	char line[1024];

	(void)param;
	vsnprintf(line, sizeof(line), format, args);
	record("log %d %s", level, line);
}
#if defined(__clang__) || defined(__GNUC__)
#pragma GCC diagnostic pop
#endif

void oracle_texrender_device_capture_log(bool on);

void oracle_texrender_device_capture_log(bool on)
{
	base_set_log_handler(on ? capture_log : NULL, NULL);
}
