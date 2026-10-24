/******************************************************************************
    Copyright (C) 2023 by Lain Bailey <lain@obsproject.com>

    This program is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 2 of the License, or
    (at your option) any later version.

    This program is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with this program.  If not, see <http://www.gnu.org/licenses/>.
******************************************************************************/

#include "libobs/build/libobs.pch.h"

static uint64_t tick_sources(uint64_t cur_time, uint64_t last_time)
{
	struct obs_core_data *data = &obs->data;
	struct obs_source *source;
	uint64_t delta_time;
	float seconds;

	if (!last_time)
		last_time = cur_time - obs->video.video_frame_interval_ns;

	delta_time = cur_time - last_time;
	seconds = (float)((double)delta_time / 1000000000.0);

	/* ------------------------------------- */
	/* call tick callbacks                   */

	pthread_mutex_lock(&data->draw_callbacks_mutex);

	for (size_t i = data->tick_callbacks.num; i > 0; i--) {
		struct tick_callback *callback;
		callback = data->tick_callbacks.array + (i - 1);
		callback->tick(callback->param, seconds);
	}

	pthread_mutex_unlock(&data->draw_callbacks_mutex);

	/* ------------------------------------- */
	/* get an array of all sources to tick   */

	da_clear(data->sources_to_tick);

	pthread_mutex_lock(&data->sources_mutex);

	source = data->sources;
	while (source) {
		obs_source_t *s = obs_source_removed(source) ? NULL : obs_source_get_ref(source);
		if (s)
			da_push_back(data->sources_to_tick, &s);
		source = (struct obs_source *)source->context.hh_uuid.next;
	}

	pthread_mutex_unlock(&data->sources_mutex);

	/* ------------------------------------- */
	/* call the tick function of each source */

	for (size_t i = 0; i < data->sources_to_tick.num; i++) {
		obs_source_t *s = data->sources_to_tick.array[i];
		if (!obs_source_removed(s)) {
			const uint64_t start = source_profiler_source_tick_start();
			obs_source_video_tick(s, seconds);
			source_profiler_source_tick_end(s, start);
		}
		obs_source_release(s);
	}

	return cur_time;
}

/* in obs-display.c */
extern void render_display(struct obs_display *display);

static inline void render_displays(void)
{
	struct obs_display *display;

	if (!obs->data.valid)
		return;

	gs_enter_context(obs->video.graphics);

	/* render extra displays/swaps */
	pthread_mutex_lock(&obs->data.displays_mutex);

	display = obs->data.first_display;
	while (display) {
		render_display(display);
		display = display->next;
	}

	pthread_mutex_unlock(&obs->data.displays_mutex);

	gs_leave_context();
}

static inline void set_render_size(uint32_t width, uint32_t height)
{
	gs_enable_depth_test(false);
	gs_set_cull_mode(GS_NEITHER);

	gs_ortho(0.0f, (float)width, 0.0f, (float)height, -100.0f, 100.0f);
	gs_set_viewport(0, 0, width, height);
}

static inline void unmap_last_surface(struct obs_core_video_mix *video)
{
	for (int c = 0; c < NUM_CHANNELS; ++c) {
		if (video->mapped_surfaces[c]) {
			gs_stagesurface_unmap(video->mapped_surfaces[c]);
			video->mapped_surfaces[c] = NULL;
		}
	}
}

static inline struct obs_video_mix_reuse_key mix_reuse_key(const struct obs_core_video_mix *mix)
{
	struct obs_video_mix_reuse_key key = {
		.view = (uintptr_t)mix->view,
		.render_space = mix->render_space,
		.base_width = mix->ovi.base_width,
		.base_height = mix->ovi.base_height,
		.rendered = mix->texture_rendered,
	};
	return key;
}

static inline bool can_reuse_mix_texture(const struct obs_core_video_mix *mix, size_t *idx)
{
	const struct obs_video_mix_reuse_key key = mix_reuse_key(mix);

	for (size_t i = 0, num = obs->video.mixes.num; i < num; i++) {
		const struct obs_core_video_mix *other = obs->video.mixes.array[i];
		if (other == mix)
			break;

		const struct obs_video_mix_reuse_key other_key = mix_reuse_key(other);
		if (!obs_video_mix_reuse_match(&other_key, &key))
			continue;

		*idx = i;
		return true;
	}

	return false;
}

