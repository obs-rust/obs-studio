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

#define get_weak(source) ((obs_weak_source_t *)source->context.control)

static inline bool data_valid(const struct obs_source *source, const char *f)
{
	return obs_source_valid(source, f) && source->context.data;
}

static inline bool deinterlacing_enabled(const struct obs_source *source)
{
	return source->deinterlace_mode != OBS_DEINTERLACE_MODE_DISABLE;
}

struct obs_source_info *get_source_info2(const char *unversioned_id, uint32_t ver)
{
	for (size_t i = 0; i < obs->source_types.num; i++) {
		struct obs_source_info *info = &obs->source_types.array[i];
		if (strcmp(info->unversioned_id, unversioned_id) == 0 && info->version == ver)
			return info;
	}

	return NULL;
}

static void allocate_audio_output_buffer(struct obs_source *source)
{
	size_t size = sizeof(float) * AUDIO_OUTPUT_FRAMES * MAX_AUDIO_CHANNELS * MAX_AUDIO_MIXES;
	float *ptr = bzalloc(size);

	for (size_t mix = 0; mix < MAX_AUDIO_MIXES; mix++) {
		size_t mix_pos = mix * AUDIO_OUTPUT_FRAMES * MAX_AUDIO_CHANNELS;

		for (size_t i = 0; i < MAX_AUDIO_CHANNELS; i++) {
			source->audio_output_buf[mix][i] = ptr + mix_pos + AUDIO_OUTPUT_FRAMES * i;
		}
	}
}

static void allocate_audio_mix_buffer(struct obs_source *source)
{
	size_t size = sizeof(float) * AUDIO_OUTPUT_FRAMES * MAX_AUDIO_CHANNELS;
	float *ptr = bzalloc(size);

	for (size_t i = 0; i < MAX_AUDIO_CHANNELS; i++) {
		source->audio_mix_buf[i] = ptr + AUDIO_OUTPUT_FRAMES * i;
	}
}

static inline bool is_audio_source(const struct obs_source *source)
{
	return source->info.output_flags & OBS_SOURCE_AUDIO;
}

static inline bool is_composite_source(const struct obs_source *source)
{
	return source->info.output_flags & OBS_SOURCE_COMPOSITE;
}

static inline bool requires_canvas(const struct obs_source *source)
{
	return source->info.output_flags & OBS_SOURCE_REQUIRES_CANVAS;
}

extern char *find_libobs_data_file(const char *file);

/* internal initialization */
static bool obs_source_init(struct obs_source *source)
{
	source->user_volume = 1.0f;
	source->volume = 1.0f;
	source->sync_offset = 0;
	source->balance = 0.5f;
	source->audio_active = true;
	pthread_mutex_init_value(&source->filter_mutex);
	pthread_mutex_init_value(&source->async_mutex);
	pthread_mutex_init_value(&source->audio_mutex);
	pthread_mutex_init_value(&source->audio_buf_mutex);
	pthread_mutex_init_value(&source->audio_cb_mutex);
	pthread_mutex_init_value(&source->caption_cb_mutex);
	pthread_mutex_init_value(&source->media_actions_mutex);

	if (pthread_mutex_init_recursive(&source->filter_mutex) != 0)
		return false;
	if (pthread_mutex_init(&source->audio_buf_mutex, NULL) != 0)
		return false;
	if (pthread_mutex_init(&source->audio_actions_mutex, NULL) != 0)
		return false;
	if (pthread_mutex_init(&source->audio_cb_mutex, NULL) != 0)
		return false;
	if (pthread_mutex_init(&source->audio_mutex, NULL) != 0)
		return false;
	if (pthread_mutex_init_recursive(&source->async_mutex) != 0)
		return false;
	if (pthread_mutex_init(&source->caption_cb_mutex, NULL) != 0)
		return false;
	if (pthread_mutex_init(&source->media_actions_mutex, NULL) != 0)
		return false;

	if (is_audio_source(source) || is_composite_source(source))
		allocate_audio_output_buffer(source);
	if (source->info.audio_mix)
		allocate_audio_mix_buffer(source);

	if (source->info.type == OBS_SOURCE_TYPE_TRANSITION) {
		if (!obs_transition_init(source))
			return false;
	}

	obs_context_init_control(&source->context, source, (obs_destroy_cb)obs_source_destroy);

	source->deinterlace_top_first = true;
	source->audio_mixers = 0xFF;

	source->private_settings = obs_data_create();
	return true;
}

static void obs_source_init_finalize(struct obs_source *source, obs_canvas_t *canvas)
{
	if (is_audio_source(source)) {
		pthread_mutex_lock(&obs->data.audio_sources_mutex);

		source->next_audio_source = obs->data.first_audio_source;
		source->prev_next_audio_source = &obs->data.first_audio_source;
		if (obs->data.first_audio_source)
			obs->data.first_audio_source->prev_next_audio_source = &source->next_audio_source;
		obs->data.first_audio_source = source;

		pthread_mutex_unlock(&obs->data.audio_sources_mutex);
	}

	if (!source->context.private) {
		if (requires_canvas(source)) {
			obs_canvas_insert_source(canvas, source);
		} else {
			obs_context_data_insert_name(&source->context, &obs->data.sources_mutex,
						     &obs->data.public_sources);
		}
	} else if (requires_canvas(source)) {
		source->canvas = obs_canvas_get_weak_canvas(canvas);
	}
	obs_context_data_insert_uuid(&source->context, &obs->data.sources_mutex, &obs->data.sources);
}

static void obs_source_init_audio_hotkeys(struct obs_source *source)
{
	if (!(source->info.output_flags & OBS_SOURCE_AUDIO) || source->info.type != OBS_SOURCE_TYPE_INPUT) {
		source->mute_unmute_key = OBS_INVALID_HOTKEY_PAIR_ID;
		source->monitor_on_off_key = OBS_INVALID_HOTKEY_PAIR_ID;
		source->push_to_mute_key = OBS_INVALID_HOTKEY_ID;
		source->push_to_talk_key = OBS_INVALID_HOTKEY_ID;
		return;
	}

	source->mute_unmute_key = obs_hotkey_pair_register_source(source, "libobs.mute", obs->hotkeys.mute,
								  "libobs.unmute", obs->hotkeys.unmute,
								  obs_source_hotkey_mute, obs_source_hotkey_unmute,
								  source, source);

	source->monitor_on_off_key = obs_hotkey_pair_register_source(
		source, "libobs.monitor-on", obs->hotkeys.monitor_on, "libobs.monitor-off", obs->hotkeys.monitor_off,
		obs_source_hotkey_monitor_on, obs_source_hotkey_monitor_off, source, source);

	source->push_to_mute_key = obs_hotkey_register_source(source, "libobs.push-to-mute", obs->hotkeys.push_to_mute,
							      obs_source_hotkey_push_to_mute, source);

	source->push_to_talk_key = obs_hotkey_register_source(source, "libobs.push-to-talk", obs->hotkeys.push_to_talk,
							      obs_source_hotkey_push_to_talk, source);
}

static obs_source_t *obs_source_create_internal(const char *id, const char *name, const char *uuid,
						obs_data_t *settings, obs_data_t *hotkey_data, bool private,
						uint32_t last_obs_ver, obs_canvas_t *canvas)
{
	struct obs_source *source = bzalloc(sizeof(struct obs_source));

	const struct obs_source_info *info = get_source_info(id);
	if (!info) {
		blog(LOG_ERROR, "Source ID '%s' not found", id);

		source->info.id = bstrdup(id);
		source->owns_info_id = true;
		source->info.unversioned_id = bstrdup(source->info.id);
	} else {
		source->info = *info;

		/* Always mark filters as private so they aren't found by
		 * source enum/search functions.
		 *
		 * XXX: Fix design flaws with filters */
		if (info->type == OBS_SOURCE_TYPE_FILTER)
			private = true;
	}

	source->mute_unmute_key = OBS_INVALID_HOTKEY_PAIR_ID;
	source->monitor_on_off_key = OBS_INVALID_HOTKEY_PAIR_ID;
	source->push_to_mute_key = OBS_INVALID_HOTKEY_ID;
	source->push_to_talk_key = OBS_INVALID_HOTKEY_ID;
	source->last_obs_ver = last_obs_ver;

	if (!obs_source_init_context(source, settings, name, uuid, hotkey_data, private))
		goto fail;

	if (info) {
		if (info->get_defaults) {
			info->get_defaults(source->context.settings);
		}
		if (info->get_defaults2) {
			info->get_defaults2(info->type_data, source->context.settings);
		}
	}

	if (!obs_source_init(source))
		goto fail;

	/* Scenes need canvases, fall back to using default canvas if none provided here. */
	if (requires_canvas(source) && !canvas) {
		blog(LOG_WARNING, "Attempted to add Scene without specifying a canvas! Using default canvas instead.");
		canvas = obs->data.main_canvas;
	}

	if (!private)
		obs_source_init_audio_hotkeys(source);

	/* A plugin may start capture threads from its create callback. */
	source->flags = source->default_flags;
	source->enabled = true;

	/* allow the source to be created even if creation fails so that the
	 * user's data doesn't become lost */
	if (info && info->create)
		source->context.data = info->create(source->context.settings, source);
	if ((!info || info->create) && !source->context.data)
		blog(LOG_ERROR, "Failed to create source '%s'!", name);

	blog(LOG_DEBUG, "%ssource '%s' (%s) created", private ? "private " : "", name, id);

	/* Preserve defaults set by create without racing capture threads when
	 * the defaults have not changed. */
	if (source->flags != source->default_flags)
		source->flags = source->default_flags;

	/* audio deduplication initialization */
	source->audio_is_duplicated = false;

	obs_source_init_finalize(source, canvas);
	if (!private) {
		if (canvas)
			obs_source_dosignal_canvas(source, canvas, "source_create_canvas", NULL);
		if (!canvas || canvas == obs->data.main_canvas)
			obs_source_dosignal(source, "source_create", NULL);
	}

	return source;

fail:
	blog(LOG_ERROR, "obs_source_create failed");
	obs_source_destroy(source);
	return NULL;
}

obs_source_t *obs_source_create(const char *id, const char *name, obs_data_t *settings, obs_data_t *hotkey_data)
{
	return obs_source_create_internal(id, name, NULL, settings, hotkey_data, false, LIBOBS_API_VER, NULL);
}

obs_source_t *obs_source_create_private(const char *id, const char *name, obs_data_t *settings)
{
	return obs_source_create_internal(id, name, NULL, settings, NULL, true, LIBOBS_API_VER, NULL);
}

obs_source_t *obs_source_create_canvas(obs_canvas_t *canvas, const char *id, const char *name, obs_data_t *settings,
				       obs_data_t *hotkey_data)
{
	return obs_source_create_internal(id, name, NULL, settings, hotkey_data, false, LIBOBS_API_VER, canvas);
}

obs_source_t *obs_source_create_private_with_canvas(obs_canvas_t *canvas, const char *id, const char *name,
						    obs_data_t *settings)
{
	return obs_source_create_internal(id, name, NULL, settings, NULL, true, LIBOBS_API_VER, canvas);
}

obs_source_t *obs_source_create_set_last_ver(obs_canvas_t *canvas, const char *id, const char *name, const char *uuid,
					     obs_data_t *settings, obs_data_t *hotkey_data, uint32_t last_obs_ver,
					     bool is_private)
{
	return obs_source_create_internal(id, name, uuid, settings, hotkey_data, is_private, last_obs_ver, canvas);
}

