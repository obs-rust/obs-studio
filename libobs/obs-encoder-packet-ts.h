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

#include <stddef.h>
#include <stdint.h>

/* The packet timestamp decisions of obs-encoder.c, on plain values
 * (obs-encoder-packet-ts.c). They run for each packet of each encoder:
 * - obs_encoder_packet_ts_timebase(): do_encode() and the GPU encode thread,
 *   the packet's timebase from the encoder's and its frame rate divisor;
 * - obs_encoder_packet_ts_dts_usec(): packet_dts_usec(), a packet's DTS in
 *   microseconds, which send_off_encoder_packet() takes as the encoder's
 *   offset_usec on the first received packet;
 * - obs_encoder_packet_ts_system(): send_off_encoder_packet(), the packet's
 *   dts_usec on the system clock (from start_ts, less the first packet's
 *   offset);
 * - obs_encoder_packet_ts_paused(): send_off_encoder_packet(), sys_dts_usec,
 *   dts_usec plus the pause offset (read under pause.mutex);
 * - obs_encoder_packet_ts_find(): send_off_encoder_packet(), the newest
 *   timing entry of a video packet's PTS.
 *
 * The arithmetic is C's: the unsigned start time and pause offset make the
 * sums unsigned, so they wrap, and the result is stored as int64_t.
 * dts * 1000000 beyond int64_t, a timebase_den of 0 and INT64_MIN / -1 are
 * undefined in C.
 *
 * obs-encoder.c keeps the encode callbacks, os_gettime_ns() and the timing
 * entries it adds, pause.mutex, callbacks_mutex and the callback walk, the
 * darray (it removes the entry found), the first keyframe test, the frame
 * count, full_stop() and the logs. Nothing here takes a lock, calls into
 * libobs or keeps state.
 *
 * Hidden in libobs: not EXPORT. */

struct encoder_packet_time;

/* find() found no entry: DARRAY_INVALID. */
#define OBS_ENCODER_PACKET_TS_NOT_FOUND ((size_t)-1)

/* The timebase of struct encoder_packet (int32_t fields). */
struct obs_encoder_packet_timebase {
	/* timebase_num * frame_rate_divisor, as uint32_t, stored as int32_t. */
	int32_t num;
	/* timebase_den, stored as int32_t. */
	int32_t den;
};

/* do_encode() and the GPU encode thread: the encoder's timebase_num,
 * timebase_den and frame_rate_divisor. */
struct obs_encoder_packet_timebase obs_encoder_packet_ts_timebase(uint32_t timebase_num, uint32_t timebase_den,
								  uint32_t frame_rate_divisor);

/* packet_dts_usec(): dts * 1000000 / timebase_den. */
int64_t obs_encoder_packet_ts_dts_usec(int64_t dts, int32_t timebase_den);

/* send_off_encoder_packet(): start_ts / 1000 + dts_usec - offset_usec, where
 * start_ts is the encoder's start time in ns, dts_usec the packet's DTS in
 * microseconds and offset_usec the first received packet's. */
int64_t obs_encoder_packet_ts_system(uint64_t start_ts, int64_t dts_usec, int64_t offset_usec);

/* send_off_encoder_packet(): sys_dts_usec + pause_ts_offset / 1000, where
 * pause_ts_offset is the encoder's pause.ts_offset in ns. */
int64_t obs_encoder_packet_ts_paused(int64_t sys_dts_usec, uint64_t pause_ts_offset);

/* send_off_encoder_packet(): the index of the newest of the num entries with
 * this pts, or OBS_ENCODER_PACKET_TS_NOT_FOUND. times may be NULL when num is
 * 0. */
size_t obs_encoder_packet_ts_find(const struct encoder_packet_time *times, size_t num, int64_t pts);