static inline void draw_mix_texture(const size_t mix_idx)
{
	gs_texture_t *tex = obs->video.mixes.array[mix_idx]->render_texture;
	gs_effect_t *effect = obs_get_base_effect(OBS_EFFECT_DEFAULT);
	gs_eparam_t *param = gs_effect_get_param_by_name(effect, "image");
	gs_effect_set_texture_srgb(param, tex);

	/* A copy of the other mix's canvas, alpha included: blending would
	 * multiply the colour by an alpha that the canvas can have at 0 where
	 * colour shows (Subtract blending), and those areas would turn black. */
	gs_blend_state_push();
	gs_enable_blending(false);
	gs_enable_framebuffer_srgb(true);
	while (gs_effect_loop(effect, "Draw"))
		gs_draw_sprite(tex, 0, 0, 0);
	gs_enable_framebuffer_srgb(false);
	gs_blend_state_pop();
}

static const char *render_main_texture_name = "render_main_texture";
static inline void render_main_texture(struct obs_core_video_mix *video)
{
	uint32_t base_width = video->ovi.base_width;
	uint32_t base_height = video->ovi.base_height;

	profile_start(render_main_texture_name);
	GS_DEBUG_MARKER_BEGIN(GS_DEBUG_COLOR_MAIN_TEXTURE, render_main_texture_name);

	struct vec4 clear_color;
	vec4_set(&clear_color, 0.0f, 0.0f, 0.0f, 0.0f);

	gs_set_render_target_with_color_space(video->render_texture, NULL, video->render_space);
	gs_clear(GS_CLEAR_COLOR, &clear_color, 1.0f, 0);

	set_render_size(base_width, base_height);

	pthread_mutex_lock(&obs->data.draw_callbacks_mutex);

	for (size_t i = obs->data.draw_callbacks.num; i > 0; i--) {
		struct draw_callback *const callback = obs->data.draw_callbacks.array + (i - 1);
		callback->draw(callback->param, base_width, base_height);
	}

	pthread_mutex_unlock(&obs->data.draw_callbacks_mutex);

	/* In some cases we can reuse a previous mix's texture and save re-rendering everything */
	size_t reuse_idx;
	if (can_reuse_mix_texture(video, &reuse_idx))
		draw_mix_texture(reuse_idx);
	else
		obs_view_render(video->view);

	video->texture_rendered = true;

	pthread_mutex_lock(&obs->data.draw_callbacks_mutex);

	for (size_t i = 0; i < obs->data.rendered_callbacks.num; ++i) {
		struct rendered_callback *const callback = &obs->data.rendered_callbacks.array[i];
		callback->rendered(callback->param);
	}

	pthread_mutex_unlock(&obs->data.draw_callbacks_mutex);

	GS_DEBUG_MARKER_END();
	profile_end(render_main_texture_name);
}

/* The scale types obs-video-copy-plan.h copies. */
_Static_assert(OBS_VIDEO_COPY_SCALE_DISABLE == OBS_SCALE_DISABLE, "OBS_SCALE_DISABLE");
_Static_assert(OBS_VIDEO_COPY_SCALE_POINT == OBS_SCALE_POINT, "OBS_SCALE_POINT");
_Static_assert(OBS_VIDEO_COPY_SCALE_BICUBIC == OBS_SCALE_BICUBIC, "OBS_SCALE_BICUBIC");
_Static_assert(OBS_VIDEO_COPY_SCALE_BILINEAR == OBS_SCALE_BILINEAR, "OBS_SCALE_BILINEAR");
_Static_assert(OBS_VIDEO_COPY_SCALE_LANCZOS == OBS_SCALE_LANCZOS, "OBS_SCALE_LANCZOS");
_Static_assert(OBS_VIDEO_COPY_SCALE_AREA == OBS_SCALE_AREA, "OBS_SCALE_AREA");
_Static_assert(OBS_VIDEO_COPY_MAX == NUM_CHANNELS, "NUM_CHANNELS");

static inline uint32_t loaded_scale_effects(const struct obs_core_video *video)
{
	uint32_t loaded = 0;
	if (video->default_effect)
		loaded |= OBS_VIDEO_SCALE_LOADED(OBS_VIDEO_SCALE_EFFECT_DEFAULT);
	if (video->bilinear_lowres_effect)
		loaded |= OBS_VIDEO_SCALE_LOADED(OBS_VIDEO_SCALE_EFFECT_BILINEAR_LOWRES);
	if (video->bicubic_effect)
		loaded |= OBS_VIDEO_SCALE_LOADED(OBS_VIDEO_SCALE_EFFECT_BICUBIC);
	if (video->lanczos_effect)
		loaded |= OBS_VIDEO_SCALE_LOADED(OBS_VIDEO_SCALE_EFFECT_LANCZOS);
	if (video->area_effect)
		loaded |= OBS_VIDEO_SCALE_LOADED(OBS_VIDEO_SCALE_EFFECT_AREA);
	return loaded;
}

