#include <obs-module.h>
#include <graphics/vec2.h>
#include <graphics/vec4.h>
#include <math.h>

/* The largest texture Direct3D 11 and Metal can create, and that most OpenGL
 * drivers allow. A source wider or taller than this cannot be drawn into one
 * texture, so only the visible window of it is drawn (render_window). */
#define MAX_TEXTURE_SIZE 16384

struct scroll_filter_data {
	obs_source_t *context;

	gs_effect_t *effect;
	gs_eparam_t *param_add;
	gs_eparam_t *param_mul;
	gs_eparam_t *param_image;
	gs_eparam_t *param_multiplier;

	struct vec2 scroll_speed;
	gs_samplerstate_t *sampler;
	bool limit_cx;
	bool limit_cy;
	uint32_t cx;
	uint32_t cy;

	struct vec2 size_i;
	struct vec2 offset;

	gs_texrender_t *window;

	bool loop;
};

static const char *scroll_filter_get_name(void *unused)
{
	UNUSED_PARAMETER(unused);
	return obs_module_text("ScrollFilter");
}

static void *scroll_filter_create(obs_data_t *settings, obs_source_t *context)
{
	struct scroll_filter_data *filter = bzalloc(sizeof(*filter));
	char *effect_path = obs_module_file("crop_filter.effect");

	filter->context = context;

	obs_enter_graphics();
	filter->effect = gs_effect_create_from_file(effect_path, NULL);
	obs_leave_graphics();

	bfree(effect_path);

	if (!filter->effect) {
		bfree(filter);
		return NULL;
	}

	filter->param_add = gs_effect_get_param_by_name(filter->effect, "add_val");
	filter->param_mul = gs_effect_get_param_by_name(filter->effect, "mul_val");
	filter->param_image = gs_effect_get_param_by_name(filter->effect, "image");
	filter->param_multiplier = gs_effect_get_param_by_name(filter->effect, "multiplier");

	obs_source_update(context, settings);
	return filter;
}

static void scroll_filter_destroy(void *data)
{
	struct scroll_filter_data *filter = data;

	obs_enter_graphics();
	gs_effect_destroy(filter->effect);
	gs_samplerstate_destroy(filter->sampler);
	gs_texrender_destroy(filter->window);
	obs_leave_graphics();

	bfree(filter);
}

static void scroll_filter_update(void *data, obs_data_t *settings)
{
	struct scroll_filter_data *filter = data;

	filter->limit_cx = obs_data_get_bool(settings, "limit_cx");
	filter->limit_cy = obs_data_get_bool(settings, "limit_cy");
	filter->cx = (uint32_t)obs_data_get_int(settings, "cx");
	filter->cy = (uint32_t)obs_data_get_int(settings, "cy");

	filter->scroll_speed.x = (float)obs_data_get_double(settings, "speed_x");
	filter->scroll_speed.y = (float)obs_data_get_double(settings, "speed_y");

	filter->loop = obs_data_get_bool(settings, "loop");

	struct gs_sampler_info sampler_info = {
		.filter = GS_FILTER_LINEAR,
		.address_u = filter->loop ? GS_ADDRESS_WRAP : GS_ADDRESS_BORDER,
		.address_v = filter->loop ? GS_ADDRESS_WRAP : GS_ADDRESS_BORDER,
	};

	obs_enter_graphics();
	gs_samplerstate_destroy(filter->sampler);
	filter->sampler = gs_samplerstate_create(&sampler_info);
	obs_leave_graphics();

	if (filter->scroll_speed.x == 0.0f)
		filter->offset.x = 0.0f;
	if (filter->scroll_speed.y == 0.0f)
		filter->offset.y = 0.0f;
}

static bool limit_cx_clicked(obs_properties_t *props, obs_property_t *p, obs_data_t *settings)
{
	bool limit_size = obs_data_get_bool(settings, "limit_cx");
	obs_property_set_visible(obs_properties_get(props, "cx"), limit_size);

	UNUSED_PARAMETER(p);
	return true;
}

static bool limit_cy_clicked(obs_properties_t *props, obs_property_t *p, obs_data_t *settings)
{
	bool limit_size = obs_data_get_bool(settings, "limit_cy");
	obs_property_set_visible(obs_properties_get(props, "cy"), limit_size);

	UNUSED_PARAMETER(p);
	return true;
}

