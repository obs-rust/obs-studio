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

#define DEBUG_AUDIO 0
#define DEBUG_LAGGED_AUDIO 0

static void push_audio_tree(obs_source_t *parent, obs_source_t *source, void *p)
{
	struct obs_core_audio *audio = p;

	if (da_find(audio->render_order, &source, 0) == DARRAY_INVALID) {
		obs_source_t *s = obs_source_get_ref(source);
		if (s) {
			da_push_back(audio->render_order, &s);
			s->audio_is_duplicated = false;
		}
	}

	UNUSED_PARAMETER(parent);
}

static inline bool is_individual_audio_source(obs_source_t *source)
{
	return source->info.type == OBS_SOURCE_TYPE_INPUT && (source->info.output_flags & OBS_SOURCE_AUDIO) &&
	       !(source->info.output_flags & OBS_SOURCE_COMPOSITE);
}

/*
 * This version of push_audio_tree checks whether any source is an Audio Output Capture source ('Desktop Audio',
 * 'wasapi_output_capture' on Windows, 'pulse_output_capture' on Linux, 'coreaudio_output_capture' on macOS), & if the
 * corresponding device is the monitoring device. It then sets the core audio bool 'prevent_monitoring_duplication' to
 * true, which will silence all monitored sources (unless the Audio Output Capture source is muted).
 * Moreover, it has the purpose of detecting sources which appear several times in the audio tree. They are then tagged
 * as such to avoid their mixing in scenes and transitions and mixed directly as root_nodes.
 */
static void push_audio_tree2(obs_source_t *parent, obs_source_t *source, void *p)
{
	if (obs_source_removed(source))
		return;

	struct obs_core_audio *audio = p;
	size_t idx = da_find(audio->render_order, &source, 0);

	if (idx == DARRAY_INVALID) {
		/* First time we see this source → add to render order */
		obs_source_t *s = obs_source_get_ref(source);
		if (s) {
			da_push_back(audio->render_order, &s);
			s->audio_is_duplicated = false;
		}
	} else {
		/* Source already present in tree → mark as duplicated if applicable */
		obs_source_t *s = audio->render_order.array[idx];
		if (is_individual_audio_source(s) && !s->audio_is_duplicated) {
			da_push_back(audio->root_nodes, &source);
			s->audio_is_duplicated = true;
		}
	}
	UNUSED_PARAMETER(parent);
}

/* The tick arithmetic is the core in obs-audio-mix.c, on these fields. */
static inline size_t convert_time_to_frames(size_t sample_rate, uint64_t t)
{
	return obs_audio_mix_time_to_frames(sample_rate, t);
}

static inline void mix_audio(struct audio_output_data *mixes, obs_source_t *source, size_t channels, size_t sample_rate,
			     struct ts_info *ts)
{
	obs_audio_mix_add(mixes, source->audio_output_buf, source->audio_ts, ts, channels, sample_rate);
}

static inline struct obs_audio_input audio_input(obs_source_t *source)
{
	struct obs_audio_input in = {
		.buf = source->audio_input_buf,
		.audio_ts = &source->audio_ts,
		.last_size = &source->last_audio_input_buf_size,
		.pending = &source->audio_pending,
		.pending_stop = &source->pending_stop,
		.timing_set = &source->timing_set,
	};
	return in;
}

/* The drop steps are the core in obs-audio-discard.c, on these fields. */
static bool ignore_audio(obs_source_t *source, size_t channels, size_t sample_rate, uint64_t start_ts)
{
	struct obs_audio_input in = audio_input(source);
	const char *name = obs_source_get_name(source);
#if DEBUG_LAGGED_AUDIO == 1
	uint64_t audio_ts = source->audio_ts;
#endif
	struct obs_audio_ignore_step step = obs_audio_discard_ignore(&in, channels, sample_rate, start_ts);

	if (step.no_timestamp) {
#if DEBUG_LAGGED_AUDIO == 1
		blog(LOG_DEBUG, "[src: %s] no timestamp, but audio available?", name);
#endif
		return false;
	}

	if (step.buffered) {
#if DEBUG_LAGGED_AUDIO == 1
		blog(LOG_DEBUG, "[src: %s] ignored %" PRIu64 "/%" PRIu64 " samples", name, (uint64_t)step.dropped,
		     (uint64_t)step.buffered);
#endif
		blog(LOG_DEBUG, "[src: %s] ts lag after ignoring: %" PRIu64, name, step.lag);
	} else {
#if DEBUG_LAGGED_AUDIO == 1
		blog(LOG_DEBUG, "[src: %s] no samples to ignore! ts = %" PRIu64, name, audio_ts);
#endif
	}

	if (step.warn) {
		blog(LOG_WARNING,
		     "Source %s audio is lagging (over by %.02f ms) "
		     "at max audio buffering. Restarting source audio.",
		     name, step.warn_lag / 1000000.);
	}

	return step.rerender;
}