_Static_assert(OBS_SOURCE_DUPLICATE_TYPE_FILTER == OBS_SOURCE_TYPE_FILTER, "OBS_SOURCE_TYPE_FILTER");
_Static_assert(OBS_SOURCE_DUPLICATE_TYPE_SCENE == OBS_SOURCE_TYPE_SCENE, "OBS_SOURCE_TYPE_SCENE");
_Static_assert(OBS_SOURCE_DUPLICATE_DO_NOT_DUPLICATE == OBS_SOURCE_DO_NOT_DUPLICATE, "OBS_SOURCE_DO_NOT_DUPLICATE");
_Static_assert(OBS_SOURCE_DUPLICATE_DUP_REFS == OBS_SCENE_DUP_REFS, "OBS_SCENE_DUP_REFS");
_Static_assert(OBS_SOURCE_DUPLICATE_DUP_COPY == OBS_SCENE_DUP_COPY, "OBS_SCENE_DUP_COPY");
_Static_assert(OBS_SOURCE_DUPLICATE_DUP_PRIVATE_REFS == OBS_SCENE_DUP_PRIVATE_REFS, "OBS_SCENE_DUP_PRIVATE_REFS");
_Static_assert(OBS_SOURCE_DUPLICATE_DUP_PRIVATE_COPY == OBS_SCENE_DUP_PRIVATE_COPY, "OBS_SCENE_DUP_PRIVATE_COPY");

char *get_new_filter_name(obs_source_t *dst, const char *name)
{
	struct dstr new_name = {0};
	int inc = 0;

	dstr_copy(&new_name, name);

	for (;;) {
		obs_source_t *existing_filter = obs_source_get_filter_by_name(dst, new_name.array);
		if (!existing_filter)
			break;

		obs_source_release(existing_filter);

		dstr_printf(&new_name, "%s %d", name, obs_source_duplicate_filter_number(++inc));
	}

	return new_name.array;
}

static inline void obs_source_frame_decref(struct obs_source_frame *frame)
{
	if (os_atomic_dec_long(&frame->refs) == 0)
		obs_source_frame_destroy(frame);
}

static void obs_source_destroy_defer(struct obs_source *source);

void obs_source_destroy(struct obs_source *source)
{
	if (!obs_source_valid(source, "obs_source_destroy"))
		return;

	if (os_atomic_set_long(&source->destroying, true) == true) {
		blog(LOG_ERROR, "Double destroy just occurred. "
				"Something called addref on a source "
				"after it was already fully released, "
				"I guess.");
		return;
	}

	if (is_audio_source(source)) {
		pthread_mutex_lock(&source->audio_cb_mutex);
		da_free(source->audio_cb_list);
		pthread_mutex_unlock(&source->audio_cb_mutex);
	}

	pthread_mutex_lock(&source->caption_cb_mutex);
	da_free(source->caption_cb_list);
	pthread_mutex_unlock(&source->caption_cb_mutex);

	if (source->info.type == OBS_SOURCE_TYPE_TRANSITION)
		obs_transition_clear(source);

	pthread_mutex_lock(&obs->data.audio_sources_mutex);
	if (source->prev_next_audio_source) {
		*source->prev_next_audio_source = source->next_audio_source;
		if (source->next_audio_source)
			source->next_audio_source->prev_next_audio_source = source->prev_next_audio_source;
	}
	pthread_mutex_unlock(&obs->data.audio_sources_mutex);

	if (source->filter_parent)
		obs_source_filter_remove_refless(source->filter_parent, source);

	while (source->filters.num)
		obs_source_filter_remove(source, source->filters.array[0]);

	obs_context_data_remove_uuid(&source->context, &obs->data.sources_mutex, &obs->data.sources);
	if (!source->context.private) {
		if (requires_canvas(source)) {
			obs_canvas_remove_source(source);
		} else {
			obs_context_data_remove_name(&source->context, &obs->data.sources_mutex,
						     &obs->data.public_sources);
		}
	} else if (source->canvas) {
		obs_weak_canvas_release(source->canvas);
		source->canvas = NULL;
	}

	source_profiler_remove_source(source);

	/* defer source destroy */
	os_task_queue_queue_task(obs->destruction_task_thread, (os_task_t)obs_source_destroy_defer, source);
}

static void obs_source_destroy_defer(struct obs_source *source)
{
	size_t i;

	/* prevents the destruction of sources if destroy triggered inside of
	 * a video tick call */
	obs_context_wait(&source->context);

	obs_source_dosignal(source, "source_destroy", "destroy");

	if (source->context.data) {
		source->info.destroy(source->context.data);
		source->context.data = NULL;
	}

	blog(LOG_DEBUG, "%ssource '%s' destroyed", source->context.private ? "private " : "", source->context.name);

	audio_monitor_destroy(source->monitor);

	obs_hotkey_unregister(source->push_to_talk_key);
	obs_hotkey_unregister(source->push_to_mute_key);
	obs_hotkey_pair_unregister(source->mute_unmute_key);
	obs_hotkey_pair_unregister(source->monitor_on_off_key);

	for (i = 0; i < source->async_cache.num; i++)
		obs_source_frame_decref(source->async_cache.array[i].frame);

	gs_enter_context(obs->video.graphics);
	if (source->async_texrender)
		gs_texrender_destroy(source->async_texrender);
	if (source->async_prev_texrender)
		gs_texrender_destroy(source->async_prev_texrender);
	for (size_t c = 0; c < MAX_AV_PLANES; c++) {
		gs_texture_destroy(source->async_textures[c]);
		gs_texture_destroy(source->async_prev_textures[c]);
	}
	if (source->filter_texrender)
		gs_texrender_destroy(source->filter_texrender);
	if (source->color_space_texrender)
		gs_texrender_destroy(source->color_space_texrender);
	gs_leave_context();

	for (i = 0; i < MAX_AV_PLANES; i++)
		bfree(source->audio_data.data[i]);
	for (i = 0; i < MAX_AUDIO_CHANNELS; i++)
		deque_free(&source->audio_input_buf[i]);
	audio_resampler_destroy(source->resampler);
	bfree(source->audio_output_buf[0][0]);
	bfree(source->audio_mix_buf[0]);

	obs_source_frame_destroy(source->async_preload_frame);

	if (source->info.type == OBS_SOURCE_TYPE_TRANSITION)
		obs_transition_free(source);

	da_free(source->audio_actions);
	da_free(source->audio_cb_list);
	da_free(source->caption_cb_list);
	da_free(source->async_cache);
	da_free(source->async_frames);
	da_free(source->filters);
	da_free(source->media_actions);
	pthread_mutex_destroy(&source->filter_mutex);
	pthread_mutex_destroy(&source->audio_actions_mutex);
	pthread_mutex_destroy(&source->audio_buf_mutex);
	pthread_mutex_destroy(&source->audio_cb_mutex);
	pthread_mutex_destroy(&source->audio_mutex);
	pthread_mutex_destroy(&source->caption_cb_mutex);
	pthread_mutex_destroy(&source->async_mutex);
	pthread_mutex_destroy(&source->media_actions_mutex);
	obs_data_release(source->private_settings);
	obs_context_data_free(&source->context);

	if (source->owns_info_id) {
		bfree((void *)(uintptr_t)source->info.id);
		bfree((void *)(uintptr_t)source->info.unversioned_id);
	}

	bfree(source);
}

void obs_source_reset_settings(obs_source_t *source, obs_data_t *settings)
{
	if (!obs_source_valid(source, "obs_source_reset_settings"))
		return;

	obs_data_clear(source->context.settings);
	obs_source_update(source, settings);
}

static inline struct obs_source_frame *get_closest_frame(obs_source_t *source, uint64_t sys_time);

static void filter_frame(obs_source_t *source, struct obs_source_frame **ref_frame)
{
	struct obs_source_frame *frame = *ref_frame;
	if (frame) {
		os_atomic_inc_long(&frame->refs);
		frame = filter_async_video(source, frame);
		if (frame)
			os_atomic_dec_long(&frame->refs);
	}

	*ref_frame = frame;
}

_Static_assert(MEDIA_ACTION_NONE == OBS_SOURCE_MEDIA_ACTION_NONE, "MEDIA_ACTION_NONE");
_Static_assert(MEDIA_ACTION_PLAY_PAUSE == OBS_SOURCE_MEDIA_ACTION_PLAY_PAUSE, "MEDIA_ACTION_PLAY_PAUSE");
_Static_assert(MEDIA_ACTION_RESTART == OBS_SOURCE_MEDIA_ACTION_RESTART, "MEDIA_ACTION_RESTART");
_Static_assert(MEDIA_ACTION_STOP == OBS_SOURCE_MEDIA_ACTION_STOP, "MEDIA_ACTION_STOP");
_Static_assert(MEDIA_ACTION_NEXT == OBS_SOURCE_MEDIA_ACTION_NEXT, "MEDIA_ACTION_NEXT");
_Static_assert(MEDIA_ACTION_PREVIOUS == OBS_SOURCE_MEDIA_ACTION_PREVIOUS, "MEDIA_ACTION_PREVIOUS");
_Static_assert(MEDIA_ACTION_SET_TIME == OBS_SOURCE_MEDIA_ACTION_SET_TIME, "MEDIA_ACTION_SET_TIME");
_Static_assert(sizeof(enum media_action_type) == sizeof(int), "enum media_action_type is passed as int");

/* What each queued action calls and the signal it emits go through
 * obs-source-media-admission.c. The queue and its mutex, the callbacks and
 * the signals stay here, the callback before the signal as before;
 * action.pause is read only for a play/pause. */

void process_media_actions(obs_source_t *source);
void process_media_actions(obs_source_t *source)
{
	struct media_action action = {0};

	for (;;) {
		pthread_mutex_lock(&source->media_actions_mutex);
		if (source->media_actions.num) {
			action = source->media_actions.array[0];
			da_pop_front(source->media_actions);
		} else {
			action.type = MEDIA_ACTION_NONE;
		}
		pthread_mutex_unlock(&source->media_actions_mutex);

		enum obs_source_media_dispatch dispatch = obs_source_media_dispatch((int)action.type);

		switch (dispatch) {
		case OBS_SOURCE_MEDIA_DISPATCH_RETURN:
			return;
		case OBS_SOURCE_MEDIA_DISPATCH_PLAY_PAUSE:
			source->info.media_play_pause(source->context.data, action.pause);
			break;
		case OBS_SOURCE_MEDIA_DISPATCH_RESTART:
			source->info.media_restart(source->context.data);
			break;
		case OBS_SOURCE_MEDIA_DISPATCH_STOP:
			source->info.media_stop(source->context.data);
			break;
		case OBS_SOURCE_MEDIA_DISPATCH_NEXT:
			source->info.media_next(source->context.data);
			break;
		case OBS_SOURCE_MEDIA_DISPATCH_PREVIOUS:
			source->info.media_previous(source->context.data);
			break;
		case OBS_SOURCE_MEDIA_DISPATCH_SET_TIME:
			source->info.media_set_time(source->context.data, action.ms);
			break;
		case OBS_SOURCE_MEDIA_DISPATCH_SKIP:
			break;
		}

		const char *signal = obs_source_media_signal(dispatch, obs_source_media_signal_reads_pause(dispatch) &&
									       action.pause);
		if (signal)
			obs_source_dosignal(source, NULL, signal);
	}
}

static void async_tick(obs_source_t *source)
{
	uint64_t sys_time = obs_video_stats_frame_time(&obs->video.stats);

	pthread_mutex_lock(&source->async_mutex);

	if (obs_source_video_tick_async_route(deinterlacing_enabled(source)) ==
	    OBS_SOURCE_VIDEO_TICK_ASYNC_DEINTERLACE) {
		deinterlace_process_last_frame(source, sys_time);
	} else {
		if (source->cur_async_frame) {
			remove_async_frame(source, source->cur_async_frame);
			source->cur_async_frame = NULL;
		}

		source->cur_async_frame = get_closest_frame(source, sys_time);
	}

	source->last_sys_timestamp = sys_time;

	if (obs_source_video_tick_async_route(deinterlacing_enabled(source)) == OBS_SOURCE_VIDEO_TICK_ASYNC_DEINTERLACE)
		filter_frame(source, &source->prev_async_frame);
	filter_frame(source, &source->cur_async_frame);

	if (source->cur_async_frame)
		source->async_update_texture = set_async_texture_size(source, source->cur_async_frame);

	pthread_mutex_unlock(&source->async_mutex);
}

