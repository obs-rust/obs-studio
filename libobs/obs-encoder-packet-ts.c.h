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

#include "libobs/build/libobs-ported-encoder-packet-ts.pch.h"

/* do_encode(): pkt.timebase_num and pkt.timebase_den. */
struct obs_encoder_packet_timebase obs_encoder_packet_ts_timebase(uint32_t timebase_num, uint32_t timebase_den,
								  uint32_t frame_rate_divisor)
{
	struct obs_encoder_packet_timebase timebase;

	timebase.num = (int32_t)(timebase_num * frame_rate_divisor);
	timebase.den = (int32_t)timebase_den;
	return timebase;
}

/* packet_dts_usec() (obs-internal.h), on the packet's values. */
int64_t obs_encoder_packet_ts_dts_usec(int64_t dts, int32_t timebase_den)
{
	return dts * 1000000 / timebase_den;
}

/* send_off_encoder_packet(): pkt->dts_usec. */
int64_t obs_encoder_packet_ts_system(uint64_t start_ts, int64_t dts_usec, int64_t offset_usec)
{
	/* As in C's expression: the unsigned start time makes the sum unsigned. */
	return (int64_t)(start_ts / 1000 + (uint64_t)dts_usec - (uint64_t)offset_usec);
}

/* send_off_encoder_packet(): pkt->sys_dts_usec after the pause offset. */
int64_t obs_encoder_packet_ts_paused(int64_t sys_dts_usec, uint64_t pause_ts_offset)
{
	return (int64_t)((uint64_t)sys_dts_usec + pause_ts_offset / 1000);
}

/* send_off_encoder_packet(): the walk over encoder_packet_times, from the
 * newest entry. */
size_t obs_encoder_packet_ts_find(const struct encoder_packet_time *times, size_t num, int64_t pts)
{
	for (size_t i = num; i > 0; i--) {
		if (times[i - 1].pts == pts)
			return i - 1;
	}
	return OBS_ENCODER_PACKET_TS_NOT_FOUND;
}
