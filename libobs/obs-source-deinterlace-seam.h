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

/* Private adapter, not a new public API: the seam between the deinterlace
 * (obs-source-deinterlace.c) and the async and deinterlace fields it reads
 * and changes in struct obs_source and struct obs_core
 * (obs-source-deinterlace-seam.c).
 *
 * - The async queue: the count of async_frames, the frame at an index
 *   (async_frames.array[idx]) and da_erase() of the first; lock and unlock
 *   of async_mutex, each a single pthread_mutex_*() call returning its
 *   result.
 * - The clock fields: async_unbuffered and last_sys_timestamp, get;
 *   last_frame_ts, deinterlace_offset, deinterlace_frame_ts and
 *   deinterlace_half_duration, get and set.
 * - The frame slots: cur_async_frame and prev_async_frame, get and set.
 * - The texture fields: async_format, async_trc, async_width, async_height
 *   and async_flip, get; the slots of async_textures and async_prev_textures
 *   (the start of each MAX_AV_PLANES array) and the addresses of
 *   async_texrender and async_prev_texrender, so a caller's swap and
 *   destroys write the same storage.
 * - The deinterlace fields: deinterlace_rendered and deinterlace_effect, get
 *   and set; deinterlace_mode and deinterlace_top_first, set. Their reads
 *   are obs_source_deinterlace_fields() (obs-source-deinterlace-fields.c).
 * - obs_source_deinterlace_seam_convert_video_space(): the obs-internal.h
 *   inline convert_video_space(), passed through.
 * - The core video: obs->video.video_frame_interval_ns, get; the address of
 *   obs->video.stats; the video time, obs_video_stats_frame_time() on it
 *   (a header inline, which takes the stats mutex); and the addresses of
 *   the eight obs->video.deinterlace_*_effect slots that obs_load_effect()
 *   fills.
 * - The frame fields: a queued frame's timestamp, get; its prev_frame, get
 *   and set; and os_atomic_inc_long() of its refs (a header inline),
 *   returning the count it made.
 *
 * None checks its arguments or takes a lock of its own: a caller takes
 * async_mutex where obs-source-deinterlace.c takes it today.
 * deinterlace_update_async_video() locks it around its reads and writes of
 * cur_async_frame and prev_async_frame, and enable_deinterlacing() around
 * its release of prev_async_frame. deinterlace_process_last_frame(), and
 * the frame selection under it, runs with async_mutex held (async_tick()).
 * The rest runs on the graphics thread and takes no lock, as today.
 *
 * Hidden in libobs: not EXPORT. */

size_t obs_source_deinterlace_seam_get_async_frames_num(const obs_source_t *source);
struct obs_source_frame *obs_source_deinterlace_seam_get_async_frame(const obs_source_t *source, size_t idx);
void obs_source_deinterlace_seam_erase_async_frame_front(obs_source_t *source);
int obs_source_deinterlace_seam_async_mutex_lock(obs_source_t *source);
int obs_source_deinterlace_seam_async_mutex_unlock(obs_source_t *source);

bool obs_source_deinterlace_seam_get_async_unbuffered(const obs_source_t *source);
uint64_t obs_source_deinterlace_seam_get_last_sys_timestamp(const obs_source_t *source);
uint64_t obs_source_deinterlace_seam_get_last_frame_ts(const obs_source_t *source);
void obs_source_deinterlace_seam_set_last_frame_ts(obs_source_t *source, uint64_t value);
uint64_t obs_source_deinterlace_seam_get_deinterlace_offset(const obs_source_t *source);
void obs_source_deinterlace_seam_set_deinterlace_offset(obs_source_t *source, uint64_t value);
uint64_t obs_source_deinterlace_seam_get_deinterlace_frame_ts(const obs_source_t *source);
void obs_source_deinterlace_seam_set_deinterlace_frame_ts(obs_source_t *source, uint64_t value);
uint32_t obs_source_deinterlace_seam_get_deinterlace_half_duration(const obs_source_t *source);
void obs_source_deinterlace_seam_set_deinterlace_half_duration(obs_source_t *source, uint32_t value);

struct obs_source_frame *obs_source_deinterlace_seam_get_cur_async_frame(const obs_source_t *source);
void obs_source_deinterlace_seam_set_cur_async_frame(obs_source_t *source, struct obs_source_frame *value);
struct obs_source_frame *obs_source_deinterlace_seam_get_prev_async_frame(const obs_source_t *source);
void obs_source_deinterlace_seam_set_prev_async_frame(obs_source_t *source, struct obs_source_frame *value);

enum video_format obs_source_deinterlace_seam_get_async_format(const obs_source_t *source);
uint8_t obs_source_deinterlace_seam_get_async_trc(const obs_source_t *source);
uint32_t obs_source_deinterlace_seam_get_async_width(const obs_source_t *source);
uint32_t obs_source_deinterlace_seam_get_async_height(const obs_source_t *source);
bool obs_source_deinterlace_seam_get_async_flip(const obs_source_t *source);
gs_texture_t **obs_source_deinterlace_seam_get_async_textures(obs_source_t *source);
gs_texture_t **obs_source_deinterlace_seam_get_async_prev_textures(obs_source_t *source);
gs_texrender_t **obs_source_deinterlace_seam_get_async_texrender_slot(obs_source_t *source);
gs_texrender_t **obs_source_deinterlace_seam_get_async_prev_texrender_slot(obs_source_t *source);

bool obs_source_deinterlace_seam_get_deinterlace_rendered(const obs_source_t *source);
void obs_source_deinterlace_seam_set_deinterlace_rendered(obs_source_t *source, bool value);
gs_effect_t *obs_source_deinterlace_seam_get_deinterlace_effect(const obs_source_t *source);
void obs_source_deinterlace_seam_set_deinterlace_effect(obs_source_t *source, gs_effect_t *value);
void obs_source_deinterlace_seam_set_deinterlace_mode(obs_source_t *source, enum obs_deinterlace_mode value);
void obs_source_deinterlace_seam_set_deinterlace_top_first(obs_source_t *source, bool value);

enum gs_color_space obs_source_deinterlace_seam_convert_video_space(enum video_format format, enum video_trc trc);

uint64_t obs_source_deinterlace_seam_get_video_frame_interval_ns(void);
struct obs_video_stats *obs_source_deinterlace_seam_get_video_stats(void);
gs_effect_t **obs_source_deinterlace_seam_get_discard_effect_slot(void);
gs_effect_t **obs_source_deinterlace_seam_get_discard_2x_effect_slot(void);
gs_effect_t **obs_source_deinterlace_seam_get_blend_effect_slot(void);
gs_effect_t **obs_source_deinterlace_seam_get_blend_2x_effect_slot(void);
gs_effect_t **obs_source_deinterlace_seam_get_linear_effect_slot(void);
gs_effect_t **obs_source_deinterlace_seam_get_linear_2x_effect_slot(void);
gs_effect_t **obs_source_deinterlace_seam_get_yadif_effect_slot(void);
gs_effect_t **obs_source_deinterlace_seam_get_yadif_2x_effect_slot(void);
uint64_t obs_source_deinterlace_seam_get_video_time(void);

uint64_t obs_source_deinterlace_seam_get_frame_timestamp(const struct obs_source_frame *frame);
bool obs_source_deinterlace_seam_get_frame_prev_frame(const struct obs_source_frame *frame);
void obs_source_deinterlace_seam_set_frame_prev_frame(struct obs_source_frame *frame, bool value);
long obs_source_deinterlace_seam_frame_addref(struct obs_source_frame *frame);