_Static_assert(OBS_SOURCE_VIDEO_TICK_TYPE_TRANSITION == OBS_SOURCE_TYPE_TRANSITION, "OBS_SOURCE_TYPE_TRANSITION");
_Static_assert(OBS_SOURCE_VIDEO_TICK_ASYNC == OBS_SOURCE_ASYNC, "OBS_SOURCE_ASYNC");
_Static_assert(OBS_SOURCE_VIDEO_TICK_CONTROLLABLE_MEDIA == OBS_SOURCE_CONTROLLABLE_MEDIA,
	       "OBS_SOURCE_CONTROLLABLE_MEDIA");

/* The decisions are in obs-source-video-tick.c. Each step asks at its own
 * point, so every field is read and every call made where it was: the
 * output flags are read again for the media step, after async_tick(), and
 * the activate refs are loaded only after the show edge has run. */
void obs_source_video_tick(obs_source_t *source, float seconds)
{
	bool now_showing, now_active, has_data, has_callback;
	enum obs_source_video_tick_edge edge;
	long refs;

	if (!obs_source_valid(source, "obs_source_video_tick"))
		return;

	if (obs_source_video_tick_transition((int)source->info.type))
		obs_transition_tick(source, seconds);

	if (obs_source_video_tick_async(source->info.output_flags))
		async_tick(source);

	if (obs_source_video_tick_media(source->info.output_flags))
		process_media_actions(source);

	if (obs_source_video_tick_deferred_update(os_atomic_load_long(&source->defer_update_count)))
		obs_source_deferred_update(source);

	/* reset the filter render texture information once every frame */
	if (source->filter_texrender)
		gs_texrender_reset(source->filter_texrender);

	/* call show/hide if the reference changed */
	refs = os_atomic_load_long(&source->show_refs);
	edge = obs_source_video_tick_edge(refs, source->showing);
	if (edge != OBS_SOURCE_VIDEO_TICK_EDGE_NONE) {
		now_showing = edge == OBS_SOURCE_VIDEO_TICK_EDGE_ON;
		if (now_showing) {
			show_source(source);
		} else {
			hide_source(source);
		}

		if (source->filters.num) {
			for (size_t i = source->filters.num; i > 0; i--) {
				obs_source_t *filter = source->filters.array[i - 1];
				if (now_showing) {
					show_source(filter);
				} else {
					hide_source(filter);
				}
			}
		}

		source->showing = now_showing;
	}

	/* call activate/deactivate if the reference changed */
	refs = os_atomic_load_long(&source->activate_refs);
	edge = obs_source_video_tick_edge(refs, source->active);
	if (edge != OBS_SOURCE_VIDEO_TICK_EDGE_NONE) {
		now_active = edge == OBS_SOURCE_VIDEO_TICK_EDGE_ON;
		if (now_active) {
			activate_source(source);
		} else {
			deactivate_source(source);
		}

		if (source->filters.num) {
			for (size_t i = source->filters.num; i > 0; i--) {
				obs_source_t *filter = source->filters.array[i - 1];
				if (now_active) {
					activate_source(filter);
				} else {
					deactivate_source(filter);
				}
			}
		}

		source->active = now_active;
	}

	/* info.video_tick is set when the source is created, so reading it
	 * without data reads the same value it always has */
	has_data = source->context.data != NULL;
	has_callback = source->info.video_tick != NULL;
	if (obs_source_video_tick_calls_tick(has_data, has_callback))
		source->info.video_tick(source->context.data, seconds);

	source->async_rendered = false;
	source->deinterlace_rendered = false;
}

/* maximum buffer size */
#define MAX_BUF_SIZE (1000 * AUDIO_OUTPUT_FRAMES * sizeof(float))

/* time threshold in nanoseconds to ensure audio timing is as seamless as
 * possible */
#define TS_SMOOTHING_THRESHOLD 70000000ULL

/* obs-source-audio-timing.c decides with the same thresholds */
_Static_assert(OBS_SOURCE_AUDIO_TS_SMOOTHING_THRESHOLD == TS_SMOOTHING_THRESHOLD, "TS_SMOOTHING_THRESHOLD");
_Static_assert(OBS_SOURCE_AUDIO_MAX_TS_VAR == MAX_TS_VAR, "MAX_TS_VAR");

void source_signal_audio_data(obs_source_t *source, const struct audio_data *in, bool muted)
{
	pthread_mutex_lock(&source->audio_cb_mutex);

	for (size_t i = source->audio_cb_list.num; i > 0; i--) {
		struct audio_cb_info info = source->audio_cb_list.array[i - 1];
		info.callback(info.param, source, in, muted);
	}

	pthread_mutex_unlock(&source->audio_cb_mutex);
}

/* The placement decisions are obs-source-audio-place.c's, on values: each
 * read of the source and each call stays here, in its order. The timestamps
 * are read before get_buf_placement() asks for the sample rate, as they were
 * when the difference was its argument, and audio_ts again after
 * reset_audio_data() has set it. */
_Static_assert(OBS_SOURCE_AUDIO_PLACE_FRAMES == AUDIO_OUTPUT_FRAMES, "AUDIO_OUTPUT_FRAMES");
_Static_assert(OBS_SOURCE_AUDIO_PLACE_MAX_BUF_SIZE == MAX_BUF_SIZE, "MAX_BUF_SIZE");
_Static_assert(OBS_SOURCE_AUDIO_PLACE_MONITOR_ONLY == OBS_MONITORING_TYPE_MONITOR_ONLY, "MONITOR_ONLY");
_Static_assert(sizeof(enum obs_monitoring_type) == sizeof(int), "enum obs_monitoring_type is passed as int");

/* The conversion plan is obs-source-conversion-plan.c's: it gets the frame's
 * format, range, transfer and size, and the stores into the source happen
 * here. */
_Static_assert(sizeof(enum video_format) == sizeof(int), "enum video_format is passed as int");
_Static_assert(sizeof(enum gs_color_format) == sizeof(int), "enum gs_color_format is passed as int");
_Static_assert(OBS_SOURCE_CONVERSION_PLAN_PLANES <= MAX_AV_PLANES, "the plan's planes fit the source's");

enum convert_type {
	CONVERT_NONE = OBS_SOURCE_CONVERT_NONE,
	CONVERT_NV12 = OBS_SOURCE_CONVERT_NV12,
	CONVERT_420 = OBS_SOURCE_CONVERT_420,
	CONVERT_420_PQ = OBS_SOURCE_CONVERT_420_PQ,
	CONVERT_420_A = OBS_SOURCE_CONVERT_420_A,
	CONVERT_422 = OBS_SOURCE_CONVERT_422,
	CONVERT_422P10LE = OBS_SOURCE_CONVERT_422P10LE,
	CONVERT_422_A = OBS_SOURCE_CONVERT_422_A,
	CONVERT_422_PACK = OBS_SOURCE_CONVERT_422_PACK,
	CONVERT_444 = OBS_SOURCE_CONVERT_444,
	CONVERT_444P12LE = OBS_SOURCE_CONVERT_444P12LE,
	CONVERT_444_A = OBS_SOURCE_CONVERT_444_A,
	CONVERT_444P12LE_A = OBS_SOURCE_CONVERT_444P12LE_A,
	CONVERT_444_A_PACK = OBS_SOURCE_CONVERT_444_A_PACK,
	CONVERT_800 = OBS_SOURCE_CONVERT_800,
	CONVERT_RGB_LIMITED = OBS_SOURCE_CONVERT_RGB_LIMITED,
	CONVERT_BGR3 = OBS_SOURCE_CONVERT_BGR3,
	CONVERT_I010 = OBS_SOURCE_CONVERT_I010,
	CONVERT_P010 = OBS_SOURCE_CONVERT_P010,
	CONVERT_V210 = OBS_SOURCE_CONVERT_V210,
	CONVERT_R10L = OBS_SOURCE_CONVERT_R10L,
};

static inline enum convert_type get_convert_type(enum video_format format, bool full_range, uint8_t trc)
{
	struct obs_source_conversion_plan plan;
	obs_source_conversion_plan_fill((int)format, full_range, trc, 0, 0, &plan);
	return (enum convert_type)plan.convert_type;
}

static inline bool init_gpu_conversion(struct obs_source *source, const struct obs_source_frame *frame)
{
	struct obs_source_conversion_plan plan;
	obs_source_conversion_plan_fill((int)frame->format, frame->full_range, frame->trc, frame->width, frame->height,
					&plan);

	if (plan.convert_type == CONVERT_NONE) {
		assert(false && "No conversion requested");
		return false;
	}

	for (int c = 0; c < plan.channel_count; c++)
		source->async_convert_width[c] = plan.width[c];
	for (int c = 0; c < plan.channel_count; c++)
		source->async_convert_height[c] = plan.height[c];
	for (int c = 0; c < plan.channel_count; c++)
		source->async_texture_formats[c] = (enum gs_color_format)plan.format[c];
	source->async_channel_count = plan.channel_count;
	return true;
}

/* The upload decisions and shader parameter values are
 * obs-source-async-upload.c's: the fields, the levels and every gs_* call
 * stay here, read and made where they were. */
bool set_async_texture_size(struct obs_source *source, const struct obs_source_frame *frame)
{
	enum convert_type cur = get_convert_type(frame->format, frame->full_range, frame->trc);

	const struct obs_source_async_upload_format cur_format = {
		.width = source->async_width,
		.height = source->async_height,
		.format = (int)source->async_format,
		.full_range = source->async_full_range,
		.trc = source->async_trc,
	};
	const struct obs_source_async_upload_format frame_format = {
		.width = frame->width,
		.height = frame->height,
		.format = (int)frame->format,
		.full_range = frame->full_range,
		.trc = frame->trc,
	};
	if (!obs_source_async_upload_texture_changed(cur_format, frame_format))
		return true;

	source->async_width = frame->width;
	source->async_height = frame->height;
	source->async_format = frame->format;
	source->async_full_range = frame->full_range;
	source->async_trc = frame->trc;

	gs_enter_context(obs->video.graphics);

	for (size_t c = 0; c < MAX_AV_PLANES; c++) {
		gs_texture_destroy(source->async_textures[c]);
		source->async_textures[c] = NULL;
		gs_texture_destroy(source->async_prev_textures[c]);
		source->async_prev_textures[c] = NULL;
	}

	gs_texrender_destroy(source->async_texrender);
	gs_texrender_destroy(source->async_prev_texrender);
	source->async_texrender = NULL;
	source->async_prev_texrender = NULL;

	const enum gs_color_format format = convert_video_format(frame->format, frame->trc);
	const bool async_gpu_conversion = obs_source_async_upload_tries_gpu((int)cur) &&
					  init_gpu_conversion(source, frame);
	source->async_gpu_conversion = async_gpu_conversion;
	if (async_gpu_conversion) {
		source->async_texrender = gs_texrender_create(format, GS_ZS_NONE);

		for (int c = 0; c < source->async_channel_count; ++c)
			source->async_textures[c] =
				gs_texture_create(source->async_convert_width[c], source->async_convert_height[c],
						  source->async_texture_formats[c], 1, NULL, GS_DYNAMIC);
	} else {
		source->async_textures[0] = gs_texture_create(frame->width, frame->height, format, 1, NULL, GS_DYNAMIC);
	}

	if (deinterlacing_enabled(source))
		set_deinterlace_texture_size(source);

	gs_leave_context();

	return source->async_textures[0] != NULL;
}

static void upload_raw_frame(gs_texture_t *tex[MAX_AV_PLANES], const struct obs_source_frame *frame)
{
	switch (get_convert_type(frame->format, frame->full_range, frame->trc)) {
	case CONVERT_422_PACK:
	case CONVERT_800:
	case CONVERT_RGB_LIMITED:
	case CONVERT_BGR3:
	case CONVERT_420:
	case CONVERT_420_PQ:
	case CONVERT_422:
	case CONVERT_422P10LE:
	case CONVERT_NV12:
	case CONVERT_444:
	case CONVERT_444P12LE:
	case CONVERT_420_A:
	case CONVERT_422_A:
	case CONVERT_444_A:
	case CONVERT_444P12LE_A:
	case CONVERT_444_A_PACK:
	case CONVERT_I010:
	case CONVERT_P010:
	case CONVERT_V210:
	case CONVERT_R10L:
		for (size_t c = 0; c < MAX_AV_PLANES; c++) {
			if (tex[c])
				gs_texture_set_image(tex[c], frame->data[c], frame->linesize[c], false);
		}
		break;

	case CONVERT_NONE:
		assert(false && "No conversion requested");
		break;
	}
}