static inline void discard_audio(struct obs_core_audio *audio, obs_source_t *source, size_t channels,
				 size_t sample_rate, struct ts_info *ts)
{
	struct obs_audio_input in = audio_input(source);
	struct obs_audio_discard_step step =
		obs_audio_discard_tick(&in, source->info.audio_render != NULL, channels, sample_rate, ts);
	/* debug assert only */
	UNUSED_PARAMETER(audio);

#if DEBUG_AUDIO == 1
	bool is_audio_source = source->info.output_flags & OBS_SOURCE_AUDIO;

	if (step.stop == OBS_AUDIO_STOP_PENDING)
		blog(LOG_DEBUG, "doing pending stop trick: '%s'", source->context.name);
	else if (step.stop == OBS_AUDIO_STOP_CLEARED)
		blog(LOG_DEBUG, "source audio data appears to have "
				"stopped, clearing");

	switch (step.path) {
	case OBS_AUDIO_DISCARD_RENDER_SOURCE:
		break;
	case OBS_AUDIO_DISCARD_AHEAD:
		blog(LOG_DEBUG,
		     "can't discard, source "
		     "timestamp (%" PRIu64 ") >= "
		     "end timestamp (%" PRIu64 ")",
		     source->audio_ts, ts->end);
		break;
	case OBS_AUDIO_DISCARD_BEHIND:
		if (step.stop == OBS_AUDIO_STOP_CLEARED)
			break;
		if (is_audio_source) {
			blog(LOG_DEBUG,
			     "can't discard, source "
			     "timestamp (%" PRIu64 ") < "
			     "start timestamp (%" PRIu64 ")",
			     source->audio_ts, ts->start);
		}

		/* ignore_audio should have already run and marked this source
		 * pending, unless we *just* added buffering */
		assert(audio->total_buffering_ticks < audio->max_buffering_ticks || source->audio_pending ||
		       !source->audio_ts || audio->buffering_wait_ticks);
		break;
	case OBS_AUDIO_DISCARD_START_AT_END:
		if (is_audio_source)
			blog(LOG_DEBUG, "can't discard, start point is "
					"at audio frame count");
		break;
	case OBS_AUDIO_DISCARD_PENDING:
		if (step.stop != OBS_AUDIO_STOP_CLEARED && is_audio_source)
			blog(LOG_DEBUG, "can't discard, data still pending");
		break;
	case OBS_AUDIO_DISCARD_DONE:
		if (is_audio_source)
			blog(LOG_DEBUG, "audio discarded, new ts: %" PRIu64, ts->end);
		break;
	}
#else
	UNUSED_PARAMETER(step);
#endif
}

static inline struct obs_audio_buffering audio_buffering(struct obs_core_audio *audio)
{
	struct obs_audio_buffering buffering = {
		.buffered_ts = &audio->buffered_ts,
		.buffered_timestamps = &audio->buffered_timestamps,
		.buffering_wait_ticks = &audio->buffering_wait_ticks,
		.total_buffering_ticks = &audio->total_buffering_ticks,
		.max_buffering_ticks = audio->max_buffering_ticks,
	};
	return buffering;
}

static inline bool audio_buffering_maxed(struct obs_core_audio *audio)
{
	struct obs_audio_buffering buffering = audio_buffering(audio);
	return obs_audio_buffering_maxed(&buffering);
}

static void set_fixed_audio_buffering(struct obs_core_audio *audio, size_t sample_rate, struct ts_info *ts)
{
	struct obs_audio_buffering buffering = audio_buffering(audio);
	struct obs_audio_buffering_step step = obs_audio_buffering_fixed(&buffering, sample_rate, ts);

	if (!step.added)
		return;

	blog(LOG_INFO,
	     "Enabling fixed audio buffering, total "
	     "audio buffering is now %d milliseconds",
	     (int)step.total_ms);
}

