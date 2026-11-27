#include "libobs/build/libobs.pch.h"

/* What obs_source_process_filter_begin() and
 * obs_source_process_filter_begin_with_color_space() read, write and draw
 * (the obs-source shim), each one step of the original: the filter's name
 * for the log lines, its bypass flag, allow_direct and texrender, its target
 * and parent, their output flags, the target's base size, the current color
 * space, the skip, and the draw into the texrender. Rust decides which run,
 * and in what order. */

void obs_source_process_filter_begin_clear_bypass(obs_source_t *filter);
void obs_source_process_filter_begin_clear_bypass(obs_source_t *filter)
{
	filter->filter_bypass_active = false;
}

obs_source_t *obs_source_process_filter_begin_get_target(obs_source_t *filter);
obs_source_t *obs_source_process_filter_begin_get_target(obs_source_t *filter)
{
	return obs_filter_get_target(filter);
}

obs_source_t *obs_source_process_filter_begin_get_parent(obs_source_t *filter);
obs_source_t *obs_source_process_filter_begin_get_parent(obs_source_t *filter)
{
	return obs_filter_get_parent(filter);
}

const char *obs_source_process_filter_begin_name(const obs_source_t *filter);
const char *obs_source_process_filter_begin_name(const obs_source_t *filter)
{
	return filter->context.name;
}

uint32_t obs_source_process_filter_begin_flags(const obs_source_t *source);
uint32_t obs_source_process_filter_begin_flags(const obs_source_t *source)
{
	return source->info.output_flags;
}

uint32_t obs_source_process_filter_begin_base_width(obs_source_t *target);
uint32_t obs_source_process_filter_begin_base_width(obs_source_t *target)
{
	return obs_source_base_width(target);
}

uint32_t obs_source_process_filter_begin_base_height(obs_source_t *target);
uint32_t obs_source_process_filter_begin_base_height(obs_source_t *target)
{
	return obs_source_base_height(target);
}

void obs_source_process_filter_begin_set_allow_direct(obs_source_t *filter, int allow_direct);
void obs_source_process_filter_begin_set_allow_direct(obs_source_t *filter, int allow_direct)
{
	filter->allow_direct = (enum obs_allow_direct_render)allow_direct;
}

int obs_source_process_filter_begin_color_space(void);
int obs_source_process_filter_begin_color_space(void)
{
	return (int)gs_get_color_space();
}

void obs_source_process_filter_begin_set_bypass(obs_source_t *filter);
void obs_source_process_filter_begin_set_bypass(obs_source_t *filter)
{
	filter->filter_bypass_active = true;
}

void obs_source_process_filter_begin_skip(obs_source_t *filter);
void obs_source_process_filter_begin_skip(obs_source_t *filter)
{
	obs_source_skip_video_filter(filter);
}

bool obs_source_process_filter_begin_has_texrender(const obs_source_t *filter);
bool obs_source_process_filter_begin_has_texrender(const obs_source_t *filter)
{
	return filter->filter_texrender != NULL;
}

int obs_source_process_filter_begin_texrender_format(const obs_source_t *filter);
int obs_source_process_filter_begin_texrender_format(const obs_source_t *filter)
{
	return (int)gs_texrender_get_format(filter->filter_texrender);
}

void obs_source_process_filter_begin_destroy_texrender(obs_source_t *filter);
void obs_source_process_filter_begin_destroy_texrender(obs_source_t *filter)
{
	gs_texrender_destroy(filter->filter_texrender);
	filter->filter_texrender = NULL;
}

void obs_source_process_filter_begin_create_texrender(obs_source_t *filter, int format);
void obs_source_process_filter_begin_create_texrender(obs_source_t *filter, int format)
{
	filter->filter_texrender = gs_texrender_create((enum gs_color_format)format, GS_ZS_NONE);
}

bool obs_source_process_filter_begin_texrender_begin(obs_source_t *filter, uint32_t cx, uint32_t cy, int space);
bool obs_source_process_filter_begin_texrender_begin(obs_source_t *filter, uint32_t cx, uint32_t cy, int space)
{
	return gs_texrender_begin_with_color_space(filter->filter_texrender, cx, cy, (enum gs_color_space)space);
}

void obs_source_process_filter_begin_draw_setup(int cx, int cy);
void obs_source_process_filter_begin_draw_setup(int cx, int cy)
{
	gs_blend_state_push();
	gs_blend_function_separate(GS_BLEND_SRCALPHA, GS_BLEND_INVSRCALPHA, GS_BLEND_ONE, GS_BLEND_INVSRCALPHA);

	struct vec4 clear_color;

	vec4_zero(&clear_color);
	gs_clear(GS_CLEAR_COLOR, &clear_color, 0.0f, 0);
	gs_ortho(0.0f, (float)cx, 0.0f, (float)cy, -100.0f, 100.0f);
}

void obs_source_process_filter_begin_default_render(obs_source_t *target);
void obs_source_process_filter_begin_default_render(obs_source_t *target)
{
	obs_source_default_render(target);
}

void obs_source_process_filter_begin_video_render(obs_source_t *target);
void obs_source_process_filter_begin_video_render(obs_source_t *target)
{
	obs_source_video_render(target);
}

void obs_source_process_filter_begin_draw_finish(obs_source_t *filter);
void obs_source_process_filter_begin_draw_finish(obs_source_t *filter)
{
	gs_blend_state_pop();

	gs_texrender_end(filter->filter_texrender);
}