static const char *select_conversion_technique(enum video_format format, bool full_range, uint8_t trc)
{
	struct obs_source_conversion_plan plan;
	obs_source_conversion_plan_fill((int)format, full_range, trc, 0, 0, &plan);

	switch (format) {
	case VIDEO_FORMAT_BGRA:
	case VIDEO_FORMAT_BGRX:
	case VIDEO_FORMAT_RGBA:
	case VIDEO_FORMAT_NONE:
		if (full_range)
			assert(false && "No conversion requested");
		break;
	default:
		break;
	}
	return plan.technique;
}

static bool need_linear_output(enum video_format format)
{
	struct obs_source_conversion_plan plan;
	obs_source_conversion_plan_fill((int)format, false, 0, 0, 0, &plan);
	return plan.linear;
}

static inline void set_eparam(gs_effect_t *effect, const char *name, float val)
{
	gs_eparam_t *param = gs_effect_get_param_by_name(effect, name);
	gs_effect_set_float(param, val);
}

static bool update_async_texrender(struct obs_source *source, const struct obs_source_frame *frame,
				   gs_texture_t *tex[MAX_AV_PLANES], gs_texrender_t *texrender)
{
	GS_DEBUG_MARKER_BEGIN(GS_DEBUG_COLOR_CONVERT_FORMAT, "Convert Format");

	gs_texrender_reset(texrender);

	upload_raw_frame(tex, frame);

	uint32_t cx = source->async_width;
	uint32_t cy = source->async_height;

	const char *tech_name = select_conversion_technique(frame->format, frame->full_range, frame->trc);
	gs_effect_t *conv = obs->video.conversion_effect;
	gs_technique_t *tech = gs_effect_get_technique(conv, tech_name);
	const bool linear = need_linear_output(frame->format);

	const bool success = gs_texrender_begin(texrender, cx, cy);

	if (success) {
		const bool previous = gs_framebuffer_srgb_enabled();
		gs_enable_framebuffer_srgb(linear);

		gs_enable_blending(false);

		gs_technique_begin(tech);
		gs_technique_begin_pass(tech, 0);

		if (tex[0])
			gs_effect_set_texture(gs_effect_get_param_by_name(conv, "image"), tex[0]);
		if (tex[1])
			gs_effect_set_texture(gs_effect_get_param_by_name(conv, "image1"), tex[1]);
		if (tex[2])
			gs_effect_set_texture(gs_effect_get_param_by_name(conv, "image2"), tex[2]);
		if (tex[3])
			gs_effect_set_texture(gs_effect_get_param_by_name(conv, "image3"), tex[3]);
		const struct obs_source_async_upload_size size = obs_source_async_upload_size_uniforms(cx, cy);
		set_eparam(conv, "width", size.width);
		set_eparam(conv, "height", size.height);
		set_eparam(conv, "width_d2", size.width_d2);
		set_eparam(conv, "height_d2", size.height_d2);
		set_eparam(conv, "width_x2_i", size.width_x2_i);
		set_eparam(conv, "height_x2_i", size.height_x2_i);

		/* BT.2408 says higher than 1000 isn't comfortable */
		const float hlg_peak_level = obs_source_async_upload_hlg_peak(obs->video.hdr_nominal_peak_level);

		const float maximum_nits = obs_source_async_upload_maximum_nits(frame->trc, hlg_peak_level);
		set_eparam(conv, "maximum_over_sdr_white_nits",
			   obs_source_async_upload_over_sdr_white(maximum_nits, obs_get_video_sdr_white_level()));
		const float hlg_exponent = 0.2f + (0.42f * log10f(hlg_peak_level / 1000.f));
		set_eparam(conv, "hlg_exponent", hlg_exponent);
		set_eparam(conv, "hdr_lw", obs_source_async_upload_hdr_lw(frame->max_luminance));
		set_eparam(conv, "hdr_lmax", obs_get_video_hdr_nominal_peak_level());

		struct vec4 vec0, vec1, vec2;
		vec4_set(&vec0, frame->color_matrix[0], frame->color_matrix[1], frame->color_matrix[2],
			 frame->color_matrix[3]);
		vec4_set(&vec1, frame->color_matrix[4], frame->color_matrix[5], frame->color_matrix[6],
			 frame->color_matrix[7]);
		vec4_set(&vec2, frame->color_matrix[8], frame->color_matrix[9], frame->color_matrix[10],
			 frame->color_matrix[11]);
		gs_effect_set_vec4(gs_effect_get_param_by_name(conv, "color_vec0"), &vec0);
		gs_effect_set_vec4(gs_effect_get_param_by_name(conv, "color_vec1"), &vec1);
		gs_effect_set_vec4(gs_effect_get_param_by_name(conv, "color_vec2"), &vec2);
		if (obs_source_async_upload_sets_range(frame->full_range)) {
			gs_eparam_t *min_param = gs_effect_get_param_by_name(conv, "color_range_min");
			gs_effect_set_val(min_param, frame->color_range_min, sizeof(float) * 3);
			gs_eparam_t *max_param = gs_effect_get_param_by_name(conv, "color_range_max");
			gs_effect_set_val(max_param, frame->color_range_max, sizeof(float) * 3);
		}

		gs_draw(GS_TRIS, 0, 3);

		gs_technique_end_pass(tech);
		gs_technique_end(tech);

		gs_enable_blending(true);

		gs_enable_framebuffer_srgb(previous);

		gs_texrender_end(texrender);
	}

	GS_DEBUG_MARKER_END();
	return success;
}

bool update_async_texture(struct obs_source *source, const struct obs_source_frame *frame, gs_texture_t *tex,
			  gs_texrender_t *texrender)
{
	gs_texture_t *tex3[MAX_AV_PLANES] = {tex, NULL, NULL, NULL, NULL, NULL, NULL, NULL};
	return update_async_textures(source, frame, tex3, texrender);
}

bool update_async_textures(struct obs_source *source, const struct obs_source_frame *frame,
			   gs_texture_t *tex[MAX_AV_PLANES], gs_texrender_t *texrender)
{
	enum convert_type type;

	source->async_flip = frame->flip;
	source->async_linear_alpha = obs_source_async_upload_linear_alpha(frame->flags);

	if (obs_source_async_upload_uses_texrender(source->async_gpu_conversion, texrender != NULL))
		return update_async_texrender(source, frame, tex, texrender);

	type = get_convert_type(frame->format, frame->full_range, frame->trc);
	if (obs_source_async_upload_plain((int)type)) {
		gs_texture_set_image(tex[0], frame->data[0], frame->linesize[0], false);
		return true;
	}

	return false;
}

static inline void obs_source_draw_texture(struct obs_source *source, gs_effect_t *effect)
{
	gs_texture_t *tex = source->async_textures[0];
	gs_eparam_t *param;

	if (source->async_texrender)
		tex = gs_texrender_get_texture(source->async_texrender);

	if (!tex)
		return;

	param = gs_effect_get_param_by_name(effect, "image");

	const bool linear_srgb = gs_get_linear_srgb();

	const bool previous = gs_framebuffer_srgb_enabled();
	gs_enable_framebuffer_srgb(linear_srgb);

	if (linear_srgb) {
		gs_effect_set_texture_srgb(param, tex);
	} else {
		gs_effect_set_texture(param, tex);
	}

	gs_draw_sprite(tex, source->async_flip ? GS_FLIP_V : 0, 0, 0);

	gs_enable_framebuffer_srgb(previous);
}

static void recreate_async_texture(obs_source_t *source, enum gs_color_format format)
{
	uint32_t cx = gs_texture_get_width(source->async_textures[0]);
	uint32_t cy = gs_texture_get_height(source->async_textures[0]);
	gs_texture_destroy(source->async_textures[0]);
	source->async_textures[0] = gs_texture_create(cx, cy, format, 1, NULL, GS_DYNAMIC);
}

static inline void check_to_swap_bgrx_bgra(obs_source_t *source, struct obs_source_frame *frame)
{
	enum gs_color_format format = gs_texture_get_color_format(source->async_textures[0]);
	const int swap = obs_source_async_upload_bgr_swap((int)format, (int)frame->format);
	if (swap != GS_UNKNOWN)
		recreate_async_texture(source, (enum gs_color_format)swap);
}

static void obs_source_update_async_video(obs_source_t *source)
{
	if (!source->async_rendered) {
		source->async_rendered = true;

		struct obs_source_frame *frame = obs_source_get_frame(source);
		if (frame) {
			check_to_swap_bgrx_bgra(source, frame);

			/* async_unbuffered is set without a lock: read it only
			 * where it was, for a decoupled source */
			const bool decoupled = source->async_decoupled;
			const bool unbuffered = obs_source_async_upload_reanchor_reads_unbuffered(decoupled) &&
						source->async_unbuffered;
			if (obs_source_async_upload_reanchor(decoupled, unbuffered)) {
				pthread_mutex_lock(&source->audio_buf_mutex);
				source->timing_adjust =
					obs_video_stats_frame_time(&obs->video.stats) - frame->timestamp;
				source->timing_set = true;
				pthread_mutex_unlock(&source->audio_buf_mutex);
			}

			if (source->async_update_texture) {
				update_async_textures(source, frame, source->async_textures, source->async_texrender);
				source->async_update_texture = false;
				os_atomic_set_bool(&source->async_textures_stale, false);
			}

			source->async_last_rendered_ts = frame->timestamp;
			obs_source_release_frame(source, frame);
		}
	}
}

static void rotate_async_video(obs_source_t *source, long rotation)
{
	const float x = obs_source_draw_plan_rotation_x(rotation, source->async_width, source->async_height);
	const float y = obs_source_draw_plan_rotation_y(rotation, source->async_width, source->async_height);

	gs_matrix_translate3f(x, y, 0);
	gs_matrix_rotaa4f(0.0f, 0.0f, -1.0f, RAD((float)rotation));
}

static inline void obs_source_render_async_video(obs_source_t *source)
{
	if (source->async_textures[0] && os_atomic_load_bool(&source->async_active) &&
	    !os_atomic_load_bool(&source->async_textures_stale)) {
		gs_timer_t *timer = NULL;
		const uint64_t start = source_profiler_source_render_begin(&timer);

		const enum gs_color_space source_space = convert_video_space(source->async_format, source->async_trc);

		gs_effect_t *const effect = obs_get_base_effect(OBS_EFFECT_DEFAULT);
		const enum gs_color_space current_space = gs_get_color_space();
		const bool linear_srgb = obs_source_draw_plan_async_linear_srgb((int)source_space, (int)current_space,
										gs_get_linear_srgb());
		const bool nonlinear_alpha = obs_source_draw_plan_async_nonlinear_alpha((int)source_space, linear_srgb,
											source->async_linear_alpha);
		const char *tech_name = obs_source_draw_technique_name(
			obs_source_draw_plan_technique((int)source_space, (int)current_space, nonlinear_alpha));
		const float multiplier = obs_source_draw_plan_reads_white_level((int)source_space, (int)current_space)
						 ? obs_source_draw_plan_multiplier((int)source_space,
										   (int)current_space,
										   obs_get_video_sdr_white_level())
						 : 1.0f;

		const bool previous = gs_set_linear_srgb(linear_srgb);

		gs_technique_t *const tech = gs_effect_get_technique(effect, tech_name);
		gs_effect_set_float(gs_effect_get_param_by_name(effect, "multiplier"), multiplier);
		gs_technique_begin(tech);
		gs_technique_begin_pass(tech, 0);

		long rotation = source->async_rotation;
		if (rotation) {
			gs_matrix_push();
			rotate_async_video(source, rotation);
		}

		if (nonlinear_alpha) {
			gs_blend_state_push();
			gs_blend_function(GS_BLEND_ONE, GS_BLEND_INVSRCALPHA);
		}

		obs_source_draw_texture(source, effect);

		if (nonlinear_alpha) {
			gs_blend_state_pop();
		}

		if (rotation) {
			gs_matrix_pop();
		}

		gs_technique_end_pass(tech);
		gs_technique_end(tech);

		gs_set_linear_srgb(previous);

		source_profiler_source_render_end(source, start, timer);
	}
}