static obs_properties_t *scroll_filter_properties(void *data)
{
	obs_properties_t *props = obs_properties_create();
	obs_property_t *p;

	obs_properties_add_float_slider(props, "speed_x", obs_module_text("ScrollFilter.SpeedX"), -500.0, 500.0, 1.0);
	obs_properties_add_float_slider(props, "speed_y", obs_module_text("ScrollFilter.SpeedY"), -500.0, 500.0, 1.0);

	p = obs_properties_add_bool(props, "limit_cx", obs_module_text("ScrollFilter.LimitWidth"));
	obs_property_set_modified_callback(p, limit_cx_clicked);
	obs_properties_add_int(props, "cx", obs_module_text("Crop.Width"), 1, 8192, 1);

	p = obs_properties_add_bool(props, "limit_cy", obs_module_text("ScrollFilter.LimitHeight"));
	obs_property_set_modified_callback(p, limit_cy_clicked);
	obs_properties_add_int(props, "cy", obs_module_text("Crop.Height"), 1, 8192, 1);

	obs_properties_add_bool(props, "loop", obs_module_text("ScrollFilter.Loop"));

	UNUSED_PARAMETER(data);
	return props;
}

static void scroll_filter_defaults(obs_data_t *settings)
{
	obs_data_set_default_bool(settings, "limit_size", false);
	obs_data_set_default_int(settings, "cx", 100);
	obs_data_set_default_int(settings, "cy", 100);
	obs_data_set_default_bool(settings, "loop", true);
}

static void scroll_filter_tick(void *data, float seconds)
{
	struct scroll_filter_data *filter = data;

	filter->offset.x += filter->size_i.x * filter->scroll_speed.x * seconds;
	filter->offset.y += filter->size_i.y * filter->scroll_speed.y * seconds;

	if (filter->loop) {
		if (filter->offset.x > 1.0f)
			filter->offset.x -= 1.0f;
		if (filter->offset.y > 1.0f)
			filter->offset.y -= 1.0f;
	} else {
		if (filter->offset.x > 1.0f)
			filter->offset.x = 1.0f;
		if (filter->offset.y > 1.0f)
			filter->offset.y = 1.0f;
	}
}

static const char *get_tech_name_and_multiplier(enum gs_color_space current_space, enum gs_color_space source_space,
						float *multiplier)
{
	const char *tech_name = "Draw";
	*multiplier = 1.f;

	switch (source_space) {
	case GS_CS_SRGB:
	case GS_CS_SRGB_16F:
		if (current_space == GS_CS_709_SCRGB) {
			tech_name = "DrawMultiply";
			*multiplier = obs_get_video_sdr_white_level() / 80.0f;
		}
		break;
	case GS_CS_709_EXTENDED:
		switch (current_space) {
		case GS_CS_SRGB:
		case GS_CS_SRGB_16F:
			tech_name = "DrawTonemap";
			break;
		case GS_CS_709_SCRGB:
			tech_name = "DrawMultiply";
			*multiplier = obs_get_video_sdr_white_level() / 80.0f;
			break;
		case GS_CS_709_EXTENDED:
			break;
		}
		break;
	case GS_CS_709_SCRGB:
		switch (current_space) {
		case GS_CS_SRGB:
		case GS_CS_SRGB_16F:
			tech_name = "DrawMultiplyTonemap";
			*multiplier = 80.0f / obs_get_video_sdr_white_level();
			break;
		case GS_CS_709_EXTENDED:
			tech_name = "DrawMultiply";
			*multiplier = 80.0f / obs_get_video_sdr_white_level();
			break;
		case GS_CS_709_SCRGB:
			break;
		}
	}

	return tech_name;
}

/* Offset of the window into the source, in pixels, for one axis. With Loop
 * the offset wraps into [0, size). */
static float window_start(float offset, uint32_t size, bool loop)
{
	float start = offset * (float)size;
	if (loop) {
		start = fmodf(start, (float)size);
		if (start < 0.0f)
			start += (float)size;
	}
	return start;
}

/* Draws the cx x cy window of a source too large for one texture into the
 * filter's own texture: the source is drawn with a projection that shows
 * only the window, once more past its end (or start) where a looping window
 * wraps around. Mirrors obs_source_process_filter_begin(). */