static void add_audio_buffering(struct obs_core_audio *audio, size_t sample_rate, struct ts_info *ts, uint64_t min_ts,
				const char *buffering_name)
{
	struct obs_audio_buffering buffering = audio_buffering(audio);
	const struct ts_info old_ts = *ts;
	struct obs_audio_buffering_step step = obs_audio_buffering_add(&buffering, sample_rate, ts, min_ts);

	if (!step.added)
		return;

	if (step.max_reached)
		blog(LOG_WARNING, "Max audio buffering reached!");

	blog(LOG_INFO,
	     "adding %d milliseconds of audio buffering, total "
	     "audio buffering is now %d milliseconds"
	     " (source: %s)\n",
	     (int)step.ms, (int)step.total_ms, buffering_name);
#if DEBUG_AUDIO == 1
	blog(LOG_DEBUG,
	     "min_ts (%" PRIu64 ") < start timestamp "
	     "(%" PRIu64 ")",
	     min_ts, old_ts.start);
	blog(LOG_DEBUG, "old buffered ts: %" PRIu64 "-%" PRIu64, old_ts.start, old_ts.end);
	blog(LOG_DEBUG, "new buffered ts: %" PRIu64 "-%" PRIu64, ts->start, ts->end);
#else
	UNUSED_PARAMETER(old_ts);
#endif
}

/* The per-source decisions are the core in obs-audio-min-ts.c, on these
 * fields. */
static bool audio_buffer_insufficient_locked(struct obs_source *source, size_t sample_rate, uint64_t min_ts)
{
	return obs_audio_min_ts_insufficient(source->info.audio_render != NULL, &source->audio_pending,
					     source->audio_ts, source->audio_input_buf[0].size, sample_rate, min_ts);
}

static bool audio_buffer_insufficient(struct obs_source *source, size_t sample_rate, uint64_t min_ts)
{
	pthread_mutex_lock(&source->audio_buf_mutex);
	bool insufficient = audio_buffer_insufficient_locked(source, sample_rate, min_ts);
	pthread_mutex_unlock(&source->audio_buf_mutex);
	return insufficient;
}

static inline const char *find_min_ts(struct obs_core_data *data, uint64_t *min_ts)
{
	obs_source_t *buffering_source = NULL;
	struct obs_source *source = data->first_audio_source;
	while (source) {
		pthread_mutex_lock(&source->audio_buf_mutex);
		if (obs_audio_min_ts_earlier(source->audio_pending, source->audio_ts, min_ts))
			buffering_source = source;
		pthread_mutex_unlock(&source->audio_buf_mutex);

		source = (struct obs_source *)source->next_audio_source;
	}
	return buffering_source ? obs_source_get_name(buffering_source) : NULL;
}

static inline bool mark_invalid_sources(struct obs_core_data *data, size_t sample_rate, uint64_t min_ts)
{
	bool recalculate = false;

	struct obs_source *source = data->first_audio_source;
	while (source) {
		recalculate |= audio_buffer_insufficient(source, sample_rate, min_ts);
		source = (struct obs_source *)source->next_audio_source;
	}

	return recalculate;
}

static inline const char *calc_min_ts(struct obs_core_data *data, size_t sample_rate, uint64_t *min_ts)
{
	const char *buffering_name = find_min_ts(data, min_ts);
	if (mark_invalid_sources(data, sample_rate, *min_ts))
		buffering_name = find_min_ts(data, min_ts);
	return buffering_name;
}

static inline void release_audio_sources(struct obs_core_audio *audio)
{
	for (size_t i = 0; i < audio->render_order.num; i++)
		obs_source_release(audio->render_order.array[i]);
}

static inline void execute_audio_tasks(void)
{
	struct obs_core_audio *audio = &obs->audio;
	pthread_mutex_lock(&audio->task_mutex);
	while (audio->tasks.size) {
		struct obs_task_info info;
		deque_pop_front(&audio->tasks, &info, sizeof(info));
		pthread_mutex_unlock(&audio->task_mutex);
		info.task(info.param);
		pthread_mutex_lock(&audio->task_mutex);
	}
	pthread_mutex_unlock(&audio->task_mutex);
}