static inline void obs_source_render_filters(obs_source_t *source)
{
	obs_source_t *first_filter;

	pthread_mutex_lock(&source->filter_mutex);
	first_filter = obs_source_get_ref(source->filters.array[0]);
	pthread_mutex_unlock(&source->filter_mutex);

	source->rendering_filter = true;
	obs_source_video_render(first_filter);
	source->rendering_filter = false;

	obs_source_release(first_filter);
}

_Static_assert(OBS_SOURCE_SIZE_TYPE_FILTER == OBS_SOURCE_TYPE_FILTER, "OBS_SOURCE_TYPE_FILTER");
_Static_assert(OBS_SOURCE_SIZE_TYPE_TRANSITION == OBS_SOURCE_TYPE_TRANSITION, "OBS_SOURCE_TYPE_TRANSITION");
_Static_assert(sizeof(enum obs_source_type) == sizeof(int), "enum obs_source_type is passed as int");

static void source_render(obs_source_t *source, gs_effect_t *effect)
{
	gs_timer_t *timer = NULL;
	const uint64_t start = source_profiler_source_render_begin(&timer);

	void *const data = source->context.data;
	const enum gs_color_space current_space = gs_get_color_space();
	const enum gs_color_space source_space = obs_source_get_color_space(source, 1, &current_space);

	enum gs_color_format format = gs_get_format_from_space(source_space);
	const bool convert = obs_source_draw_plan_convert((int)source_space, (int)current_space);
	const char *convert_tech = obs_source_draw_technique_name(
		obs_source_draw_plan_technique((int)source_space, (int)current_space, false));
	const float multiplier = obs_source_draw_plan_reads_white_level((int)source_space, (int)current_space)
					 ? obs_source_draw_plan_multiplier((int)source_space, (int)current_space,
									   obs_get_video_sdr_white_level())
					 : 1.0f;

	if (convert) {
		if (source->color_space_texrender) {
			if (gs_texrender_get_format(source->color_space_texrender) != format) {
				gs_texrender_destroy(source->color_space_texrender);
				source->color_space_texrender = NULL;
			}
		}

		if (!source->color_space_texrender) {
			source->color_space_texrender = gs_texrender_create(format, GS_ZS_NONE);
		}

		gs_texrender_reset(source->color_space_texrender);
		const uint32_t cx = obs_source_base_width(source);
		const uint32_t cy = obs_source_base_height(source);
		if (gs_texrender_begin_with_color_space(source->color_space_texrender, cx, cy, source_space)) {
			gs_enable_blending(false);

			struct vec4 clear_color;
			vec4_zero(&clear_color);
			gs_clear(GS_CLEAR_COLOR, &clear_color, 0.0f, 0);
			gs_ortho(0.0f, (float)cx, 0.0f, (float)cy, -100.0f, 100.0f);

			source->info.video_render(data, effect);

			gs_enable_blending(true);

			gs_texrender_end(source->color_space_texrender);

			gs_effect_t *default_effect = obs->video.default_effect;
			gs_technique_t *tech = gs_effect_get_technique(default_effect, convert_tech);

			const bool previous = gs_framebuffer_srgb_enabled();
			gs_enable_framebuffer_srgb(true);

			gs_texture_t *const tex = gs_texrender_get_texture(source->color_space_texrender);
			gs_effect_set_texture_srgb(gs_effect_get_param_by_name(default_effect, "image"), tex);
			gs_effect_set_float(gs_effect_get_param_by_name(default_effect, "multiplier"), multiplier);

			gs_blend_state_push();
			gs_blend_function(GS_BLEND_ONE, GS_BLEND_INVSRCALPHA);

			const size_t passes = gs_technique_begin(tech);
			for (size_t i = 0; i < passes; i++) {
				gs_technique_begin_pass(tech, i);
				gs_draw_sprite(tex, 0, 0, 0);
				gs_technique_end_pass(tech);
			}
			gs_technique_end(tech);

			gs_blend_state_pop();

			gs_enable_framebuffer_srgb(previous);
		}
	} else {
		source->info.video_render(data, effect);
	}
	source_profiler_source_render_end(source, start, timer);
}

void obs_source_default_render(obs_source_t *source)
{
	if (source->context.data) {
		gs_effect_t *effect = obs->video.default_effect;
		gs_technique_t *tech = gs_effect_get_technique(effect, "Draw");
		size_t passes, i;

		passes = gs_technique_begin(tech);
		for (i = 0; i < passes; i++) {
			gs_technique_begin_pass(tech, i);
			source_render(source, effect);
			gs_technique_end_pass(tech);
		}
		gs_technique_end(tech);
	}
}

_Static_assert(sizeof(enum obs_source_type) == sizeof(int), "enum obs_source_type is passed as int");
_Static_assert(sizeof(enum obs_allow_direct_render) == sizeof(int), "enum obs_allow_direct_render is passed as int");
_Static_assert(sizeof(enum gs_color_space) == sizeof(int), "enum gs_color_space is passed as int");
_Static_assert(sizeof(enum gs_color_format) == sizeof(int), "enum gs_color_format is passed as int");

static inline void obs_source_main_render(obs_source_t *source)
{
	uint32_t flags = source->info.output_flags;
	bool custom_draw = obs_source_render_route_main_custom_draw(flags);
	bool srgb_aware = obs_source_render_route_main_srgb_aware(flags);
	const bool has_filter_parent = source->filter_parent != NULL;
	bool default_effect = obs_source_render_route_main_default_effect(
		flags, has_filter_parent,
		obs_source_render_route_main_reads_filters(has_filter_parent) && source->filters.num != 0);
	bool previous_srgb = false;

	if (!srgb_aware) {
		previous_srgb = gs_get_linear_srgb();
		gs_set_linear_srgb(false);
	}

	if (default_effect) {
		obs_source_default_render(source);
	} else if (source->context.data) {
		source_render(source, custom_draw ? NULL : gs_get_effect());
	}

	if (!srgb_aware)
		gs_set_linear_srgb(previous_srgb);
}

#if GS_USE_DEBUG_MARKERS
static const char *get_type_format(enum obs_source_type type)
{
	switch (type) {
	case OBS_SOURCE_TYPE_INPUT:
		return "Input: %s";
	case OBS_SOURCE_TYPE_FILTER:
		return "Filter: %s";
	case OBS_SOURCE_TYPE_TRANSITION:
		return "Transition: %s";
	case OBS_SOURCE_TYPE_SCENE:
		return "Scene: %s";
	default:
		return "[Unknown]: %s";
	}
}
#endif

/* The draw of source_render_video(): each READ_ step reads one value, in the
 * old order, and only when the route asks for it. */
static inline enum obs_source_render_route_draw source_draw_route(obs_source_t *source)
{
	enum obs_source_render_route_draw route = obs_source_render_route_draw(source->filters.num != 0);

	if (route == OBS_SOURCE_RENDER_ROUTE_DRAW_READ_RENDERING_FILTER)
		route = obs_source_render_route_draw_next(route, source->rendering_filter);
	if (route == OBS_SOURCE_RENDER_ROUTE_DRAW_READ_VIDEO_RENDER)
		route = obs_source_render_route_draw_next(route, source->info.video_render != NULL);
	if (route == OBS_SOURCE_RENDER_ROUTE_DRAW_READ_FILTER_TARGET)
		route = obs_source_render_route_draw_next(route, source->filter_target != NULL);
	if (route == OBS_SOURCE_RENDER_ROUTE_DRAW_READ_DEINTERLACING)
		route = obs_source_render_route_draw_next(route, deinterlacing_enabled(source));

	return route;
}

static inline void source_render_video(obs_source_t *source)
{
	int type = (int)source->info.type;
	uint32_t flags = obs_source_render_route_video_reads_flags(type) ? source->info.output_flags : 0;

	if (obs_source_render_route_video_none(type, flags)) {
		if (source->filter_parent)
			obs_source_skip_video_filter(source);
		return;
	}

	/* the async check reads the type and the flags again, as before */
	type = (int)source->info.type;
	flags = obs_source_render_route_async_reads_flags(type) ? source->info.output_flags : 0;
	if (obs_source_render_route_async_update(type, flags,
						 obs_source_render_route_async_reads_rendering_filter(type, flags) &&
							 source->rendering_filter)) {
		if (deinterlacing_enabled(source))
			deinterlace_update_async_video(source);
		obs_source_update_async_video(source);
	}

	const bool has_data = source->context.data != NULL;
	if (obs_source_render_route_inactive(has_data,
					     obs_source_render_route_reads_enabled(has_data) && source->enabled)) {
		if (source->filter_parent)
			obs_source_skip_video_filter(source);
		return;
	}

	GS_DEBUG_MARKER_BEGIN_FORMAT(GS_DEBUG_COLOR_SOURCE, get_type_format(source->info.type),
				     obs_source_get_name(source));

	switch (source_draw_route(source)) {
	case OBS_SOURCE_RENDER_ROUTE_DRAW_FILTERS:
		obs_source_render_filters(source);
		break;
	case OBS_SOURCE_RENDER_ROUTE_DRAW_MAIN:
		obs_source_main_render(source);
		break;
	case OBS_SOURCE_RENDER_ROUTE_DRAW_TARGET:
		obs_source_video_render(source->filter_target);
		break;
	case OBS_SOURCE_RENDER_ROUTE_DRAW_DEINTERLACE:
		deinterlace_render(source);
		break;
	case OBS_SOURCE_RENDER_ROUTE_DRAW_ASYNC:
		obs_source_render_async_video(source);
		break;
	case OBS_SOURCE_RENDER_ROUTE_DRAW_DEFAULT:
	case OBS_SOURCE_RENDER_ROUTE_DRAW_READ_RENDERING_FILTER:
	case OBS_SOURCE_RENDER_ROUTE_DRAW_READ_VIDEO_RENDER:
	case OBS_SOURCE_RENDER_ROUTE_DRAW_READ_FILTER_TARGET:
	case OBS_SOURCE_RENDER_ROUTE_DRAW_READ_DEINTERLACING:
	case OBS_SOURCE_RENDER_ROUTE_DRAW_READ_TARGET_VIDEO_RENDER:
	case OBS_SOURCE_RENDER_ROUTE_DRAW_READ_TARGET_DEINTERLACING:
		/* not a draw of source_render_video() */
		break;
	}

	GS_DEBUG_MARKER_END();
}

void obs_source_video_render(obs_source_t *source)
{
	if (!obs_source_valid(source, "obs_source_video_render"))
		return;

	source = obs_source_get_ref(source);
	if (source) {
		source_render_video(source);
		obs_source_release(source);
	}
}

/* Where obs_source_get_width() and obs_source_get_height() start a source
 * that is not a filter. The caller holds its filter_mutex. */
static const obs_source_t *size_start(const obs_source_t *source)
{
	switch (obs_source_size_entry((int)source->info.type, source->filters.num != 0)) {
	case OBS_SOURCE_SIZE_ENTRY_FIRST_FILTER:
		return source->filters.array[0];
	case OBS_SOURCE_SIZE_ENTRY_SELF:
		break;
	}

	return source;
}

static uint32_t get_recurse_width(obs_source_t *source)
{
	uint32_t width;

	pthread_mutex_lock(&source->filter_mutex);

	width = obs_source_base_width(size_start(source));

	pthread_mutex_unlock(&source->filter_mutex);

	return width;
}

static uint32_t get_recurse_height(obs_source_t *source)
{
	uint32_t height;

	pthread_mutex_lock(&source->filter_mutex);

	height = obs_source_base_height(size_start(source));

	pthread_mutex_unlock(&source->filter_mutex);

	return height;
}

uint32_t obs_source_get_width(obs_source_t *source)
{
	if (!data_valid(source, "obs_source_get_width"))
		return 0;

	/* a filter starts at itself (obs_source_size_entry()) and takes no
	 * lock: its filter_mutex guards filters it never reads */
	return (source->info.type != OBS_SOURCE_TYPE_FILTER) ? get_recurse_width(source)
							     : obs_source_base_width(source);
}