static gs_texture_t *render_window(struct scroll_filter_data *filter, uint32_t base_cx, uint32_t base_cy, uint32_t cx,
				   uint32_t cy, enum gs_color_format format, enum gs_color_space space)
{
	obs_source_t *target = obs_filter_get_target(filter->context);
	obs_source_t *parent = obs_filter_get_parent(filter->context);
	if (!target || !parent)
		return NULL;

	if (filter->window && gs_texrender_get_format(filter->window) != format) {
		gs_texrender_destroy(filter->window);
		filter->window = NULL;
	}
	if (!filter->window)
		filter->window = gs_texrender_create(format, GS_ZS_NONE);

	gs_texrender_reset(filter->window);
	if (!gs_texrender_begin_with_color_space(filter->window, cx, cy, space))
		return NULL;

	const uint32_t parent_flags = obs_source_get_output_flags(parent);
	const bool custom_draw = (parent_flags & OBS_SOURCE_CUSTOM_DRAW) != 0;
	const bool async = (parent_flags & OBS_SOURCE_ASYNC) != 0;

	const float x0 = window_start(filter->offset.x, base_cx, filter->loop);
	const float y0 = window_start(filter->offset.y, base_cy, filter->loop);
	const int copies_x = filter->loop && x0 + (float)cx > (float)base_cx ? 2 : 1;
	const int copies_y = filter->loop && y0 + (float)cy > (float)base_cy ? 2 : 1;

	gs_blend_state_push();
	gs_blend_function_separate(GS_BLEND_SRCALPHA, GS_BLEND_INVSRCALPHA, GS_BLEND_ONE, GS_BLEND_INVSRCALPHA);

	struct vec4 clear_color;
	vec4_zero(&clear_color);
	gs_clear(GS_CLEAR_COLOR, &clear_color, 0.0f, 0);

	for (int j = 0; j < copies_y; j++) {
		for (int i = 0; i < copies_x; i++) {
			const float left = x0 - (float)i * (float)base_cx;
			const float top = y0 - (float)j * (float)base_cy;
			gs_ortho(left, left + (float)cx, top, top + (float)cy, -100.0f, 100.0f);

			if (target == parent && !custom_draw && !async)
				obs_source_default_render(target);
			else
				obs_source_video_render(target);
		}
	}

	gs_blend_state_pop();
	gs_texrender_end(filter->window);

	return gs_texrender_get_texture(filter->window);
}

/* Draws the window texture from render_window() as the filter's output, as
 * obs_source_process_filter_tech_end() draws a filter's texture. */
static void draw_window(struct scroll_filter_data *filter, gs_texture_t *tex, uint32_t cx, uint32_t cy,
			const char *technique, float multiplier)
{
	struct vec2 add_val, mul_val;
	vec2_zero(&add_val);
	vec2_set(&mul_val, 1.0f, 1.0f);

	const bool previous_linear = gs_set_linear_srgb(true);
	const bool previous_fb = gs_framebuffer_srgb_enabled();
	gs_enable_framebuffer_srgb(true);

	gs_effect_set_vec2(filter->param_add, &add_val);
	gs_effect_set_vec2(filter->param_mul, &mul_val);
	gs_effect_set_float(filter->param_multiplier, multiplier);
	gs_effect_set_texture_srgb(filter->param_image, tex);

	gs_blend_state_push();
	gs_blend_function(GS_BLEND_ONE, GS_BLEND_INVSRCALPHA);
	while (gs_effect_loop(filter->effect, technique))
		gs_draw_sprite(tex, 0, cx, cy);
	gs_blend_state_pop();

	gs_enable_framebuffer_srgb(previous_fb);
	gs_set_linear_srgb(previous_linear);
}