static inline gs_effect_t *scale_effect_pointer(const struct obs_core_video *video, enum obs_video_scale_effect effect)
{
	switch (effect) {
	case OBS_VIDEO_SCALE_EFFECT_BILINEAR_LOWRES:
		return video->bilinear_lowres_effect;
	case OBS_VIDEO_SCALE_EFFECT_BICUBIC:
		return video->bicubic_effect;
	case OBS_VIDEO_SCALE_EFFECT_LANCZOS:
		return video->lanczos_effect;
	case OBS_VIDEO_SCALE_EFFECT_AREA:
		return video->area_effect;
	case OBS_VIDEO_SCALE_EFFECT_DEFAULT:
	default:
		return video->default_effect;
	}
}

static inline gs_effect_t *get_scale_effect(struct obs_core_video_mix *mix, uint32_t width, uint32_t height)
{
	struct obs_core_video *video = &obs->video;
	struct obs_video_scale_input input = {
		.base_width = mix->ovi.base_width,
		.base_height = mix->ovi.base_height,
		.target_width = width,
		.target_height = height,
		.scale_type = mix->ovi.scale_type,
		.loaded = loaded_scale_effects(video),
	};

	/* The output's size is only read when the target is not close to the
	 * base size, as before. */
	if (!obs_video_scale_resolution_close(input.base_width, input.base_height, width, height)) {
		const struct video_output_info *info = video_output_get_info(mix->video);
		input.output_width = info->width;
		input.output_height = info->height;
	}

	return scale_effect_pointer(video, obs_video_scale_effect_choice(&input));
}

static const char *render_output_texture_name = "render_output_texture";
static inline gs_texture_t *render_output_texture(struct obs_core_video_mix *mix)
{
	struct obs_video_info *const ovi = &mix->ovi;
	gs_texture_t *texture = mix->render_texture;
	gs_texture_t *target = mix->output_texture;
	const uint32_t width = gs_texture_get_width(target);
	const uint32_t height = gs_texture_get_height(target);
	if ((width == ovi->base_width) && (height == ovi->base_height))
		return texture;

	profile_start(render_output_texture_name);

	gs_effect_t *effect = get_scale_effect(mix, width, height);
	gs_technique_t *tech = gs_effect_get_technique(effect, "Draw");

	gs_eparam_t *image = gs_effect_get_param_by_name(effect, "image");
	gs_eparam_t *bres = gs_effect_get_param_by_name(effect, "base_dimension");
	gs_eparam_t *bres_i = gs_effect_get_param_by_name(effect, "base_dimension_i");
	size_t passes, i;

	gs_set_render_target(target, NULL);
	set_render_size(width, height);

	if (bres) {
		struct vec2 base;
		vec2_set(&base, (float)mix->ovi.base_width, (float)mix->ovi.base_height);
		gs_effect_set_vec2(bres, &base);
	}

	if (bres_i) {
		struct vec2 base_i;
		vec2_set(&base_i, 1.0f / (float)mix->ovi.base_width, 1.0f / (float)mix->ovi.base_height);
		gs_effect_set_vec2(bres_i, &base_i);
	}

	gs_effect_set_texture_srgb(image, texture);

	gs_enable_framebuffer_srgb(true);
	gs_enable_blending(false);
	passes = gs_technique_begin(tech);
	for (i = 0; i < passes; i++) {
		gs_technique_begin_pass(tech, i);
		gs_draw_sprite(texture, 0, width, height);
		gs_technique_end_pass(tech);
	}
	gs_technique_end(tech);
	gs_enable_blending(true);
	gs_enable_framebuffer_srgb(false);

	profile_end(render_output_texture_name);

	return target;
}

static void render_convert_plane(gs_effect_t *effect, gs_texture_t *target, const char *tech_name)
{
	gs_technique_t *tech = gs_effect_get_technique(effect, tech_name);

	const uint32_t width = gs_texture_get_width(target);
	const uint32_t height = gs_texture_get_height(target);

	gs_set_render_target(target, NULL);
	set_render_size(width, height);

	size_t passes = gs_technique_begin(tech);
	for (size_t i = 0; i < passes; i++) {
		gs_technique_begin_pass(tech, i);
		gs_draw(GS_TRIS, 0, 3);
		gs_technique_end_pass(tech);
	}
	gs_technique_end(tech);
}

