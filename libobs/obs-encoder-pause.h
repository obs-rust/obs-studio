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

#include <stdbool.h>
#include <stdint.h>

/* The pause checks of obs-encoder.c, on plain values
 * (obs-encoder-pause.c). One function per decision, each asked where the C
 * made it, from values the caller reads at that point, under the pause
 * mutex the caller holds:
 * - video_pause_check(), after the last_video_ts store:
 *   obs_encoder_pause_off() (no pause, the frame passes), then
 *   obs_encoder_pause_video_end() (the frame ends the pause and passes)
 *   and, for a frame that does not, obs_encoder_pause_video_drop();
 * - audio_pause_check(): obs_encoder_pause_off(), obs_encoder_pause_audio_end()
 *   (the buffer's end), obs_encoder_pause_audio_start() (where the pause
 *   start falls against the buffer) and, for a buffer after the start,
 *   obs_encoder_pause_audio_paused() (where the pause end falls);
 *   obs_encoder_pause_audio_cut_ns() for the cut at the start
 *   (pause_audio()) or the end (unpause_audio()), then
 *   obs_encoder_pause_audio_kept() or obs_encoder_pause_audio_left(), the
 *   frames the buffer keeps, and obs_encoder_pause_audio_drop() (a buffer
 *   with no frame left is dropped).
 *
 * obs-encoder.c keeps every lock and unlock of pause->mutex, the
 * last_video_ts store, every read of the pause record and the buffer (each
 * where it was), both conversions that divide by the sample rate or by
 * 1000000000 (audio_frames_to_ns() for the buffer's length and
 * ns_to_audio_frames() for each cut; util_mul_div64() traps on a zero rate
 * and on a quotient above 64 bits on MSVC x64), every write to ts_start,
 * ts_end and the buffer's frames and timestamp, the data pointer advance of
 * each plane, the ts_offset subtraction, pause_reset() and every caller.
 * It passes their results here. Nothing here takes a lock, calls into
 * libobs or keeps state.
 *
 * Hidden in libobs: not EXPORT. */

/* obs_encoder_pause_audio_start(): the pause starts after the buffer, so it
 * passes. */
#define OBS_ENCODER_PAUSE_AUDIO_START_AFTER 0
/* obs_encoder_pause_audio_start(): the pause starts inside the buffer (or at
 * either end), so the buffer is cut there (pause_audio()). */
#define OBS_ENCODER_PAUSE_AUDIO_START_INSIDE 1
/* obs_encoder_pause_audio_start(): the pause started before the buffer, so
 * obs_encoder_pause_audio_paused() decides. */
#define OBS_ENCODER_PAUSE_AUDIO_START_BEFORE 2

/* obs_encoder_pause_audio_paused(): the buffer is dropped. */
#define OBS_ENCODER_PAUSE_AUDIO_PAUSED_DROP 0
/* obs_encoder_pause_audio_paused(): the pause ends inside the buffer (or at
 * either end), so the buffer is cut there and the pause ends
 * (unpause_audio()). */
#define OBS_ENCODER_PAUSE_AUDIO_PAUSED_UNPAUSE 1
/* obs_encoder_pause_audio_paused(): the pause ended before the buffer, so the
 * pause ends (ts_start and ts_end are cleared) and the buffer passes as it
 * is. Only the Rust port (rust/obs-output, encoder_pause) answers it; the C
 * here drops such a buffer. */
#define OBS_ENCODER_PAUSE_AUDIO_PAUSED_RESUME 2

/* video_pause_check() and audio_pause_check(): no pause is on (ts_start is
 * 0), so the frame or buffer passes. */
bool obs_encoder_pause_off(uint64_t ts_start);

/* video_pause_check(), for a pause that is on: the frame stamped ts ends
 * the pause (ts_start and ts_end are cleared) and passes, from the pause end
 * (0 until the pause ends). The C here ends the pause only on a frame stamped
 * ts_end; the Rust port also on the first frame after a set ts_end. */
bool obs_encoder_pause_video_end(uint64_t ts_end, uint64_t ts);

/* video_pause_check(), for a frame that does not end the pause: the frame is
 * dropped, at or after the pause start. */
bool obs_encoder_pause_video_drop(uint64_t ts_start, uint64_t ts);

/* audio_pause_check(): the buffer's end, timestamp + duration_ns
 * (audio_frames_to_ns(sample_rate, frames)), a uint64_t sum. */
uint64_t obs_encoder_pause_audio_end(uint64_t timestamp, uint64_t duration_ns);

/* audio_pause_check(), for a pause that is on: where its start falls against
 * the buffer from timestamp to end_ts. One of
 * OBS_ENCODER_PAUSE_AUDIO_START_*. */
int obs_encoder_pause_audio_start(uint64_t ts_start, uint64_t timestamp, uint64_t end_ts);

/* audio_pause_check(), for a buffer after the pause start: what the buffer
 * from timestamp to end_ts does, from the pause end (0 until the pause
 * ends). One of OBS_ENCODER_PAUSE_AUDIO_PAUSED_*. */
int obs_encoder_pause_audio_paused(uint64_t ts_end, uint64_t timestamp, uint64_t end_ts);

/* pause_audio() and unpause_audio(): the nanoseconds from the buffer's start
 * to the cut at ts (ts_start or ts_end), ts - timestamp. */
uint64_t obs_encoder_pause_audio_cut_ns(uint64_t ts, uint64_t timestamp);

/* pause_audio(): the frames the buffer keeps before the cut, cut_frames
 * (ns_to_audio_frames(sample_rate, cut_ns)) truncated to uint32_t. */
uint32_t obs_encoder_pause_audio_kept(uint64_t cut_frames);

/* unpause_audio(): the frames the buffer keeps after the cut,
 * frames - (uint32_t)cut_frames. */
uint32_t obs_encoder_pause_audio_left(uint32_t frames, uint64_t cut_frames);

/* audio_pause_check(), after a cut: no frame is left, so the buffer is
 * dropped. */
bool obs_encoder_pause_audio_drop(uint32_t frames);