/* In case monitoring and an 'Audio Output Capture' source have the same device, one silences all the monitored
 * sources unless the 'Audio Output Capture' is muted.
 */
static inline bool should_silence_monitored_source(obs_source_t *source, struct obs_core_audio *audio)
{
	obs_source_t *dup_src = audio->monitoring_duplicating_source;

	if (!dup_src || !obs_source_active(dup_src))
		return false;

	bool fader_muted = close_float(audio->monitoring_duplicating_source->volume, 0.0f, 0.0001f);
	bool output_capture_unmuted = !audio->monitoring_duplicating_source->muted && !fader_muted;

	if (output_capture_unmuted) {
		if (source->monitoring_enabled && source != audio->monitoring_duplicating_source) {
			return true;
		}
	}
	return false;
}

static inline void clear_audio_output_buf(obs_source_t *source, struct obs_core_audio *audio)
{
	if (!audio->monitoring_duplicating_source)
		return;

	uint32_t aoc_mixers = audio->monitoring_duplicating_source->audio_mixers;
	uint32_t source_mixers = source->audio_mixers;

	for (size_t mix = 0; mix < MAX_AUDIO_MIXES; mix++) {
		uint32_t mix_and_val = (1 << mix);
		if ((aoc_mixers & mix_and_val) && (source_mixers & mix_and_val)) {
			for (size_t ch = 0; ch < MAX_AUDIO_CHANNELS; ch++) {
				float *buf = source->audio_output_buf[mix][ch];
				if (buf)
					memset(buf, 0, AUDIO_OUTPUT_FRAMES * sizeof(float));
			}
		}
	}
}

bool audio_callback(void *param, uint64_t start_ts_in, uint64_t end_ts_in, uint64_t *out_ts, uint32_t mixers,
		    struct audio_output_data *mixes)
{
	struct obs_core_data *data = &obs->data;
	struct obs_core_audio *audio = &obs->audio;
	struct obs_source *source;
	size_t sample_rate = audio_output_get_sample_rate(audio->audio);
	size_t channels = audio_output_get_channels(audio->audio);
	struct ts_info ts = {start_ts_in, end_ts_in};
	size_t audio_size;
	uint64_t min_ts;

	da_resize(audio->render_order, 0);
	da_resize(audio->root_nodes, 0);

	deque_push_back(&audio->buffered_timestamps, &ts, sizeof(ts));
	deque_peek_front(&audio->buffered_timestamps, &ts, sizeof(ts));
	min_ts = ts.start;

	audio_size = AUDIO_OUTPUT_FRAMES * sizeof(float);

#if DEBUG_AUDIO == 1
	blog(LOG_DEBUG, "ts %llu-%llu", ts.start, ts.end);
#endif

	/* ------------------------------------------------ */
	/* build audio render order */

	pthread_mutex_lock(&obs->video.mixes_mutex);
	for (size_t j = 0; j < obs->video.mixes.num; j++) {
		struct obs_core_video_mix *mix = obs->video.mixes.array[j];
		struct obs_view *view = mix->view;
		bool duplicate_view = false;

		for (size_t k = 0; k < j; ++k) {
			if (obs->video.mixes.array[k]->view == view) {
				duplicate_view = true;
				break;
			}
		}
		if (!view || duplicate_view)
			continue;

		pthread_mutex_lock(&view->channels_mutex);

		/* NOTE: these are source channels, not audio channels */
		for (uint32_t i = 0; i < MAX_CHANNELS; i++) {
			obs_source_t *channel_source = view->channels[i];
			if (!channel_source)
				continue;
			if (!obs_source_active(channel_source))
				continue;
			if (obs_source_removed(channel_source))
				continue;

			/* first, add top-level sources as root_nodes */
			if (mix->mix_audio)
				da_push_back(audio->root_nodes, &channel_source);

			/* Build audio tree, tag duplicate individual sources */
			obs_source_enum_active_tree(channel_source, push_audio_tree2, audio);

			/* add top-level sources to audio tree */
			push_audio_tree(NULL, channel_source, audio);
		}
		pthread_mutex_unlock(&view->channels_mutex);
	}
	pthread_mutex_unlock(&obs->video.mixes_mutex);

	pthread_mutex_lock(&data->audio_sources_mutex);