static const char *render_convert_texture_name = "render_convert_texture";
static void render_convert_texture(struct obs_core_video_mix *video, gs_texture_t *const *const convert_textures,
				   gs_texture_t *texture)
{
	profile_start(render_convert_texture_name);

	gs_effect_t *effect = obs->video.conversion_effect;
	gs_eparam_t *color_vec0 = gs_effect_get_param_by_name(effect, "color_vec0");
	gs_eparam_t *color_vec1 = gs_effect_get_param_by_name(effect, "color_vec1");
	gs_eparam_t *color_vec2 = gs_effect_get_param_by_name(effect, "color_vec2");
	gs_eparam_t *image = gs_effect_get_param_by_name(effect, "image");
	gs_eparam_t *width_i = gs_effect_get_param_by_name(effect, "width_i");
	gs_eparam_t *height_i = gs_effect_get_param_by_name(effect, "height_i");
	gs_eparam_t *sdr_white_nits_over_maximum = gs_effect_get_param_by_name(effect, "sdr_white_nits_over_maximum");
	gs_eparam_t *hdr_lw = gs_effect_get_param_by_name(effect, "hdr_lw");

	struct vec4 vec0, vec1, vec2;
	vec4_set(&vec0, video->color_matrix[4], video->color_matrix[5], video->color_matrix[6], video->color_matrix[7]);
	vec4_set(&vec1, video->color_matrix[0], video->color_matrix[1], video->color_matrix[2], video->color_matrix[3]);
	vec4_set(&vec2, video->color_matrix[8], video->color_matrix[9], video->color_matrix[10],
		 video->color_matrix[11]);

	gs_enable_blending(false);

	if (convert_textures[0]) {
		const float hdr_nominal_peak_level = obs->video.hdr_nominal_peak_level;
		const float multiplier = obs_get_video_sdr_white_level() / 10000.f;
		gs_effect_set_texture(image, texture);
		gs_effect_set_vec4(color_vec0, &vec0);
		gs_effect_set_float(sdr_white_nits_over_maximum, multiplier);
		gs_effect_set_float(hdr_lw, hdr_nominal_peak_level);
		render_convert_plane(effect, convert_textures[0], video->conversion_techs[0]);

		if (convert_textures[1]) {
			gs_effect_set_texture(image, texture);
			gs_effect_set_vec4(color_vec1, &vec1);
			if (!convert_textures[2])
				gs_effect_set_vec4(color_vec2, &vec2);
			gs_effect_set_float(width_i, video->conversion_width_i);
			gs_effect_set_float(height_i, video->conversion_height_i);
			gs_effect_set_float(sdr_white_nits_over_maximum, multiplier);
			gs_effect_set_float(hdr_lw, hdr_nominal_peak_level);
			render_convert_plane(effect, convert_textures[1], video->conversion_techs[1]);

			if (convert_textures[2]) {
				gs_effect_set_texture(image, texture);
				gs_effect_set_vec4(color_vec2, &vec2);
				gs_effect_set_float(width_i, video->conversion_width_i);
				gs_effect_set_float(height_i, video->conversion_height_i);
				gs_effect_set_float(sdr_white_nits_over_maximum, multiplier);
				gs_effect_set_float(hdr_lw, hdr_nominal_peak_level);
				render_convert_plane(effect, convert_textures[2], video->conversion_techs[2]);
			}
		}
	}

	gs_enable_blending(true);

	video->texture_converted = true;

	profile_end(render_convert_texture_name);
}

/* The texture ring and channel counts obs-video-output-step.h copies. */
_Static_assert(OBS_VIDEO_STEP_TEXTURES == NUM_TEXTURES, "NUM_TEXTURES");
_Static_assert(OBS_VIDEO_STEP_CHANNELS == NUM_CHANNELS, "NUM_CHANNELS");

static const char *stage_output_texture_name = "stage_output_texture";
static inline void stage_output_texture(struct obs_core_video_mix *video, int cur_texture,
					gs_texture_t *const *const convert_textures, gs_texture_t *output_texture,
					gs_stagesurf_t *const *const copy_surfaces, size_t channel_count)
{
	profile_start(stage_output_texture_name);

	unmap_last_surface(video);

	const struct obs_video_step_stage stage =
		obs_video_step_stage(video->gpu_conversion, video->texture_converted, channel_count);

	if (stage.staged) {
		for (size_t i = 0; i < stage.channels; i++) {
			gs_stagesurf_t *copy = copy_surfaces[i];
			if (copy)
				gs_stage_texture(copy, stage.from_output ? output_texture : convert_textures[i]);
			video->active_copy_surfaces[cur_texture][i] = copy;
		}

		for (size_t i = stage.channels; i < NUM_CHANNELS; ++i)
			video->active_copy_surfaces[cur_texture][i] = NULL;

		video->textures_copied[cur_texture] = true;
	}

	profile_end(stage_output_texture_name);
}