uint32_t obs_source_get_height(obs_source_t *source)
{
	if (!data_valid(source, "obs_source_get_height"))
		return 0;

	/* as obs_source_get_width() */
	return (source->info.type != OBS_SOURCE_TYPE_FILTER) ? get_recurse_height(source)
							     : obs_source_base_height(source);
}

_Static_assert(OBS_SOURCE_COLOR_SPACE_TYPE_FILTER == OBS_SOURCE_TYPE_FILTER, "OBS_SOURCE_TYPE_FILTER");
_Static_assert(OBS_SOURCE_COLOR_SPACE_VIDEO == OBS_SOURCE_VIDEO, "OBS_SOURCE_VIDEO");
_Static_assert(OBS_SOURCE_COLOR_SPACE_ASYNC == OBS_SOURCE_ASYNC, "OBS_SOURCE_ASYNC");
_Static_assert(sizeof(enum gs_color_space) == sizeof(int), "enum gs_color_space is passed as int");
_Static_assert(sizeof(enum obs_source_type) == sizeof(int), "enum obs_source_type is passed as int");

enum gs_color_space obs_source_get_color_space(obs_source_t *source, size_t count,
					       const enum gs_color_space *preferred_spaces)
{
	if (!data_valid(source, "obs_source_get_color_space"))
		return GS_CS_SRGB;

	const struct obs_source_color_space_node node = {
		.type = (int)source->info.type,
		.output_flags = source->info.output_flags,
		.has_data = source->context.data != NULL,
		.enabled = source->enabled,
		.has_filter_parent = source->filter_parent != NULL,
		.has_filter_target = source->filter_target != NULL,
	};

	switch (obs_source_color_space_route(node)) {
	case OBS_SOURCE_COLOR_SPACE_ASK_PARENT:
		return obs_source_get_color_space(source->filter_parent, count, preferred_spaces);
	case OBS_SOURCE_COLOR_SPACE_ASK_TARGET:
		return obs_source_get_color_space(source->filter_target, count, preferred_spaces);
	case OBS_SOURCE_COLOR_SPACE_ASYNC_PICK: {
		const enum gs_color_space video_space = convert_video_space(source->async_format, source->async_trc);
		return (enum gs_color_space)obs_source_color_space_pick((int)video_space, count,
									(const int *)preferred_spaces);
	}
	case OBS_SOURCE_COLOR_SPACE_CALLBACK:
		break;
	}

	assert(source->context.data);
	return source->info.video_get_color_space
		       ? source->info.video_get_color_space(source->context.data, count, preferred_spaces)
		       : GS_CS_SRGB;
}

_Static_assert(OBS_SOURCE_FILTER_CHAIN_VIDEO == OBS_SOURCE_VIDEO, "OBS_SOURCE_VIDEO");
_Static_assert(OBS_SOURCE_FILTER_CHAIN_AUDIO == OBS_SOURCE_AUDIO, "OBS_SOURCE_AUDIO");
_Static_assert(OBS_SOURCE_FILTER_CHAIN_ASYNC == OBS_SOURCE_ASYNC, "OBS_SOURCE_ASYNC");
_Static_assert(OBS_SOURCE_FILTER_CHAIN_MOVE_UP == OBS_ORDER_MOVE_UP, "OBS_ORDER_MOVE_UP");
_Static_assert(OBS_SOURCE_FILTER_CHAIN_MOVE_DOWN == OBS_ORDER_MOVE_DOWN, "OBS_ORDER_MOVE_DOWN");
_Static_assert(OBS_SOURCE_FILTER_CHAIN_MOVE_TOP == OBS_ORDER_MOVE_TOP, "OBS_ORDER_MOVE_TOP");
_Static_assert(OBS_SOURCE_FILTER_CHAIN_MOVE_BOTTOM == OBS_ORDER_MOVE_BOTTOM, "OBS_ORDER_MOVE_BOTTOM");

bool filter_compatible(obs_source_t *source, obs_source_t *filter)
{
	return obs_source_filter_chain_compatible(source->info.output_flags, filter->info.output_flags);
}

void reorder_filter_targets(obs_source_t *source)
{
	/* reorder filter targets, not the nicest way of dealing with things */
	for (size_t i = 0; i < source->filters.num; i++) {
		obs_source_t *next_filter = obs_source_filter_chain_target_of(i, source->filters.num) ==
							    OBS_SOURCE_FILTER_CHAIN_TARGET_SOURCE
						    ? source
						    : source->filters.array[i + 1];

		source->filters.array[i]->filter_target = next_filter;
	}
}

static bool set_filter_index(obs_source_t *source, obs_source_t *filter, size_t index)
{
	size_t idx = da_find(source->filters, &filter, 0);
	if (idx == DARRAY_INVALID)
		return false;

	da_move_item(source->filters, idx, index);
	reorder_filter_targets(source);

	return true;
}

void obs_source_filter_set_index(obs_source_t *source, obs_source_t *filter, size_t index)
{
	bool success;

	if (!obs_source_valid(source, "obs_source_filter_set_index"))
		return;
	if (!obs_ptr_valid(filter, "obs_source_filter_set_index"))
		return;

	pthread_mutex_lock(&source->filter_mutex);
	success = set_filter_index(source, filter, index);
	pthread_mutex_unlock(&source->filter_mutex);

	if (success)
		obs_source_dosignal(source, NULL, "reorder_filters");
}

struct obs_source_frame *filter_async_video(obs_source_t *source, struct obs_source_frame *in)
{
	size_t i;

	pthread_mutex_lock(&source->filter_mutex);

	for (i = source->filters.num; i > 0; i--) {
		struct obs_source *filter = source->filters.array[i - 1];

		if (!filter->enabled)
			continue;

		if (filter->context.data && filter->info.filter_video) {
			in = filter->info.filter_video(filter->context.data, in);
			if (!in)
				break;
		}
	}

	pthread_mutex_unlock(&source->filter_mutex);

	return in;
}

/* What async_texture_changed() compared: the cache's size and convert type,
 * and the frame's. */
static inline struct obs_source_frame_copy_key async_cache_key(const struct obs_source *source)
{
	const struct obs_source_frame_copy_key key = {
		.width = source->async_cache_width,
		.height = source->async_cache_height,
		.convert_type = (int)get_convert_type(source->async_cache_format, source->async_cache_full_range,
						      source->async_cache_trc),
	};
	return key;
}

static inline struct obs_source_frame_copy_key async_frame_key(const struct obs_source_frame *frame)
{
	const struct obs_source_frame_copy_key key = {
		.width = frame->width,
		.height = frame->height,
		.convert_type = (int)get_convert_type(frame->format, frame->full_range, frame->trc),
	};
	return key;
}

static inline void free_async_cache(struct obs_source *source)
{
	for (size_t i = 0; i < source->async_cache.num; i++)
		obs_source_frame_decref(source->async_cache.array[i].frame);

	da_resize(source->async_cache, 0);
	da_resize(source->async_frames, 0);
	source->cur_async_frame = NULL;
	source->prev_async_frame = NULL;
}

#define MAX_UNUSED_FRAME_DURATION 5
_Static_assert(MAX_UNUSED_FRAME_DURATION == OBS_SOURCE_FRAME_COPY_MAX_UNUSED, "MAX_UNUSED_FRAME_DURATION");

/* frees frame allocations if they haven't been used for a specific period
 * of time */
static void clean_cache(obs_source_t *source)
{
	for (size_t i = source->async_cache.num; i > 0; i--) {
		struct async_frame *af = &source->async_cache.array[i - 1];
		const struct obs_source_frame_cache_age age =
			obs_source_frame_copy_cache_clean(af->used, af->unused_count);
		if (age.counted) {
			af->unused_count = age.unused_count;
			if (age.drop) {
				obs_source_frame_destroy(af->frame);
				da_erase(source->async_cache, i - 1);
			}
		}
	}
}

#define MAX_ASYNC_FRAMES 30
_Static_assert(MAX_ASYNC_FRAMES == OBS_SOURCE_FRAME_COPY_MAX_ASYNC_FRAMES, "MAX_ASYNC_FRAMES");
//if return value is not null then do (os_atomic_dec_long(&output->refs) == 0) && obs_source_frame_destroy(output)
static inline struct obs_source_frame *cache_video(struct obs_source *source, const struct obs_source_frame *frame)
{
	struct obs_source_frame *new_frame = NULL;

	pthread_mutex_lock(&source->async_mutex);

	const enum obs_source_frame_cache_step step = obs_source_frame_copy_cache_step(
		source->async_frames.num, async_cache_key(source), async_frame_key(frame));

	if (step == OBS_SOURCE_FRAME_CACHE_OVERFLOW) {
		free_async_cache(source);
		/* Overflow drops queued pictures, not source time. Keep the
		 * running clock so the next tick catches up after a graphics
		 * stall instead of anchoring audio to the leftover backlog. */
		pthread_mutex_unlock(&source->async_mutex);
		return NULL;
	}

	if (step == OBS_SOURCE_FRAME_CACHE_RESET) {
		free_async_cache(source);
		source->async_cache_width = frame->width;
		source->async_cache_height = frame->height;
	}

	const enum video_format format = frame->format;
	source->async_cache_format = format;
	source->async_cache_full_range = frame->full_range;
	source->async_cache_trc = frame->trc;

	for (size_t i = 0; i < source->async_cache.num; i++) {
		struct async_frame *af = &source->async_cache.array[i];
		if (!af->used) {
			new_frame = af->frame;
			new_frame->format = format;
			/* A frame from the cache starts unpaired, whatever its
			 * last use left on it (#2009). */
			new_frame->prev_frame = false;
			af->used = true;
			af->unused_count = 0;
			break;
		}
	}

	clean_cache(source);

	if (!new_frame) {
		struct async_frame new_af;

		new_frame = obs_source_frame_create(format, frame->width, frame->height);
		new_af.frame = new_frame;
		new_af.used = true;
		new_af.unused_count = 0;
		new_frame->refs = 1;

		da_push_back(source->async_cache, &new_af);
	}

	os_atomic_inc_long(&new_frame->refs);

	pthread_mutex_unlock(&source->async_mutex);

	obs_source_frame_copy(new_frame, frame);

	return new_frame;
}

void obs_source_output_video_internal(obs_source_t *source, const struct obs_source_frame *frame)
{
	if (!obs_source_valid(source, "obs_source_output_video"))
		return;

	/* the clock of the source's video, for its audio (#3747): taken alone,
	 * the graphics thread takes audio_buf_mutex inside async_mutex */
	pthread_mutex_lock(&source->audio_buf_mutex);
	source->last_output_frame_ts = frame ? frame->timestamp : 0;
	pthread_mutex_unlock(&source->audio_buf_mutex);

	if (!frame) {
		pthread_mutex_lock(&source->async_mutex);
		os_atomic_set_bool(&source->async_active, false);
		/* A new frame makes the source active again as soon as it is
		 * queued, but it only reaches the textures when it is rendered.
		 * Until then, do not draw what was there before. */
		os_atomic_set_bool(&source->async_textures_stale, true);
		source->last_frame_ts = 0;
		free_async_cache(source);
		pthread_mutex_unlock(&source->async_mutex);
		return;
	}

	source_profiler_async_frame_received(source);

	struct obs_source_frame *output = cache_video(source, frame);

	/* ------------------------------------------- */
	pthread_mutex_lock(&source->async_mutex);
	if (output) {
		if (os_atomic_dec_long(&output->refs) == 0) {
			obs_source_frame_destroy(output);
			output = NULL;
		} else {
			da_push_back(source->async_frames, &output);
			os_atomic_set_bool(&source->async_active, true);
		}
	}
	pthread_mutex_unlock(&source->async_mutex);
}

struct obs_audio_data *filter_async_audio(obs_source_t *source, struct obs_audio_data *in)
{
	size_t i;
	for (i = source->filters.num; i > 0; i--) {
		struct obs_source *filter = source->filters.array[i - 1];

		if (!filter->enabled)
			continue;

		if (filter->context.data && filter->info.filter_audio) {
			in = filter->info.filter_audio(filter->context.data, in);
			if (!in)
				return NULL;
		}
	}

