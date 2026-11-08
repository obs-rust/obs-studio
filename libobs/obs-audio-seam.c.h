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

/* Private adapter, not a new public API (obs-audio-seam.h). Each body is
 * copied from the line of obs-audio.c named beside it. */

/* ------------------------------------------------------------------------- */
/* The core audio */

/* audio_callback(): audio_output_get_sample_rate(audio->audio) */
audio_t *obs_audio_seam_get_output(void)
{
	return obs->audio.audio;
}

/* release_audio_sources(), audio_callback(): render_order.num */
size_t obs_audio_seam_render_order_num(void)
{
	return obs->audio.render_order.num;
}

/* push_audio_tree2(), release_audio_sources(), audio_callback() */
obs_source_t *obs_audio_seam_render_order_get(size_t idx)
{
	return obs->audio.render_order.array[idx];
}

/* push_audio_tree(), push_audio_tree2(): da_find(.., &source, 0) */
size_t obs_audio_seam_render_order_find(obs_source_t *source)
{
	return da_find(obs->audio.render_order, &source, 0);
}

/* push_audio_tree(), push_audio_tree2(): da_push_back(.., &s) */
void obs_audio_seam_render_order_push(obs_source_t *source)
{
	da_push_back(obs->audio.render_order, &source);
}

/* audio_callback(): da_resize(audio->render_order, 0) */
void obs_audio_seam_render_order_clear(void)
{
	da_resize(obs->audio.render_order, 0);
}

/* audio_callback(): root_nodes.num */
size_t obs_audio_seam_root_nodes_num(void)
{
	return obs->audio.root_nodes.num;
}

/* audio_callback(): root_nodes.array[i] */
obs_source_t *obs_audio_seam_root_nodes_get(size_t idx)
{
	return obs->audio.root_nodes.array[idx];
}

/* push_audio_tree2(), audio_callback(): da_push_back(.., &source) */
void obs_audio_seam_root_nodes_push(obs_source_t *source)
{
	da_push_back(obs->audio.root_nodes, &source);
}

/* audio_callback(): da_resize(audio->root_nodes, 0) */
void obs_audio_seam_root_nodes_clear(void)
{
	da_resize(obs->audio.root_nodes, 0);
}

/* audio_callback(): deque_push_back(&audio->buffered_timestamps, ..) */
void obs_audio_seam_buffered_timestamps_push_back(const struct ts_info *ts)
{
	deque_push_back(&obs->audio.buffered_timestamps, ts, sizeof(*ts));
}

/* audio_callback(): deque_peek_front(&audio->buffered_timestamps, ..) */
void obs_audio_seam_buffered_timestamps_peek_front(struct ts_info *ts)
{
	deque_peek_front(&obs->audio.buffered_timestamps, ts, sizeof(*ts));
}

/* audio_callback(): deque_pop_front(&audio->buffered_timestamps, NULL, ..) */
void obs_audio_seam_buffered_timestamps_pop_front(void)
{
	deque_pop_front(&obs->audio.buffered_timestamps, NULL, sizeof(struct ts_info));
}

/* audio_buffering() */
struct obs_audio_buffering obs_audio_seam_get_buffering(void)
{
	struct obs_core_audio *audio = &obs->audio;
	struct obs_audio_buffering buffering = {
		.buffered_ts = &audio->buffered_ts,
		.buffered_timestamps = &audio->buffered_timestamps,
		.buffering_wait_ticks = &audio->buffering_wait_ticks,
		.total_buffering_ticks = &audio->total_buffering_ticks,
		.max_buffering_ticks = audio->max_buffering_ticks,
	};
	return buffering;
}

/* audio_callback(): audio->fixed_buffer */
bool obs_audio_seam_get_fixed_buffer(void)
{
	return obs->audio.fixed_buffer;
}

/* audio_callback(): audio->buffering_wait_ticks */
uint64_t obs_audio_seam_get_buffering_wait_ticks(void)
{
	return obs->audio.buffering_wait_ticks;
}

/* audio_callback(): audio->buffering_wait_ticks-- */
void obs_audio_seam_set_buffering_wait_ticks(uint64_t value)
{
	obs->audio.buffering_wait_ticks = value;
}

/* monitor_dedup(), should_silence_monitored_source(),
 * clear_audio_output_buf() */
obs_source_t *obs_audio_seam_get_monitoring_duplicating_source(void)
{
	return obs->audio.monitoring_duplicating_source;
}

/* execute_audio_tasks(): pthread_mutex_lock(&audio->task_mutex) */
int obs_audio_seam_lock_tasks(void)
{
	return pthread_mutex_lock(&obs->audio.task_mutex);
}

/* execute_audio_tasks(): pthread_mutex_unlock(&audio->task_mutex) */
int obs_audio_seam_unlock_tasks(void)
{
	return pthread_mutex_unlock(&obs->audio.task_mutex);
}

/* execute_audio_tasks(): audio->tasks.size */
size_t obs_audio_seam_tasks_size(void)
{
	return obs->audio.tasks.size;
}

/* execute_audio_tasks(): deque_pop_front(&audio->tasks, &info, ..) */
void obs_audio_seam_tasks_pop_front(struct obs_task_info *info)
{
	deque_pop_front(&obs->audio.tasks, info, sizeof(*info));
}

/* ------------------------------------------------------------------------- */
/* The core data and video */

/* find_min_ts(), mark_invalid_sources(), audio_callback() */
obs_source_t *obs_audio_seam_get_first_audio_source(void)
{
	return obs->data.first_audio_source;
}

/* audio_callback(): pthread_mutex_lock(&data->audio_sources_mutex) */
int obs_audio_seam_lock_audio_sources(void)
{
	return pthread_mutex_lock(&obs->data.audio_sources_mutex);
}