static inline bool queue_frame(struct obs_core_video_mix *video, bool raw_active, struct obs_vframe_info *vframe_info)
{
	const enum obs_video_step_queue_action action = obs_video_step_queue(
		video->gpu_encoder_avail_queue.size, video->gpu_encoder_queue.size, vframe_info->count, raw_active);

	if (action == OBS_VIDEO_STEP_QUEUE_DUPLICATE) {
		struct obs_tex_frame *tf =
			deque_data(&video->gpu_encoder_queue, video->gpu_encoder_queue.size - sizeof(*tf));

		/* texture-based encoding is stopping */
		if (!tf) {
			return false;
		}

		tf->count++;
		os_sem_post(video->gpu_encode_semaphore);
		goto finish;
	}

	struct obs_tex_frame tf;
	deque_pop_front(&video->gpu_encoder_avail_queue, &tf, sizeof(tf));

	if (tf.released) {
		obs_host_tex_frame_acquire(&tf);
		tf.released = false;
	}

	if (action == OBS_VIDEO_STEP_QUEUE_COPY) {
		gs_copy_texture(tf.tex, video->convert_textures_encode[0]);
		if (obs_video_step_queue_copy_uv(obs_host_nv12_planes_share_texture()))
			gs_copy_texture(tf.tex_uv, video->convert_textures_encode[1]);
	} else {
		gs_texture_t *tex = video->convert_textures_encode[0];
		gs_texture_t *tex_uv = video->convert_textures_encode[1];

		video->convert_textures_encode[0] = tf.tex;
		video->convert_textures_encode[1] = tf.tex_uv;

		tf.tex = tex;
		tf.tex_uv = tex_uv;
	}

	tf.count = 1;
	tf.timestamp = vframe_info->timestamp;
	tf.released = true;
	obs_host_tex_frame_release(&tf);
	deque_push_back(&video->gpu_encoder_queue, &tf, sizeof(tf));

	os_sem_post(video->gpu_encode_semaphore);

finish:
	vframe_info->count = obs_video_step_queue_count(vframe_info->count);
	return vframe_info->count;
}

extern void full_stop(struct obs_encoder *encoder);

static inline void encode_gpu(struct obs_core_video_mix *video, bool raw_active, struct obs_vframe_info *vframe_info)
{
	while (queue_frame(video, raw_active, vframe_info))
		;
}

static const char *output_gpu_encoders_name = "output_gpu_encoders";
static void output_gpu_encoders(struct obs_core_video_mix *video, bool raw_active)
{
	profile_start(output_gpu_encoders_name);

	if (!video->texture_converted)
		goto end;
	if (!video->vframe_info_buffer_gpu.size)
		goto end;

	struct obs_vframe_info vframe_info;
	deque_pop_front(&video->vframe_info_buffer_gpu, &vframe_info, sizeof(vframe_info));

	pthread_mutex_lock(&video->gpu_encoder_mutex);
	encode_gpu(video, raw_active, &vframe_info);
	pthread_mutex_unlock(&video->gpu_encoder_mutex);

end:
	profile_end(output_gpu_encoders_name);
}

static inline void render_video(struct obs_core_video_mix *video, bool raw_active, const bool gpu_active,
				int cur_texture)
{
	gs_begin_scene();

	gs_enable_depth_test(false);
	gs_set_cull_mode(GS_NEITHER);

	render_main_texture(video);

	const struct obs_video_step_render_targets targets = obs_video_step_render_targets(raw_active, gpu_active);
	if (targets.outputs) {
		gs_texture_t *const *convert_textures = video->convert_textures;
		gs_stagesurf_t *const *copy_surfaces = video->copy_surfaces[cur_texture];
		size_t channel_count = NUM_CHANNELS;
		gs_texture_t *output_texture = render_output_texture(video);

		if (targets.encode) {
			convert_textures = video->convert_textures_encode;
			obs_host_gpu_encode_surfaces(video, &copy_surfaces, &channel_count);
		}

		const struct obs_video_step_render step =
			obs_video_step_render(raw_active, gpu_active, video->gpu_conversion);

		if (step.convert) {
			render_convert_texture(video, convert_textures, output_texture);
		}

		if (step.encode) {
			output_gpu_encoders(video, raw_active);
		}

		if (step.stage) {
			stage_output_texture(video, cur_texture, convert_textures, output_texture, copy_surfaces,
					     channel_count);
		}
	}

	gs_set_render_target(NULL, NULL);
	gs_enable_blending(true);

	gs_end_scene();
}

