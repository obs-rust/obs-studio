/* Test-only stand-in for the graphics functions effect-passes.c.h calls:
 * gs_load_vertexshader, gs_load_pixelshader, gs_shader_set_val,
 * gs_shader_set_next_sampler, gs_shader_get_param_info,
 * gs_shader_set_texture and gs_get_effect (the Rust shim's
 * obs_graphics_set_cur_effect is the adapter, which oracle/image_file.c
 * builds into the test binary). Each
 * call is recorded as a line, so the C oracle and the shim can be compared
 * call by call. Shaders, shader parameters, samplers and textures are tags
 * (small integers as pointers); a shader parameter tag of 100 or more is a
 * texture parameter. */

#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>

#include "graphics/graphics.h"
#include "graphics/graphics-internal.h"
#include "util/bmem.h"

#if defined(__GNUC__) || defined(__clang__)
#define RECORD_FORMAT __attribute__((format(printf, 1, 2)))
#else
#define RECORD_FORMAT
#endif

#define PASSES_MAX_EVENTS 256
static char passes_events[PASSES_MAX_EVENTS][96];
static size_t passes_num_events;
static graphics_t *passes_graphics;
static bool passes_recording = true;

static void passes_record(const char *fmt, ...) RECORD_FORMAT;
static void passes_record(const char *fmt, ...)
{
	if (!passes_recording || passes_num_events >= PASSES_MAX_EVENTS)
		return;
	va_list args;
	va_start(args, fmt);
	vsnprintf(passes_events[passes_num_events++], sizeof(passes_events[0]), fmt, args);
	va_end(args);
}

#define PASSES_TAG(p) ((unsigned)(uintptr_t)(p))

void gs_load_vertexshader(gs_shader_t *vertshader)
{
	passes_record("load_vs %u", PASSES_TAG(vertshader));
}

void gs_load_pixelshader(gs_shader_t *pixelshader)
{
	passes_record("load_ps %u", PASSES_TAG(pixelshader));
}

void gs_shader_set_val(gs_sparam_t *param, const void *val, size_t size)
{
	if (!passes_recording)
		return;
	char hex[2 * 32 + 1] = {0};
	for (size_t i = 0; i < size && i < 32; i++)
		snprintf(hex + i * 2, 3, "%02x", ((const uint8_t *)val)[i]);
	passes_record("set_val %u %zu %s", PASSES_TAG(param), size, hex);
}

void gs_shader_set_next_sampler(gs_sparam_t *param, gs_samplerstate_t *sampler)
{
	passes_record("next_sampler %u %u", PASSES_TAG(param), PASSES_TAG(sampler));
}

void gs_shader_get_param_info(const gs_sparam_t *param, struct gs_shader_param_info *info)
{
	info->name = "p";
	info->type = PASSES_TAG(param) >= 100 ? GS_SHADER_PARAM_TEXTURE : GS_SHADER_PARAM_FLOAT;
	passes_record("param_info %u", PASSES_TAG(param));
}

void gs_shader_set_texture(gs_sparam_t *param, gs_texture_t *val)
{
	passes_record("set_texture %u %u", PASSES_TAG(param), PASSES_TAG(val));
}

gs_effect_t *gs_get_effect(void)
{
	return passes_graphics ? passes_graphics->cur_effect : NULL;
}

/* The layout of struct gs_shader_param_info, for the shim's mirror. */
size_t oracle_passes_shader_param_info_layout(size_t which);
size_t oracle_passes_shader_param_info_layout(size_t which)
{
	const size_t values[4] = {
		sizeof(struct gs_shader_param_info),
		_Alignof(struct gs_shader_param_info),
		offsetof(struct gs_shader_param_info, type),
		offsetof(struct gs_shader_param_info, name),
	};
	return which < 4 ? values[which] : 0;
}

/* The test's side of the stand-in. */
graphics_t *oracle_passes_graphics_new(void);
void oracle_passes_graphics_free(graphics_t *graphics);
void oracle_passes_set_graphics(graphics_t *graphics);
void *oracle_passes_cur_effect(graphics_t *graphics);
void oracle_passes_set_cur_effect(graphics_t *graphics, void *effect);
void oracle_passes_reset(void);
void oracle_passes_set_recording(bool recording);
size_t oracle_passes_count(void);
const char *oracle_passes_event(size_t i);

graphics_t *oracle_passes_graphics_new(void)
{
	return bzalloc(sizeof(struct graphics_subsystem));
}

void oracle_passes_graphics_free(graphics_t *graphics)
{
	bfree(graphics);
}

void oracle_passes_set_graphics(graphics_t *graphics)
{
	passes_graphics = graphics;
}

void *oracle_passes_cur_effect(graphics_t *graphics)
{
	return graphics->cur_effect;
}

void oracle_passes_set_cur_effect(graphics_t *graphics, void *effect)
{
	graphics->cur_effect = effect;
}

void oracle_passes_set_recording(bool recording)
{
	passes_recording = recording;
}

void oracle_passes_reset(void)
{
	passes_num_events = 0;
}

size_t oracle_passes_count(void)
{
	return passes_num_events;
}

const char *oracle_passes_event(size_t i)
{
	return i < passes_num_events ? passes_events[i] : "";
}