/* audio_callback(): pthread_mutex_unlock(&data->audio_sources_mutex) */
int obs_audio_seam_unlock_audio_sources(void)
{
	return pthread_mutex_unlock(&obs->data.audio_sources_mutex);
}

/* audio_callback(): pthread_mutex_lock(&obs->video.mixes_mutex) */
int obs_audio_seam_lock_mixes(void)
{
	return pthread_mutex_lock(&obs->video.mixes_mutex);
}

/* audio_callback(): pthread_mutex_unlock(&obs->video.mixes_mutex) */
int obs_audio_seam_unlock_mixes(void)
{
	return pthread_mutex_unlock(&obs->video.mixes_mutex);
}

/* audio_callback(): obs->video.mixes.num */
size_t obs_audio_seam_mixes_num(void)
{
	return obs->video.mixes.num;
}

/* audio_callback(): obs->video.mixes.array[j]->view */
struct obs_view *obs_audio_seam_mix_get_view(size_t idx)
{
	return obs->video.mixes.array[idx]->view;
}

/* audio_callback(): mix->mix_audio */
bool obs_audio_seam_mix_get_mix_audio(size_t idx)
{
	return obs->video.mixes.array[idx]->mix_audio;
}

/* audio_callback(): pthread_mutex_lock(&view->channels_mutex) */
int obs_audio_seam_view_lock_channels(struct obs_view *view)
{
	return pthread_mutex_lock(&view->channels_mutex);
}

/* audio_callback(): pthread_mutex_unlock(&view->channels_mutex) */
int obs_audio_seam_view_unlock_channels(struct obs_view *view)
{
	return pthread_mutex_unlock(&view->channels_mutex);
}

/* audio_callback(): view->channels[i] */
obs_source_t *obs_audio_seam_view_get_channel(const struct obs_view *view, uint32_t idx)
{
	return view->channels[idx];
}

/* ------------------------------------------------------------------------- */
/* A source */

/* find_min_ts(), mark_invalid_sources(), audio_callback():
 * (struct obs_source *)source->next_audio_source */
obs_source_t *obs_audio_seam_source_get_next(const obs_source_t *source)
{
	return (struct obs_source *)source->next_audio_source;
}

/* pthread_mutex_lock(&source->audio_buf_mutex) */
int obs_audio_seam_source_lock_buf(obs_source_t *source)
{
	return pthread_mutex_lock(&source->audio_buf_mutex);
}

/* pthread_mutex_unlock(&source->audio_buf_mutex) */
int obs_audio_seam_source_unlock_buf(obs_source_t *source)
{
	return pthread_mutex_unlock(&source->audio_buf_mutex);
}

/* push_audio_tree2(): s->audio_is_duplicated */
bool obs_audio_seam_source_get_is_duplicated(const obs_source_t *source)
{
	return source->audio_is_duplicated;
}

/* push_audio_tree(), push_audio_tree2(): s->audio_is_duplicated = .. */
void obs_audio_seam_source_set_is_duplicated(obs_source_t *source, bool value)
{
	source->audio_is_duplicated = value;
}

/* find_min_ts(), audio_callback(): source->audio_pending */
bool obs_audio_seam_source_get_pending(const obs_source_t *source)
{
	return source->audio_pending;
}

/* audio_callback(): render_source->audio_pending = true */
void obs_audio_seam_source_set_pending(obs_source_t *source, bool value)
{
	source->audio_pending = value;
}

/* audio_buffer_insufficient_locked(), find_min_ts(), audio_callback() */
uint64_t obs_audio_seam_source_get_audio_ts(const obs_source_t *source)
{
	return source->audio_ts;
}

/* audio_buffer_insufficient_locked(): source->audio_input_buf[0].size */
size_t obs_audio_seam_source_get_input_size(const obs_source_t *source)
{
	return source->audio_input_buf[0].size;
}

/* audio_input() */
struct obs_audio_input obs_audio_seam_source_get_input(obs_source_t *source)
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

/* push_audio_tree2(): s->info.type */
enum obs_source_type obs_audio_seam_source_get_type(const obs_source_t *source)
{
	return source->info.type;
}

/* push_audio_tree2(), discard_audio(): source->info.output_flags */
uint32_t obs_audio_seam_source_get_output_flags(const obs_source_t *source)
{
	return source->info.output_flags;
}

/* discard_audio(), audio_buffer_insufficient_locked(), audio_callback():
 * source->info.audio_render != NULL */
bool obs_audio_seam_source_has_audio_render(const obs_source_t *source)
{
	return source->info.audio_render != NULL;
}

/* mix_audio(), mix_audio_mixers(), clear_audio_output_buf(),
 * audio_callback(): source->audio_output_buf */
obs_audio_seam_output_row_t *obs_audio_seam_source_get_output_buf(obs_source_t *source)
{
	return source->audio_output_buf;
}

/* monitor_dedup(): dup_src->muted */
bool obs_audio_seam_source_get_muted(const obs_source_t *source)
{
	return source->muted;
}

/* monitor_dedup(): dup_src->volume */
float obs_audio_seam_source_get_volume(const obs_source_t *source)
{
	return source->volume;
}

/* monitor_dedup(), clear_audio_output_buf(): ->audio_mixers */
uint32_t obs_audio_seam_source_get_audio_mixers(const obs_source_t *source)
{
	return source->audio_mixers;
}

/* should_silence_monitored_source():
 * os_atomic_load_bool(&source->monitoring_enabled) */
bool obs_audio_seam_source_get_monitoring_enabled(obs_source_t *source)
{
	return os_atomic_load_bool(&source->monitoring_enabled);
}