static inline bool download_frame(struct obs_core_video_mix *video, int prev_texture, struct video_data *frame)
{
	if (!video->textures_copied[prev_texture])
		return false;

	for (int channel = 0; channel < NUM_CHANNELS; ++channel) {
		gs_stagesurf_t *surface = video->active_copy_surfaces[prev_texture][channel];
		if (surface) {
			if (!gs_stagesurface_map(surface, &frame->data[channel], &frame->linesize[channel]))
				return false;

			video->mapped_surfaces[channel] = surface;
		}
	}
	return true;
}

/* One copy of the plan (obs-video-copy-plan.c): returns where the source
 * ended. */
static const uint8_t *set_gpu_converted_plane(const struct obs_video_copy *copy, const uint8_t *in, uint8_t *out)
{
	if (copy->whole) {
		memcpy(out, in, copy->bytes);
		in += copy->bytes;
	} else {
		for (size_t y = 0; y < copy->rows; y++) {
			memcpy(out, in, copy->bytes);
			out += copy->dst_stride;
			in += copy->src_stride;
		}
	}

	return in;
}

static void set_gpu_converted_data(struct video_frame *output, const struct video_data *input,
				   const struct video_output_info *info)
{
	const struct obs_video_copy_plan plan =
		obs_video_gpu_copy_plan(info->format, info->width, info->height, input->linesize, output->linesize);
	const uint8_t *after_plane0 = NULL;

	for (size_t i = 0; i < plan.count && i < OBS_VIDEO_COPY_MAX; i++) {
		const struct obs_video_copy *copy = &plan.copies[i];
		const uint8_t *in = copy->src_after_plane0 ? after_plane0 : input->data[i];
		const uint8_t *end = set_gpu_converted_plane(copy, in, output->data[i]);
		if (i == 0)
			after_plane0 = end;
	}
}

static inline void copy_rgbx_frame(struct video_frame *output, const struct video_data *input,
				   const struct video_output_info *info)
{
	const struct obs_video_copy copy =
		obs_video_rgbx_copy_plan(info->width, info->height, input->linesize[0], output->linesize[0]);

	set_gpu_converted_plane(&copy, input->data[0], output->data[0]);
}

static inline void output_video_data(struct obs_core_video_mix *video, struct video_data *input_frame, int count)
{
	const struct video_output_info *info;
	struct video_frame output_frame;
	bool locked;

	info = video_output_get_info(video->video);

	locked = video_output_lock_frame(video->video, &output_frame, count, input_frame->timestamp);
	if (locked) {
		if (video->gpu_conversion) {
			set_gpu_converted_data(&output_frame, input_frame, info);
		} else {
			copy_rgbx_frame(&output_frame, input_frame, info);
		}

		video_output_unlock_frame(video->video);
	}
}

void add_ready_encoder_group(obs_encoder_t *encoder)
{
	obs_weak_encoder_t *weak = obs_encoder_get_weak_encoder(encoder);
	pthread_mutex_lock(&obs->video.encoder_group_mutex);
	da_push_back(obs->video.ready_encoder_groups, &weak);
	pthread_mutex_unlock(&obs->video.encoder_group_mutex);
}

static inline void video_sleep(struct obs_core_video *video, uint64_t *p_time, uint64_t interval_ns)
{
	struct obs_vframe_info vframe_info;
	uint64_t cur_time = *p_time;
	uint64_t t = cur_time + interval_ns;
	int count;

	if (os_sleepto_ns(t)) {
		*p_time = t;
		count = 1;
	} else {
		const uint64_t udiff = os_gettime_ns() - cur_time;
		int64_t diff;
		memcpy(&diff, &udiff, sizeof(diff));
		const uint64_t clamped_diff = (diff > (int64_t)interval_ns) ? (uint64_t)diff : interval_ns;
		count = (int)(clamped_diff / interval_ns);
		*p_time = cur_time + interval_ns * count;
	}

	obs_video_stats_add_frames(&video->stats, count, count - 1);

	vframe_info.timestamp = cur_time;
	vframe_info.count = count;

	pthread_mutex_lock(&video->encoder_group_mutex);
	for (size_t i = 0; i < video->ready_encoder_groups.num; i++) {
		obs_encoder_t *encoder = obs_weak_encoder_get_encoder(video->ready_encoder_groups.array[i]);
		obs_weak_encoder_release(video->ready_encoder_groups.array[i]);
		if (!encoder)
			continue;

		if (encoder->encoder_group) {
			struct obs_encoder_group *group = encoder->encoder_group;
			pthread_mutex_lock(&group->mutex);
			if (group->num_encoders_started >= group->encoders.num && !group->start_timestamp)
				group->start_timestamp = *p_time;
			pthread_mutex_unlock(&group->mutex);
		}
		obs_encoder_release(encoder);
	}
	da_clear(video->ready_encoder_groups);
	pthread_mutex_unlock(&video->encoder_group_mutex);

	pthread_mutex_lock(&obs->video.mixes_mutex);
	for (size_t i = 0, num = obs->video.mixes.num; i < num; i++) {
		struct obs_core_video_mix *mix = obs->video.mixes.array[i];
		bool raw_active = mix->raw_was_active;
		bool gpu_active = mix->gpu_was_active;

		if (raw_active)
			deque_push_back(&mix->vframe_info_buffer, &vframe_info, sizeof(vframe_info));
		if (gpu_active)
			deque_push_back(&mix->vframe_info_buffer_gpu, &vframe_info, sizeof(vframe_info));
	}
	pthread_mutex_unlock(&obs->video.mixes_mutex);
}

