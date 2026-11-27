#include "libobs/build/libobs.pch.h"

/* What obs_source_process_filter_end() and obs_source_process_filter_tech_end()
 * read, write and draw (the obs-source shim), each one step of the original:
 * the filter's bypass flag (read, then cleared), its target and parent, its
 * output flags, linear sRGB, its texrender's texture, and the two draws. The
 * draws are static in obs-source.c.h and reached through its hidden
 * wrappers. Rust decides which run, and in what order. */

void obs_source_process_filter_end_render_bypass(obs_source_t *target, gs_effect_t *effect, const char *tech_name);
void obs_source_process_filter_end_render_tex(gs_texture_t *tex, gs_effect_t *effect, uint32_t width, uint32_t height,
					      const char *tech_name);

bool obs_source_process_filter_end_bypass_active(const obs_source_t *filter);
bool obs_source_process_filter_end_bypass_active(const obs_source_t *filter)
{
	return filter->filter_bypass_active;
}

void obs_source_process_filter_end_clear_bypass(obs_source_t *filter);
void obs_source_process_filter_end_clear_bypass(obs_source_t *filter)
{
	filter->filter_bypass_active = false;
}

obs_source_t *obs_source_process_filter_end_get_target(obs_source_t *filter);
obs_source_t *obs_source_process_filter_end_get_target(obs_source_t *filter)
{
	return obs_filter_get_target(filter);
}

obs_source_t *obs_source_process_filter_end_get_parent(obs_source_t *filter);
obs_source_t *obs_source_process_filter_end_get_parent(obs_source_t *filter)
{
	return obs_filter_get_parent(filter);
}

uint32_t obs_source_process_filter_end_filter_flags(const obs_source_t *filter);
uint32_t obs_source_process_filter_end_filter_flags(const obs_source_t *filter)
{
	return filter->info.output_flags;
}

bool obs_source_process_filter_end_set_linear_srgb(bool linear_srgb);
bool obs_source_process_filter_end_set_linear_srgb(bool linear_srgb)
{
	return gs_set_linear_srgb(linear_srgb);
}

void obs_source_process_filter_end_draw_bypass(obs_source_t *target, gs_effect_t *effect, const char *tech_name);
void obs_source_process_filter_end_draw_bypass(obs_source_t *target, gs_effect_t *effect, const char *tech_name)
{
	obs_source_process_filter_end_render_bypass(target, effect, tech_name);
}

gs_texture_t *obs_source_process_filter_end_texture(const obs_source_t *filter);
gs_texture_t *obs_source_process_filter_end_texture(const obs_source_t *filter)
{
	return gs_texrender_get_texture(filter->filter_texrender);
}

void obs_source_process_filter_end_draw_texture(gs_texture_t *tex, gs_effect_t *effect, uint32_t width, uint32_t height,
						const char *tech_name);
void obs_source_process_filter_end_draw_texture(gs_texture_t *tex, gs_effect_t *effect, uint32_t width, uint32_t height,
						const char *tech_name)
{
	obs_source_process_filter_end_render_tex(tex, effect, width, height, tech_name);
}
