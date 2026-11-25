/* Tier 3: the unchanged extracted obs_source_draw_set_color_matrix() and
 * obs_source_draw() (libobs/obs-source-draw.c.h) against the Rust exports'
 * bridges, over one recorder for every graphics call they make.
 *
 * The recorder logs each call with its arguments (pointers as fixture ids,
 * floats as bits, the bytes a set copies as hex) and answers from the run's
 * state: the current effect (or NULL), NULL params when asked, linear sRGB
 * and the framebuffer sRGB flag. The two log lines (the "no active effect"
 * warnings and obs_ptr_valid()'s) go through the real blog(); a run that
 * passes a log handler records them in the same log, in order. The real
 * device and its pixels stay unchanged Tier 2 (test_source_draw). */
#define _NO_CRT_STDIO_INLINE
#include <stdarg.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "libobs/build/libobs.pch.h"

#define SD_LOG 4096
#define SD_TOKENS 8
#define SD_EFFECT 0
#define SD_TEXTURE 1
#define SD_PARAMS 2 /* 2..5: color_matrix, color_range_min, color_range_max, image */

static unsigned char sd_tokens[SD_TOKENS];
static char sd_log[SD_LOG];
static size_t sd_len;
static bool sd_effect;
static bool sd_null_params;
static bool sd_linear;
static bool sd_srgb;

#ifdef __GNUC__
__attribute__((format(printf, 1, 2)))
#endif
static void sd_record(const char *format, ...)
{
	va_list args;
	va_start(args, format);
	int n = vsnprintf(sd_log + sd_len, SD_LOG - sd_len, format, args);
	va_end(args);
	if (n < 0 || (size_t)n >= SD_LOG - sd_len)
		abort();
	sd_len += (size_t)n;
}

/* A token's index, -1 for NULL, 99 for anything else. */
static int sd_id(const void *ptr)
{
	if (!ptr)
		return -1;
	for (int i = 0; i < SD_TOKENS; ++i)
		if (ptr == (const void *)&sd_tokens[i])
			return i;
	return 99;
}

static unsigned sd_bits(float value)
{
	uint32_t bits;
	memcpy(&bits, &value, sizeof(bits));
	return (unsigned)bits;
}

static void sd_bytes(const void *data, size_t size)
{
	const unsigned char *bytes = data;
	for (size_t i = 0; i < size; ++i)
		sd_record("%02x", bytes[i]);
}