static const char *output_frame_gs_context_name = "gs_context(video->graphics)";
static const char *output_frame_render_video_name = "render_video";
static const char *output_frame_download_frame_name = "download_frame";
static const char *output_frame_gs_flush_name = "gs_flush";
static const char *output_frame_output_video_data_name = "output_video_data";
static inline void output_frame(struct obs_core_video_mix *video)
{
	const bool raw_active = video->raw_was_active;
	const bool gpu_active = video->gpu_was_active;

	int cur_texture = video->cur_texture;
	int prev_texture = obs_video_step_ring(cur_texture).prev;
	struct video_data frame;
	bool frame_ready = 0;

	memset(&frame, 0, sizeof(struct video_data));

	profile_start(output_frame_gs_context_name);
	gs_enter_context(obs->video.graphics);

	profile_start(output_frame_render_video_name);
	GS_DEBUG_MARKER_BEGIN(GS_DEBUG_COLOR_RENDER_VIDEO, output_frame_render_video_name);
	render_video(video, raw_active, gpu_active, cur_texture);
	GS_DEBUG_MARKER_END();
	profile_end(output_frame_render_video_name);

	if (obs_video_step_download(raw_active)) {
		profile_start(output_frame_download_frame_name);
		frame_ready = download_frame(video, prev_texture, &frame);
		profile_end(output_frame_download_frame_name);
	}

	profile_start(output_frame_gs_flush_name);
	gs_flush();
	profile_end(output_frame_gs_flush_name);

	gs_leave_context();
	profile_end(output_frame_gs_context_name);

	if (obs_video_step_output(raw_active, frame_ready)) {
		struct obs_vframe_info vframe_info;
		deque_pop_front(&video->vframe_info_buffer, &vframe_info, sizeof(vframe_info));

		frame.timestamp = vframe_info.timestamp;
		profile_start(output_frame_output_video_data_name);
		output_video_data(video, &frame, vframe_info.count);
		profile_end(output_frame_output_video_data_name);
	}

	video->cur_texture = obs_video_step_ring(video->cur_texture).next;
}

static inline void output_frames(void)
{
	pthread_mutex_lock(&obs->video.mixes_mutex);
	for (size_t i = 0, num = obs->video.mixes.num; i < num; i++) {
		struct obs_core_video_mix *mix = obs->video.mixes.array[i];
		if (mix->view) {
			output_frame(mix);
		} else {
			obs->video.mixes.array[i] = NULL;
			obs_free_video_mix(mix);
			da_erase(obs->video.mixes, i);
			i--;
			num--;
		}
	}
	pthread_mutex_unlock(&obs->video.mixes_mutex);
}

#define NBSP "\xC2\xA0"

static void clear_base_frame_data(struct obs_core_video_mix *video)
{
	video->texture_rendered = false;
	video->texture_converted = false;
	deque_free(&video->vframe_info_buffer);
	video->cur_texture = 0;
}

static void clear_raw_frame_data(struct obs_core_video_mix *video)
{
	memset(video->textures_copied, 0, sizeof(video->textures_copied));
	deque_free(&video->vframe_info_buffer);
}

static void clear_gpu_frame_data(struct obs_core_video_mix *video)
{
	deque_free(&video->vframe_info_buffer_gpu);
}

extern THREAD_LOCAL bool is_graphics_thread;

static void execute_graphics_tasks(void)
{
	struct obs_core_video *video = &obs->video;
	pthread_mutex_lock(&video->task_mutex);
	while (video->tasks.size) {
		struct obs_task_info info;
		deque_pop_front(&video->tasks, &info, sizeof(info));
		pthread_mutex_unlock(&video->task_mutex);
		info.task(info.param);
		pthread_mutex_lock(&video->task_mutex);
	}
	pthread_mutex_unlock(&video->task_mutex);
}