	source = data->first_audio_source;
	while (source) {
		if (!obs_source_removed(source)) {
			push_audio_tree(NULL, source, audio);
		}
		source = (struct obs_source *)source->next_audio_source;
	}

	pthread_mutex_unlock(&data->audio_sources_mutex);

	/* ------------------------------------------------ */
	/* render audio data */
	for (size_t i = 0; i < audio->render_order.num; i++) {
		obs_source_t *render_source = audio->render_order.array[i];
		obs_source_audio_render(render_source, mixers, channels, sample_rate, audio_size);
		if (should_silence_monitored_source(render_source, audio))
			clear_audio_output_buf(render_source, audio);

		/* if a source has gone backward in time and we can no
		 * longer buffer, drop some or all of its audio */
		bool rerender = false;
		pthread_mutex_lock(&render_source->audio_buf_mutex);
		if (audio_buffering_maxed(audio) && render_source->audio_ts != 0 &&
		    render_source->audio_ts < ts.start) {
			if (render_source->info.audio_render) {
				blog(LOG_DEBUG,
				     "render audio source %s timestamp has "
				     "gone backwards",
				     obs_source_get_name(render_source));

				/* just avoid further damage */
				render_source->audio_pending = true;
#if DEBUG_AUDIO == 1
				/* this should really be fixed */
				assert(false);
#endif
			} else {
				rerender = ignore_audio(render_source, channels, sample_rate, ts.start);
			}
		}
		pthread_mutex_unlock(&render_source->audio_buf_mutex);

		/* Rendering can call plugins; release the buffer lock first. */
		if (rerender)
			obs_source_audio_render(render_source, mixers, channels, sample_rate, audio_size);
	}

	/* ------------------------------------------------ */
	/* get minimum audio timestamp */
	pthread_mutex_lock(&data->audio_sources_mutex);
	const char *buffering_name = calc_min_ts(data, sample_rate, &min_ts);
	pthread_mutex_unlock(&data->audio_sources_mutex);

	/* ------------------------------------------------ */
	/* if a source has gone backward in time, buffer    */
	switch (obs_audio_min_ts_buffering(audio->fixed_buffer, audio->fixed_buffer && audio_buffering_maxed(audio),
					   min_ts, ts.start)) {
	case OBS_AUDIO_BUFFERING_FIXED:
		set_fixed_audio_buffering(audio, sample_rate, &ts);
		break;
	case OBS_AUDIO_BUFFERING_ADD:
		add_audio_buffering(audio, sample_rate, &ts, min_ts, buffering_name);
		break;
	case OBS_AUDIO_BUFFERING_NONE:
		break;
	}

	/* ------------------------------------------------ */
	/* mix audio */
	if (!audio->buffering_wait_ticks) {
		for (size_t i = 0; i < audio->root_nodes.num; i++) {
			obs_source_t *root_source = audio->root_nodes.array[i];

			if (root_source->audio_pending)
				continue;

			pthread_mutex_lock(&root_source->audio_buf_mutex);

			if (root_source->audio_output_buf[0][0] && root_source->audio_ts)
				mix_audio(mixes, root_source, channels, sample_rate, &ts);

			pthread_mutex_unlock(&root_source->audio_buf_mutex);
		}
	}

	/* ------------------------------------------------ */
	/* discard audio */
	pthread_mutex_lock(&data->audio_sources_mutex);

	source = data->first_audio_source;
	while (source) {
		pthread_mutex_lock(&source->audio_buf_mutex);
		discard_audio(audio, source, channels, sample_rate, &ts);
		pthread_mutex_unlock(&source->audio_buf_mutex);

		source = (struct obs_source *)source->next_audio_source;
	}

	pthread_mutex_unlock(&data->audio_sources_mutex);

	/* ------------------------------------------------ */
	/* release audio sources */
	release_audio_sources(audio);

	deque_pop_front(&audio->buffered_timestamps, NULL, sizeof(ts));

	*out_ts = ts.start;

	if (audio->buffering_wait_ticks) {
		audio->buffering_wait_ticks--;
		return false;
	}

	execute_audio_tasks();

	UNUSED_PARAMETER(param);
	return true;
}

/* File-local macros (BUILD.md): out of the next file in the unity group. */
#undef DEBUG_AUDIO
#undef DEBUG_LAGGED_AUDIO
