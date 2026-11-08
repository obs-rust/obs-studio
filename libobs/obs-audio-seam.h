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

#pragma once

#include "obs-internal.h"
#include "obs-audio-discard.h"
#include "obs-audio-mix.h"

/* Private adapter, not a new public API: the seam between the audio tick
 * (obs-audio.c) and the fields it reads and changes in struct obs_core and
 * struct obs_source (obs-audio-seam.c).
 *
 * - The core audio, obs->audio: the output; render_order and root_nodes
 *   (num, get, find, push and clear: da_find(), da_push_back() and
 *   da_resize(.., 0)); buffered_timestamps (deque_push_back(),
 *   deque_peek_front() and deque_pop_front() of one struct ts_info);
 *   obs_audio_seam_get_buffering(), the struct obs_audio_buffering
 *   obs-audio.c's audio_buffering() fills; fixed_buffer; buffering_wait_ticks
 *   get and set; monitoring_duplicating_source; and the task queue (lock and
 *   unlock of task_mutex, the queue size in bytes and pop front of one struct
 *   obs_task_info).
 * - The core data and video: first_audio_source with lock and unlock of
 *   obs->data.audio_sources_mutex; lock and unlock of
 *   obs->video.mixes_mutex, the mix count and a mix's view and mix_audio; and
 *   lock and unlock of a view's channels_mutex with a channel (a source
 *   channel, 0 to MAX_CHANNELS - 1, not an audio channel).
 * - A source: next_audio_source; lock and unlock of audio_buf_mutex;
 *   audio_is_duplicated and audio_pending, get and set; audio_ts; the size of
 *   audio_input_buf[0]; obs_audio_seam_source_get_input(), the struct
 *   obs_audio_input obs-audio.c's audio_input() fills; info.type,
 *   info.output_flags and whether info.audio_render is set; audio_output_buf
 *   (the array itself, MAX_AUDIO_MIXES rows of MAX_AUDIO_CHANNELS
 *   buffers: the type obs_audio_mix_add() takes); muted, volume and
 *   audio_mixers; and monitoring_enabled, an atomic load.
 *
 * Each is a single read, write, lock or container call, copied from the line
 * of obs-audio.c it wraps. Each lock and unlock is one pthread_mutex_*()
 * call returning its result. None checks its arguments or takes a lock of
 * its own: a caller takes the same locks around them as obs-audio.c does
 * today. That is audio_sources_mutex around the walks of the audio source
 * list, mixes_mutex around the mixes and channels_mutex inside it around a
 * view's channels, audio_buf_mutex (inside audio_sources_mutex when both are
 * held) around a source's audio state, and task_mutex around the task queue.
 * render_order, root_nodes, buffered_timestamps and the buffering fields
 * belong to the audio thread and take no lock.
 *
 * Hidden in libobs: not EXPORT. */

/* One mix's row of struct obs_source's audio_output_buf: a pointer to it is
 * the array itself, as obs_audio_mix_add() takes it. */
typedef float *obs_audio_seam_output_row_t[MAX_AUDIO_CHANNELS];

/* The core audio. */
audio_t *obs_audio_seam_get_output(void);
size_t obs_audio_seam_render_order_num(void);
obs_source_t *obs_audio_seam_render_order_get(size_t idx);
size_t obs_audio_seam_render_order_find(obs_source_t *source);
void obs_audio_seam_render_order_push(obs_source_t *source);
void obs_audio_seam_render_order_clear(void);
size_t obs_audio_seam_root_nodes_num(void);
obs_source_t *obs_audio_seam_root_nodes_get(size_t idx);
void obs_audio_seam_root_nodes_push(obs_source_t *source);
void obs_audio_seam_root_nodes_clear(void);
void obs_audio_seam_buffered_timestamps_push_back(const struct ts_info *ts);
void obs_audio_seam_buffered_timestamps_peek_front(struct ts_info *ts);
void obs_audio_seam_buffered_timestamps_pop_front(void);
struct obs_audio_buffering obs_audio_seam_get_buffering(void);
bool obs_audio_seam_get_fixed_buffer(void);
uint64_t obs_audio_seam_get_buffering_wait_ticks(void);
void obs_audio_seam_set_buffering_wait_ticks(uint64_t value);
obs_source_t *obs_audio_seam_get_monitoring_duplicating_source(void);
int obs_audio_seam_lock_tasks(void);
int obs_audio_seam_unlock_tasks(void);
size_t obs_audio_seam_tasks_size(void);
void obs_audio_seam_tasks_pop_front(struct obs_task_info *info);

/* The core data and video. */
obs_source_t *obs_audio_seam_get_first_audio_source(void);
int obs_audio_seam_lock_audio_sources(void);
int obs_audio_seam_unlock_audio_sources(void);
int obs_audio_seam_lock_mixes(void);
int obs_audio_seam_unlock_mixes(void);
size_t obs_audio_seam_mixes_num(void);
struct obs_view *obs_audio_seam_mix_get_view(size_t idx);
bool obs_audio_seam_mix_get_mix_audio(size_t idx);
int obs_audio_seam_view_lock_channels(struct obs_view *view);
int obs_audio_seam_view_unlock_channels(struct obs_view *view);
obs_source_t *obs_audio_seam_view_get_channel(const struct obs_view *view, uint32_t idx);

/* A source. */
obs_source_t *obs_audio_seam_source_get_next(const obs_source_t *source);
int obs_audio_seam_source_lock_buf(obs_source_t *source);
int obs_audio_seam_source_unlock_buf(obs_source_t *source);
bool obs_audio_seam_source_get_is_duplicated(const obs_source_t *source);
void obs_audio_seam_source_set_is_duplicated(obs_source_t *source, bool value);
bool obs_audio_seam_source_get_pending(const obs_source_t *source);
void obs_audio_seam_source_set_pending(obs_source_t *source, bool value);
uint64_t obs_audio_seam_source_get_audio_ts(const obs_source_t *source);
size_t obs_audio_seam_source_get_input_size(const obs_source_t *source);
struct obs_audio_input obs_audio_seam_source_get_input(obs_source_t *source);
enum obs_source_type obs_audio_seam_source_get_type(const obs_source_t *source);
uint32_t obs_audio_seam_source_get_output_flags(const obs_source_t *source);
bool obs_audio_seam_source_has_audio_render(const obs_source_t *source);
obs_audio_seam_output_row_t *obs_audio_seam_source_get_output_buf(obs_source_t *source);
bool obs_audio_seam_source_get_muted(const obs_source_t *source);
float obs_audio_seam_source_get_volume(const obs_source_t *source);
uint32_t obs_audio_seam_source_get_audio_mixers(const obs_source_t *source);
bool obs_audio_seam_source_get_monitoring_enabled(obs_source_t *source);