	return in;
}

_Static_assert(OBS_SOURCE_AUDIO_INPUT_SMOOTHING_THRESHOLD == TS_SMOOTHING_THRESHOLD, "TS_SMOOTHING_THRESHOLD");
_Static_assert(OBS_SOURCE_AUDIO_INPUT_SPEAKERS_STEREO == SPEAKERS_STEREO, "SPEAKERS_STEREO");
_Static_assert(OBS_SOURCE_AUDIO_INPUT_FORCE_MONO == OBS_SOURCE_FLAG_FORCE_MONO, "OBS_SOURCE_FLAG_FORCE_MONO");
_Static_assert(OBS_SOURCE_AUDIO_INPUT_SINE_LAW == OBS_BALANCE_TYPE_SINE_LAW, "OBS_BALANCE_TYPE_SINE_LAW");
_Static_assert(OBS_SOURCE_AUDIO_INPUT_SQUARE_LAW == OBS_BALANCE_TYPE_SQUARE_LAW, "OBS_BALANCE_TYPE_SQUARE_LAW");
_Static_assert(OBS_SOURCE_AUDIO_INPUT_LINEAR == OBS_BALANCE_TYPE_LINEAR, "OBS_BALANCE_TYPE_LINEAR");
_Static_assert(sizeof(enum audio_format) == sizeof(int), "enum audio_format is passed as int");
_Static_assert(sizeof(enum speaker_layout) == sizeof(int), "enum speaker_layout is passed as int");

/* obs-source-async-pacing.c decides with the same threshold */
_Static_assert(OBS_SOURCE_ASYNC_PACING_MAX_TS_VAR == MAX_TS_VAR, "MAX_TS_VAR");

/* async_mutex is held (async_tick()). obs-source-async-pacing.c decides from
 * the queue's timestamps; this applies the result in the order
 * ready_async_frame() and get_closest_frame() did: the dropped frames off the
 * front, each erased and then released to the cache, the new last_frame_ts,
 * and the front frame taken. */
static inline struct obs_source_frame *get_closest_frame(obs_source_t *source, uint64_t sys_time)
{
	uint64_t stack_timestamps[MAX_ASYNC_FRAMES];
	uint64_t *timestamps = stack_timestamps;
	const size_t num = source->async_frames.num;
	struct obs_source_async_pacing_result pacing;
	struct obs_source_frame *frame = NULL;

	if (!num)
		return NULL;

	/* cache_video() stops at MAX_ASYNC_FRAMES, but it unlocks async_mutex
	 * before obs_source_output_video_internal() queues the frame, so
	 * sources that output from more than one thread can queue a few more. */
	if (num > MAX_ASYNC_FRAMES)
		timestamps = bmalloc(num * sizeof(*timestamps));
	for (size_t i = 0; i < num; i++)
		timestamps[i] = source->async_frames.array[i]->timestamp;

	pacing = obs_source_async_pacing(timestamps, num, sys_time, source->last_frame_ts, source->last_sys_timestamp,
					 source->async_unbuffered);

	if (timestamps != stack_timestamps)
		bfree(timestamps);

	for (size_t i = 0; i < pacing.drop_front; i++) {
		struct obs_source_frame *dropped = source->async_frames.array[0];
		da_erase(source->async_frames, 0);
		remove_async_frame(source, dropped);
	}

	source->last_frame_ts = pacing.last_frame_ts;

	if (pacing.take_first) {
		frame = source->async_frames.array[0];
		da_erase(source->async_frames, 0);
	}

	return frame;
}

static inline void render_filter_bypass(obs_source_t *target, gs_effect_t *effect, const char *tech_name)
{
	gs_technique_t *tech = gs_effect_get_technique(effect, tech_name);
	size_t passes, i;

	passes = gs_technique_begin(tech);
	for (i = 0; i < passes; i++) {
		gs_technique_begin_pass(tech, i);
		obs_source_video_render(target);
		gs_technique_end_pass(tech);
	}
	gs_technique_end(tech);
}

static inline void render_filter_tex(gs_texture_t *tex, gs_effect_t *effect, uint32_t width, uint32_t height,
				     const char *tech_name)
{
	gs_technique_t *tech = gs_effect_get_technique(effect, tech_name);
	gs_eparam_t *image = gs_effect_get_param_by_name(effect, "image");
	size_t passes, i;

	const bool linear_srgb = gs_get_linear_srgb();

	const bool previous = gs_framebuffer_srgb_enabled();
	gs_enable_framebuffer_srgb(linear_srgb);

	if (linear_srgb)
		gs_effect_set_texture_srgb(image, tex);
	else
		gs_effect_set_texture(image, tex);

	passes = gs_technique_begin(tech);
	for (i = 0; i < passes; i++) {
		gs_technique_begin_pass(tech, i);
		gs_draw_sprite(tex, 0, width, height);
		gs_technique_end_pass(tech);
	}
	gs_technique_end(tech);

	gs_enable_framebuffer_srgb(previous);
}

/* The static draws obs_source_process_filter_tech_end() makes
 * (obs-source-process-filter-end.c.h). Hidden: no EXPORT. */
void obs_source_process_filter_end_render_bypass(obs_source_t *target, gs_effect_t *effect, const char *tech_name);
void obs_source_process_filter_end_render_bypass(obs_source_t *target, gs_effect_t *effect, const char *tech_name)
{
	render_filter_bypass(target, effect, tech_name);
}

void obs_source_process_filter_end_render_tex(gs_texture_t *tex, gs_effect_t *effect, uint32_t width, uint32_t height,
					      const char *tech_name);
void obs_source_process_filter_end_render_tex(gs_texture_t *tex, gs_effect_t *effect, uint32_t width, uint32_t height,
					      const char *tech_name)
{
	render_filter_tex(tex, effect, width, height, tech_name);
}

/* The static draws obs_source_skip_video_filter() makes
 * (obs-source-skip-video-filter.c.h). Hidden: no EXPORT. */
void obs_source_skip_video_filter_main_render(obs_source_t *source);
void obs_source_skip_video_filter_main_render(obs_source_t *source)
{
	obs_source_main_render(source);
}

void obs_source_skip_video_filter_async_video(obs_source_t *source);
void obs_source_skip_video_filter_async_video(obs_source_t *source)
{
	obs_source_render_async_video(source);
}

struct source_enum_data {
	obs_source_enum_proc_t enum_callback;
	void *param;
	obs_source_t *stop_source;
};

static void enum_source_active_tree_callback(obs_source_t *parent, obs_source_t *child, void *param)
{
	struct source_enum_data *data = param;
	bool is_transition = child->info.type == OBS_SOURCE_TYPE_TRANSITION;

	if (is_transition)
		obs_transition_enum_sources(child, enum_source_active_tree_callback, param);
	if (child->info.enum_active_sources) {
		if (child->context.data) {
			child->info.enum_active_sources(child->context.data, enum_source_active_tree_callback, data);
		}
	}

	data->enum_callback(parent, child, data->param);
}

void obs_source_enum_active_sources(obs_source_t *source, obs_source_enum_proc_t enum_callback, void *param)
{
	bool is_transition;
	if (!data_valid(source, "obs_source_enum_active_sources"))
		return;

	is_transition = source->info.type == OBS_SOURCE_TYPE_TRANSITION;
	if (!is_transition && !source->info.enum_active_sources)
		return;

	source = obs_source_get_ref(source);
	if (!data_valid(source, "obs_source_enum_active_sources"))
		return;

	if (is_transition)
		obs_transition_enum_sources(source, enum_callback, param);
	if (source->info.enum_active_sources)
		source->info.enum_active_sources(source->context.data, enum_callback, param);

	obs_source_release(source);
}

void obs_source_enum_active_tree(obs_source_t *source, obs_source_enum_proc_t enum_callback, void *param)
{
	struct source_enum_data data = {enum_callback, param};
	bool is_transition;

	if (!data_valid(source, "obs_source_enum_active_tree"))
		return;

	is_transition = source->info.type == OBS_SOURCE_TYPE_TRANSITION;
	if (!is_transition && !source->info.enum_active_sources)
		return;

	source = obs_source_get_ref(source);
	if (!data_valid(source, "obs_source_enum_active_tree"))
		return;

	if (source->info.type == OBS_SOURCE_TYPE_TRANSITION)
		obs_transition_enum_sources(source, enum_source_active_tree_callback, &data);
	if (source->info.enum_active_sources)
		source->info.enum_active_sources(source->context.data, enum_source_active_tree_callback, &data);

	obs_source_release(source);
}

_Static_assert(OBS_SOURCE_MONITORING_AUDIO == OBS_SOURCE_AUDIO, "OBS_SOURCE_AUDIO");
_Static_assert(OBS_SOURCE_MONITORING_TYPE_NONE == OBS_MONITORING_TYPE_NONE, "OBS_MONITORING_TYPE_NONE");
_Static_assert(OBS_SOURCE_MONITORING_TYPE_MONITOR_ONLY == OBS_MONITORING_TYPE_MONITOR_ONLY,
	       "OBS_MONITORING_TYPE_MONITOR_ONLY");
_Static_assert(OBS_SOURCE_MONITORING_TYPE_MONITOR_AND_OUTPUT == OBS_MONITORING_TYPE_MONITOR_AND_OUTPUT,
	       "OBS_MONITORING_TYPE_MONITOR_AND_OUTPUT");
_Static_assert(sizeof(enum obs_monitoring_type) == sizeof(int), "enum obs_monitoring_type is passed as int");

obs_source_t *obs_source_get_filter_by_name(obs_source_t *source, const char *name)
{
	obs_source_t *filter = NULL;

	if (!obs_source_valid(source, "obs_source_get_filter_by_name"))
		return NULL;
	if (!obs_ptr_valid(name, "obs_source_get_filter_by_name"))
		return NULL;

	pthread_mutex_lock(&source->filter_mutex);

	for (size_t i = 0; i < source->filters.num; i++) {
		struct obs_source *cur_filter = source->filters.array[i];
		if (strcmp(cur_filter->context.name, name) == 0) {
			filter = obs_source_get_ref(cur_filter);
			break;
		}
	}

	pthread_mutex_unlock(&source->filter_mutex);

	return filter;
}

static void source_signal_push_to_changed(obs_source_t *source, const char *signal, bool enabled)
{
	struct calldata data;
	uint8_t stack[128];

	calldata_init_fixed(&data, stack, sizeof(stack));
	calldata_set_ptr(&data, "source", source);
	calldata_set_bool(&data, "enabled", enabled);

	signal_handler_signal(source->context.signals, signal, &data);
}

/* Keep signal construction in obs-source.c.h for the extracted setters. */
void obs_source_push_to_enable_signal_changed(obs_source_t *source, const char *signal, bool enabled)
{
	source_signal_push_to_changed(source, signal, enabled);
}

/* The multiplies go through obs-source-volume-mul.c, on plain pointers and
 * counts. A mix's channel buffers are consecutive (allocate_audio_output_buffer()),
 * so multiply_output_audio() multiplies them as one block from channel 0. */
static inline void multiply_output_audio(obs_source_t *source, size_t mix, size_t channels, float vol)
{
	obs_source_volume_mul_block(source->audio_output_buf[mix][0], AUDIO_OUTPUT_FRAMES * channels, vol);
}

static inline void multiply_vol_data(obs_source_t *source, size_t mix, size_t channels, float *vol_data)
{
	for (size_t ch = 0; ch < channels; ch++)
		obs_source_volume_mul_data(source->audio_output_buf[mix][ch], vol_data, AUDIO_OUTPUT_FRAMES);
}

static inline void apply_audio_action(obs_source_t *source, const struct audio_action *action)
{
	switch (action->type) {
	case AUDIO_ACTION_VOL:
		source->volume = action->vol;
		break;
	case AUDIO_ACTION_MUTE:
		source->muted = action->set;
		break;
	case AUDIO_ACTION_PTT:
		source->push_to_talk_pressed = action->set;
		break;
	case AUDIO_ACTION_PTM:
		source->push_to_mute_pressed = action->set;
		break;
	}
}