static const char *tick_sources_name = "tick_sources";
static const char *render_displays_name = "render_displays";
static const char *output_frame_name = "output_frame";
static inline void update_active_state(struct obs_core_video_mix *video)
{
	struct obs_video_step_activation_input input = {
		.raw_was_active = video->raw_was_active,
		.gpu_was_active = video->gpu_was_active,
		.was_active = video->was_active,
	};

	input.raw_active = os_atomic_load_long(&video->raw_active);
	input.gpu_encoder_active = os_atomic_load_long(&video->gpu_encoder_active);

	const struct obs_video_step_activation step = obs_video_step_activation(&input);

	if (step.clear_base)
		clear_base_frame_data(video);
	if (step.clear_raw)
		clear_raw_frame_data(video);
	if (step.clear_gpu)
		clear_gpu_frame_data(video);

	video->gpu_was_active = step.gpu_active;
	video->raw_was_active = step.raw_active;
	video->was_active = step.active;
}

static inline void update_active_states(void)
{
	pthread_mutex_lock(&obs->video.mixes_mutex);
	for (size_t i = 0, num = obs->video.mixes.num; i < num; i++)
		update_active_state(obs->video.mixes.array[i]);
	pthread_mutex_unlock(&obs->video.mixes_mutex);
}

static inline bool stop_requested(void)
{
	bool success = true;

	pthread_mutex_lock(&obs->video.mixes_mutex);
	for (size_t i = 0, num = obs->video.mixes.num; i < num; i++)
		if (!video_output_stopped(obs->video.mixes.array[i]->video))
			success = false;
	pthread_mutex_unlock(&obs->video.mixes_mutex);

	return success;
}

bool obs_graphics_thread_loop(struct obs_graphics_context *context)
{
	uint64_t frame_start = os_gettime_ns();
	uint64_t frame_time_ns;

	update_active_states();

	profile_start(context->video_thread_name);
	source_profiler_frame_begin();

	gs_enter_context(obs->video.graphics);
	gs_begin_frame();
	gs_leave_context();

	profile_start(tick_sources_name);
	context->last_time = tick_sources(obs->video.video_time, context->last_time);
	profile_end(tick_sources_name);

	obs_host_pump_messages();

	source_profiler_render_begin();
	profile_start(output_frame_name);
	output_frames();
	profile_end(output_frame_name);

	profile_start(render_displays_name);
	render_displays();
	profile_end(render_displays_name);
	source_profiler_render_end();

	execute_graphics_tasks();

	frame_time_ns = os_gettime_ns() - frame_start;

	source_profiler_frame_collect();
	profile_end(context->video_thread_name);

	profile_reenable_thread();

	video_sleep(&obs->video, &obs->video.video_time, context->interval);

	context->frame_time_total_ns += frame_time_ns;
	context->fps_total_ns += (obs->video.video_time - context->last_time);
	context->fps_total_frames++;

	if (context->fps_total_ns >= 1000000000ULL) {
		const double fps = (double)context->fps_total_frames / ((double)context->fps_total_ns / 1000000000.0);
		const uint64_t avg_frame_time_ns = context->frame_time_total_ns / (uint64_t)context->fps_total_frames;
		obs_video_stats_set_timing(&obs->video.stats, fps, avg_frame_time_ns);

		context->frame_time_total_ns = 0;
		context->fps_total_ns = 0;
		context->fps_total_frames = 0;
	}

	return !stop_requested();
}

void *obs_graphics_thread(void *param)
{
	obs_host_graphics_thread_begin();

	is_graphics_thread = true;

	const uint64_t interval = obs->video.video_frame_interval_ns;

	obs->video.video_time = os_gettime_ns();

	os_set_thread_name("libobs: graphics thread");

	const char *video_thread_name = profile_store_name(obs_get_profiler_name_store(),
							   "obs_graphics_thread(%g" NBSP "ms)", interval / 1000000.);
	profile_register_root(video_thread_name, interval);

	srand((unsigned int)time(NULL));

	struct obs_graphics_context context;
	context.interval = interval;
	context.frame_time_total_ns = 0;
	context.fps_total_ns = 0;
	context.fps_total_frames = 0;
	context.last_time = 0;
	context.video_thread_name = video_thread_name;

	while (obs_host_graphics_thread_loop(&context))
		;

	obs_host_graphics_thread_end();

	UNUSED_PARAMETER(param);
	return NULL;
}

/* File-local macros (BUILD.md): out of the next file in the unity group. */
#undef NBSP