static void scroll_filter_render(void *data, gs_effect_t *effect)
{
	UNUSED_PARAMETER(effect);

	struct scroll_filter_data *filter = data;
	struct vec2 mul_val;
	uint32_t base_cx;
	uint32_t base_cy;
	uint32_t cx;
	uint32_t cy;

	obs_source_t *target = obs_filter_get_target(filter->context);
	base_cx = obs_source_get_base_width(target);
	base_cy = obs_source_get_base_height(target);

	cx = filter->limit_cx ? filter->cx : base_cx;
	cy = filter->limit_cy ? filter->cy : base_cy;

	if (base_cx && base_cy) {
		vec2_set(&filter->size_i, 1.0f / (float)base_cx, 1.0f / (float)base_cy);
	} else {
		vec2_zero(&filter->size_i);
		obs_source_skip_video_filter(filter->context);
		return;
	}

	vec2_set(&mul_val, (float)cx / (float)base_cx, (float)cy / (float)base_cy);

	const enum gs_color_space preferred_spaces[] = {
		GS_CS_SRGB,
		GS_CS_SRGB_16F,
		GS_CS_709_EXTENDED,
	};

	const enum gs_color_space source_space = obs_source_get_color_space(
		obs_filter_get_target(filter->context), OBS_COUNTOF(preferred_spaces), preferred_spaces);
	float multiplier;
	const char *technique = get_tech_name_and_multiplier(gs_get_color_space(), source_space, &multiplier);
	const enum gs_color_format format = gs_get_format_from_space(source_space);

	/* A source too large for one texture: draw only the visible window.
	 * (obs_source_process_filter_begin_with_color_space() below would need
	 * a texture of the source's full size, and the filter showed nothing.) */
	if ((base_cx > MAX_TEXTURE_SIZE || base_cy > MAX_TEXTURE_SIZE) && cx <= MAX_TEXTURE_SIZE &&
	    cy <= MAX_TEXTURE_SIZE) {
		gs_texture_t *tex = render_window(filter, base_cx, base_cy, cx, cy, format, source_space);
		if (tex)
			draw_window(filter, tex, cx, cy, technique, multiplier);
		return;
	}

	if (obs_source_process_filter_begin_with_color_space(filter->context, format, source_space,
							     OBS_NO_DIRECT_RENDERING)) {
		gs_effect_set_vec2(filter->param_add, &filter->offset);
		gs_effect_set_vec2(filter->param_mul, &mul_val);
		gs_effect_set_float(filter->param_multiplier, multiplier);

		gs_effect_set_next_sampler(filter->param_image, filter->sampler);

		gs_blend_state_push();
		gs_blend_function(GS_BLEND_ONE, GS_BLEND_INVSRCALPHA);

		obs_source_process_filter_tech_end(filter->context, filter->effect, cx, cy, technique);

		gs_blend_state_pop();
	}
}

static uint32_t scroll_filter_width(void *data)
{
	struct scroll_filter_data *filter = data;
	obs_source_t *target = obs_filter_get_target(filter->context);

	return filter->limit_cx ? filter->cx : obs_source_get_base_width(target);
}

static uint32_t scroll_filter_height(void *data)
{
	struct scroll_filter_data *filter = data;
	obs_source_t *target = obs_filter_get_target(filter->context);

	return filter->limit_cy ? filter->cy : obs_source_get_base_height(target);
}

static void scroll_filter_show(void *data)
{
	struct scroll_filter_data *filter = data;
	filter->offset.x = 0.0f;
	filter->offset.y = 0.0f;
}

static enum gs_color_space scroll_filter_get_color_space(void *data, size_t count,
							 const enum gs_color_space *preferred_spaces)
{
	const enum gs_color_space potential_spaces[] = {
		GS_CS_SRGB,
		GS_CS_SRGB_16F,
		GS_CS_709_EXTENDED,
	};

	struct scroll_filter_data *const filter = data;
	const enum gs_color_space source_space = obs_source_get_color_space(
		obs_filter_get_target(filter->context), OBS_COUNTOF(potential_spaces), potential_spaces);

	enum gs_color_space space = source_space;
	for (size_t i = 0; i < count; ++i) {
		space = preferred_spaces[i];
		if (space == source_space)
			break;
	}

	return space;
}

struct obs_source_info scroll_filter = {
	.id = "scroll_filter",
	.type = OBS_SOURCE_TYPE_FILTER,
	.output_flags = OBS_SOURCE_VIDEO | OBS_SOURCE_SRGB,
	.get_name = scroll_filter_get_name,
	.create = scroll_filter_create,
	.destroy = scroll_filter_destroy,
	.update = scroll_filter_update,
	.get_properties = scroll_filter_properties,
	.get_defaults = scroll_filter_defaults,
	.video_tick = scroll_filter_tick,
	.video_render = scroll_filter_render,
	.get_width = scroll_filter_width,
	.get_height = scroll_filter_height,
	.show = scroll_filter_show,
	.video_get_color_space = scroll_filter_get_color_space,
};