/* Each action's clamp, frame and stop, how far the previous volume fills and
 * the mixes multiplied go through obs-source-audio-place.c, on values. The
 * queue and its mutex, apply_audio_action(), the get_source_volume() calls
 * (before the loop and after each applied action, so the push-to-talk and
 * push-to-mute state of each action is seen), the volume array and
 * multiply_vol_data() stay here. source->audio_ts is read where it was:
 * once for the clamp, again when the clamp applies, and again for the
 * frame. */
static void apply_audio_actions(obs_source_t *source, size_t channels, size_t sample_rate)
{
	float vol_data[AUDIO_OUTPUT_FRAMES];
	float cur_vol = get_source_volume(source, source->audio_ts);
	size_t frame_num = 0;

	pthread_mutex_lock(&source->audio_actions_mutex);

	for (size_t i = 0; i < source->audio_actions.num; i++) {
		struct audio_action action = source->audio_actions.array[i];
		uint64_t timestamp = action.timestamp;
		size_t new_frame_num;

		if (obs_source_audio_place_older(timestamp, source->audio_ts))
			timestamp = source->audio_ts;

		new_frame_num = obs_source_audio_place_action_frame(timestamp, source->audio_ts, sample_rate);

		if (obs_source_audio_place_action_stops(new_frame_num))
			break;

		da_erase(source->audio_actions, i--);

		apply_audio_action(source, &action);

		size_t fill_end = obs_source_audio_place_fill_end(frame_num, new_frame_num);
		for (; frame_num < fill_end; frame_num++)
			vol_data[frame_num] = cur_vol;

		cur_vol = get_source_volume(source, timestamp);
	}

	for (; frame_num < AUDIO_OUTPUT_FRAMES; frame_num++)
		vol_data[frame_num] = cur_vol;

	pthread_mutex_unlock(&source->audio_actions_mutex);

	/* source->audio_mixers is read once per mix, as before. */
	for (size_t mix = 0; mix < MAX_AUDIO_MIXES; mix++) {
		if (obs_source_audio_place_mix_on(source->audio_mixers, mix))
			multiply_vol_data(source, mix, channels, vol_data);
	}
}

/* Whether the first queued action is due, and skip, clear or multiply, go
 * through obs-source-volume.c, on values. The queue and its mutex,
 * apply_audio_actions(), conv_frames_to_time(), the memset and the multiply
 * stay here. */
static void apply_audio_volume(obs_source_t *source, uint32_t mixers, size_t channels, size_t sample_rate)
{
	/* Set only when an action is pending, and read under the same guard;
	 * MSVC's flow analysis cannot carry that across the branch: C4701 at
	 * /W4. Zero-initializing cannot be observed, the read is guarded. */
	struct audio_action action = {0};
	bool actions_pending;
	float vol;

	pthread_mutex_lock(&source->audio_actions_mutex);

	actions_pending = source->audio_actions.num > 0;
	if (actions_pending)
		action = source->audio_actions.array[0];

	pthread_mutex_unlock(&source->audio_actions_mutex);

	if (actions_pending) {
		uint64_t duration = conv_frames_to_time(sample_rate, AUDIO_OUTPUT_FRAMES);

		if (obs_source_volume_action_due(action.timestamp, source->audio_ts, duration)) {
			apply_audio_actions(source, channels, sample_rate);
			return;
		}
	}

	vol = get_source_volume(source, source->audio_ts);

	switch (obs_source_volume_step(vol, mixers)) {
	case OBS_SOURCE_VOLUME_SKIP:
		return;
	case OBS_SOURCE_VOLUME_CLEAR:
		memset(source->audio_output_buf[0][0], 0,
		       AUDIO_OUTPUT_FRAMES * sizeof(float) * MAX_AUDIO_CHANNELS * MAX_AUDIO_MIXES);
		return;
	case OBS_SOURCE_VOLUME_MULTIPLY:
	default:
		break;
	}

	/* source->audio_mixers is read once per mix, as before. */
	for (size_t mix = 0; mix < MAX_AUDIO_MIXES; mix++) {
		if (obs_source_volume_mixes(source->audio_mixers, mixers, mix))
			multiply_output_audio(source, mix, channels, vol);
	}
}

/* Which mixes are cleared before and after the callback, and its timestamp
 * and pending result, go through obs-source-audio-render.c, on values. The
 * buffers, the callback, audio_buf_mutex, the memsets and
 * apply_audio_volume() stay here, in the same order; source->audio_mixers is
 * read once per mix where it was before, source->audio_ts only after a
 * successful callback. */
static void custom_audio_render(obs_source_t *source, uint32_t mixers, size_t channels, size_t sample_rate)
{
	struct obs_source_audio_mix audio_data;
	bool success;
	uint64_t ts;

	for (size_t mix = 0; mix < MAX_AUDIO_MIXES; mix++) {
		for (size_t ch = 0; ch < channels; ch++) {
			audio_data.output[mix].data[ch] = source->audio_output_buf[mix][ch];
		}

		if (obs_source_audio_render_custom_clear_before(source->audio_mixers, mixers, mix)) {
			memset(source->audio_output_buf[mix][0], 0, sizeof(float) * AUDIO_OUTPUT_FRAMES * channels);
		}
	}

	success = source->info.audio_render(source->context.data, &ts, &audio_data, mixers, channels, sample_rate);
	/* The callback may render other sources or submit capture audio. */
	pthread_mutex_lock(&source->audio_buf_mutex);
	/* ts is passed but not used when the callback failed. */
	source->audio_ts = obs_source_audio_render_custom_ts(success, ts);
	source->audio_pending = obs_source_audio_render_custom_pending(success);

	if (obs_source_audio_render_custom_pending(success) ||
	    obs_source_audio_render_custom_unmixed(source->audio_ts, mixers)) {
		pthread_mutex_unlock(&source->audio_buf_mutex);
		return;
	}

	for (size_t mix = 0; mix < MAX_AUDIO_MIXES; mix++) {
		if (!obs_source_audio_render_custom_in_tick(mixers, mix))
			continue;

		if (obs_source_audio_render_custom_clear_after(source->audio_mixers, mix)) {
			memset(source->audio_output_buf[mix][0], 0, sizeof(float) * AUDIO_OUTPUT_FRAMES * channels);
		}
	}

	apply_audio_volume(source, mixers, channels, sample_rate);
	pthread_mutex_unlock(&source->audio_buf_mutex);
}

static void audio_submix(obs_source_t *source, size_t channels, size_t sample_rate)
{
	struct audio_output_data audio_data;
	struct obs_source_audio audio = {0};
	bool success;
	uint64_t ts;

	for (size_t ch = 0; ch < channels; ch++) {
		audio_data.data[ch] = source->audio_mix_buf[ch];
	}

	memset(source->audio_mix_buf[0], 0, sizeof(float) * AUDIO_OUTPUT_FRAMES * channels);

	success = source->info.audio_mix(source->context.data, &ts, &audio_data, channels, sample_rate);

	if (!success)
		return;

	for (size_t i = 0; i < channels; i++)
		audio.data[i] = (const uint8_t *)audio_data.data[i];

	audio.samples_per_sec = (uint32_t)sample_rate;
	audio.frames = AUDIO_OUTPUT_FRAMES;
	audio.format = AUDIO_FORMAT_FLOAT_PLANAR;
	audio.speakers = (enum speaker_layout)channels;
	audio.timestamp = ts;

	obs_source_output_audio(source, &audio);
}

/* The pending check and whether each mix copies mix 0 or is cleared go
 * through obs-source-audio-render.c, on values. audio_buf_mutex,
 * deque_peek_front(), the memsets, the memcpys and apply_audio_volume() stay
 * here, in the same order; the input size is read only with a timestamp, and
 * source->audio_mixers once per mix where it was before. */
static inline void process_audio_source_tick(obs_source_t *source, uint32_t mixers, size_t channels, size_t sample_rate,
					     size_t size)
{
	bool audio_submix = !!(source->info.output_flags & OBS_SOURCE_SUBMIX);

	pthread_mutex_lock(&source->audio_buf_mutex);

	if (obs_source_audio_render_tick_unset(source->audio_ts) ||
	    obs_source_audio_render_tick_short(source->audio_input_buf[0].size, size)) {
		source->audio_pending = true;
		pthread_mutex_unlock(&source->audio_buf_mutex);
		return;
	}

	for (size_t ch = 0; ch < channels; ch++)
		deque_peek_front(&source->audio_input_buf[ch], source->audio_output_buf[0][ch], size);

	for (size_t mix = 1; mix < MAX_AUDIO_MIXES; mix++) {
		if (obs_source_audio_render_tick_stop(audio_submix, mix))
			break;

		if (!obs_source_audio_render_tick_copy(audio_submix, source->audio_mixers, mixers, mix)) {
			memset(source->audio_output_buf[mix][0], 0, size * channels);
			continue;
		}

		for (size_t ch = 0; ch < channels; ch++)
			memcpy(source->audio_output_buf[mix][ch], source->audio_output_buf[0][ch], size);
	}

	if (!obs_source_audio_render_tick_volume(audio_submix)) {
		source->audio_pending = false;
		pthread_mutex_unlock(&source->audio_buf_mutex);
		return;
	}

	if (obs_source_audio_render_tick_clear_main(source->audio_mixers, mixers))
		memset(source->audio_output_buf[0][0], 0, size * channels);

	apply_audio_volume(source, mixers, channels, sample_rate);
	source->audio_pending = false;
	pthread_mutex_unlock(&source->audio_buf_mutex);
}

/* The route goes through obs-source-audio-render.c, on values. Each field is
 * read only where the checks before reached it, as before: the audio thread
 * can render a source whose context.data is not set yet (432017b2c). */
void obs_source_audio_render(obs_source_t *source, uint32_t mixers, size_t channels, size_t sample_rate, size_t size)
{
	const bool has_buffer = source->audio_output_buf[0][0] != NULL;
	const bool custom = has_buffer && source->info.audio_render != NULL;
	const bool has_data = custom && source->context.data != NULL;

	switch (obs_source_audio_render_route(has_buffer, custom, has_data)) {
	case OBS_SOURCE_AUDIO_RENDER_PENDING:
		source->audio_pending = true;
		return;
	case OBS_SOURCE_AUDIO_RENDER_CUSTOM:
		custom_audio_render(source, mixers, channels, sample_rate);
		return;
	case OBS_SOURCE_AUDIO_RENDER_TICK:
	default:
		break;
	}

	if (source->info.audio_mix) {
		audio_submix(source, channels, sample_rate);
	}

	process_audio_source_tick(source, mixers, channels, sample_rate, size);
}

/* hidden/undocumented export to allow source type redefinition for scripts */

EXPORT void obs_enable_source_type(const char *name, bool enable);
EXPORT void obs_enable_source_type(const char *name, bool enable)
{
	struct obs_source_info *info = get_source_info(name);
	if (!info)
		return;

	if (enable)
		info->output_flags &= ~(uint32_t)OBS_SOURCE_CAP_DISABLED;
	else
		info->output_flags |= OBS_SOURCE_CAP_DISABLED;
}

_Static_assert(OBS_SOURCE_MEDIA_CONTROLLABLE == OBS_SOURCE_CONTROLLABLE_MEDIA, "OBS_SOURCE_CONTROLLABLE_MEDIA");
_Static_assert(OBS_SOURCE_MEDIA_STATE_NONE == OBS_MEDIA_STATE_NONE, "OBS_MEDIA_STATE_NONE");
_Static_assert(sizeof(enum obs_media_state) == sizeof(int), "enum obs_media_state is passed as int");

/* File-local macros: keep them out of the next source of a unity build. */
#undef get_weak

/* File-local macros (BUILD.md): out of the next file in the unity group. */
#undef MAX_BUF_SIZE
#undef TS_SMOOTHING_THRESHOLD
#undef OBS_SOURCE_AV
#undef MAX_UNUSED_FRAME_DURATION
#undef MAX_ASYNC_FRAMES