#if defined(__GNUC__) || defined(__clang__)
static void sd_log_handler(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void sd_log_handler(int level, const char *format, va_list args, void *param)
{
	(void)param;
	char text[256];
	vsnprintf(text, sizeof(text), format, args);
	sd_record("log %d %s;", level, text);
}

gs_effect_t *oracle_sd_get_effect(void);
gs_effect_t *oracle_sd_get_effect(void)
{
	sd_record("get_effect;");
	return sd_effect ? (gs_effect_t *)&sd_tokens[SD_EFFECT] : NULL;
}

gs_eparam_t *oracle_sd_param(const gs_effect_t *effect, const char *name);
gs_eparam_t *oracle_sd_param(const gs_effect_t *effect, const char *name)
{
	sd_record("param %d %s;", sd_id(effect), name);
	if (sd_null_params)
		return NULL;
	static const char *const names[] = {"color_matrix", "color_range_min", "color_range_max", "image"};
	for (int i = 0; i < 4; ++i)
		if (strcmp(name, names[i]) == 0)
			return (gs_eparam_t *)&sd_tokens[SD_PARAMS + i];
	return (gs_eparam_t *)&sd_tokens[SD_TOKENS - 1];
}

void oracle_sd_set_matrix4(gs_eparam_t *param, const struct matrix4 *val);
void oracle_sd_set_matrix4(gs_eparam_t *param, const struct matrix4 *val)
{
	sd_record("set_matrix4 %d ", sd_id(param));
	sd_bytes(val, sizeof(struct matrix4));
	sd_record(";");
}

void oracle_sd_set_val(gs_eparam_t *param, const void *val, size_t size);
void oracle_sd_set_val(gs_eparam_t *param, const void *val, size_t size)
{
	sd_record("set_val %d %u ", sd_id(param), (unsigned)size);
	sd_bytes(val, size);
	sd_record(";");
}

bool oracle_sd_linear_srgb(void);
bool oracle_sd_linear_srgb(void)
{
	sd_record("linear_srgb;");
	return sd_linear;
}

bool oracle_sd_srgb_enabled(void);
bool oracle_sd_srgb_enabled(void)
{
	sd_record("srgb_enabled;");
	return sd_srgb;
}

void oracle_sd_enable_srgb(bool enable);
void oracle_sd_enable_srgb(bool enable)
{
	sd_record("enable_srgb %d;", enable ? 1 : 0);
	sd_srgb = enable;
}

void oracle_sd_set_texture_srgb(gs_eparam_t *param, gs_texture_t *texture);
void oracle_sd_set_texture_srgb(gs_eparam_t *param, gs_texture_t *texture)
{
	sd_record("set_texture_srgb %d %d;", sd_id(param), sd_id(texture));
}

void oracle_sd_set_texture(gs_eparam_t *param, gs_texture_t *texture);
void oracle_sd_set_texture(gs_eparam_t *param, gs_texture_t *texture)
{
	sd_record("set_texture %d %d;", sd_id(param), sd_id(texture));
}

void oracle_sd_matrix_push(void);
void oracle_sd_matrix_push(void)
{
	sd_record("push;");
}

void oracle_sd_matrix_translate3f(float x, float y, float z);
void oracle_sd_matrix_translate3f(float x, float y, float z)
{
	sd_record("translate %08x %08x %08x;", sd_bits(x), sd_bits(y), sd_bits(z));
}

void oracle_sd_draw_sprite(gs_texture_t *texture, uint32_t flip, uint32_t width, uint32_t height);
void oracle_sd_draw_sprite(gs_texture_t *texture, uint32_t flip, uint32_t width, uint32_t height)
{
	sd_record("draw_sprite %d %u %u %u;", sd_id(texture), (unsigned)flip, (unsigned)width, (unsigned)height);
}

void oracle_sd_matrix_pop(void);
void oracle_sd_matrix_pop(void)
{
	sd_record("pop;");
}

void oracle_obs_source_draw_set_color_matrix(const struct matrix4 *color_matrix, const struct vec3 *color_range_min,
					     const struct vec3 *color_range_max);
void oracle_obs_source_draw(gs_texture_t *texture, int x, int y, uint32_t cx, uint32_t cy, bool flip);

#define obs_source_draw_set_color_matrix oracle_obs_source_draw_set_color_matrix
#define obs_source_draw oracle_obs_source_draw
#define gs_get_effect oracle_sd_get_effect
#define gs_effect_get_param_by_name oracle_sd_param
#define gs_effect_set_matrix4 oracle_sd_set_matrix4
#define gs_effect_set_val oracle_sd_set_val
#define gs_get_linear_srgb oracle_sd_linear_srgb
#define gs_framebuffer_srgb_enabled oracle_sd_srgb_enabled
#define gs_enable_framebuffer_srgb oracle_sd_enable_srgb
#define gs_effect_set_texture_srgb oracle_sd_set_texture_srgb
#define gs_effect_set_texture oracle_sd_set_texture
#define gs_matrix_push oracle_sd_matrix_push
#define gs_matrix_translate3f oracle_sd_matrix_translate3f
#define gs_draw_sprite oracle_sd_draw_sprite
#define gs_matrix_pop oracle_sd_matrix_pop
#include "source-draw.inc"
#undef gs_matrix_pop
#undef gs_draw_sprite
#undef gs_matrix_translate3f
#undef gs_matrix_push
#undef gs_effect_set_texture
#undef gs_effect_set_texture_srgb
#undef gs_enable_framebuffer_srgb
#undef gs_framebuffer_srgb_enabled
#undef gs_get_linear_srgb
#undef gs_effect_set_val
#undef gs_effect_set_matrix4
#undef gs_effect_get_param_by_name
#undef gs_get_effect
#undef obs_source_draw
#undef obs_source_draw_set_color_matrix

/* The fixture's texture token, for both sides to pass. */
void *oracle_source_draw_texture(void);
void *oracle_source_draw_texture(void)
{
	return &sd_tokens[SD_TEXTURE];
}

typedef void (*sd_matrix_fn)(const struct matrix4 *color_matrix, const struct vec3 *color_range_min,
			     const struct vec3 *color_range_max);
typedef void (*sd_draw_fn)(gs_texture_t *texture, int x, int y, uint32_t cx, uint32_t cy, bool flip);
typedef void (*sd_get_log_fn)(log_handler_t *handler, void **param);
typedef void (*sd_set_log_fn)(log_handler_t handler, void *param);

/* One call. state (all 0 or 1, checked):
 *   [0] entry: 0 set_color_matrix, 1 draw
 *   [1] a current effect; [2] NULL params; [3] linear sRGB;
 *   [4] the framebuffer sRGB flag before;
 *   set_color_matrix: [5] NULL matrix, [6] NULL min, [7] NULL max;
 *   draw: [5] NULL texture, [6] flip.
 * values: the matrix (16 floats), then min and max (4 floats each, the
 * fourth past the 12 bytes the C copies). x, y, cx, cy: the draw's.
 * get_log/set_log: NULL, or base_get_log_handler / base_set_log_handler to
 * record the log lines (the caller owns the process's handler).
 * Writes the log (NUL-terminated); returns the sRGB flag after. */
bool oracle_source_draw_run(sd_matrix_fn set_color_matrix, sd_draw_fn draw, const int *state, const float *values,
			    int x, int y, uint32_t cx, uint32_t cy, sd_get_log_fn get_log, sd_set_log_fn set_log,
			    char *log, size_t log_len);
bool oracle_source_draw_run(sd_matrix_fn set_color_matrix, sd_draw_fn draw, const int *state, const float *values,
			    int x, int y, uint32_t cx, uint32_t cy, sd_get_log_fn get_log, sd_set_log_fn set_log,
			    char *log, size_t log_len)
{
	for (int i = 0; i < 8; ++i)
		if (state[i] < 0 || state[i] > 1)
			abort();
	if (log_len < SD_LOG || (get_log == NULL) != (set_log == NULL))
		abort();

	struct matrix4 matrix;
	struct vec3 range_min, range_max;
	memcpy(&matrix, values, sizeof(matrix));
	memcpy(&range_min, values + 16, sizeof(float) * 4);
	memcpy(&range_max, values + 20, sizeof(float) * 4);

	sd_effect = state[1] != 0;
	sd_null_params = state[2] != 0;
	sd_linear = state[3] != 0;
	sd_srgb = state[4] != 0;
	sd_len = 0;
	sd_log[0] = 0;

	log_handler_t old = NULL;
	void *old_param = NULL;
	if (get_log) {
		get_log(&old, &old_param);
		set_log(sd_log_handler, NULL);
	}

	if (state[0] == 0) {
		set_color_matrix(state[5] ? NULL : &matrix, state[6] ? NULL : &range_min, state[7] ? NULL : &range_max);
	} else {
		draw(state[5] ? NULL : (gs_texture_t *)&sd_tokens[SD_TEXTURE], x, y, cx, cy, state[6] != 0);
	}

	if (set_log)
		set_log(old, old_param);

	memcpy(log, sd_log, sd_len + 1);
	return sd_srgb;
}

/* The values the port relies on. */
void oracle_source_draw_values(int *out);
void oracle_source_draw_values(int *out)
{
	out[0] = GS_FLIP_V;
	out[1] = LOG_WARNING;
	out[2] = LOG_DEBUG;
	out[3] = (int)sizeof(struct matrix4);
	out[4] = (int)_Alignof(struct matrix4);
	out[5] = (int)sizeof(struct vec3);
	out[6] = (int)_Alignof(struct vec3);
}
