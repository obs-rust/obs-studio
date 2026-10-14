/*
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
*/

#pragma once

#include <stdbool.h>
#include <stdint.h>

#include <pulse/def.h>

/* Flags for the monitoring playback stream, kept free of context and
 * stream calls so they can be tested on their own.
 *
 * device_id is the monitoring device chosen in the settings: "default" to
 * play on whatever the default sink is, or the name of one sink. */
pa_stream_flags_t pulseaudio_monitor_stream_flags(const char *device_id);

/* Whether a sink event means the monitoring stream must be reopened.
 *
 * A stream on the chosen sink cannot move, so when that sink goes away
 * (unplugged) it is reopened on the default sink (fallback), as the server
 * would have moved it there. While on the fallback, a new sink may be the
 * chosen one coming back, so it is reopened on the chosen sink if present.
 *
 * event is the subscription event (facility and type), event_idx the sink
 * index it is about, sink_idx the sink the stream plays on
 * (PA_INVALID_INDEX if none). */
bool pulseaudio_monitor_reset_on_sink_event(pa_subscription_event_type_t event, uint32_t event_idx, uint32_t sink_idx,
					    bool fallback);

/* Catching up after a stall.
 *
 * Monitored audio is written to the server as soon as OBS outputs it. When
 * playback stalls (the server or the device hiccups, an xrun, a suspend, a
 * CPU spike) while OBS keeps producing, everything produced meanwhile is
 * queued and then played in order, so the monitor stays that far behind for
 * good: the queue drains only as fast as it fills.
 *
 * Feed one sample per write: queued_usec is how long audio handed over now
 * takes to reach the sink (what the server holds plus what is not written
 * yet; the sink's own latency excluded), target_usec the buffer the stream
 * asks the server for (tlength). Returns how much queued audio to drop, in
 * microseconds: when the queue stayed more than target_usec (and
 * at least PULSEAUDIO_MONITOR_BACKLOG_SLACK) above the target for a whole
 * window, down to the target; 0 otherwise. A source that delivers its audio
 * in bursts drains the queue between bursts, and a short spike drains within
 * the window, so neither is cut. */
#define PULSEAUDIO_MONITOR_BACKLOG_WINDOW 2000000000ULL /* ns */
#define PULSEAUDIO_MONITOR_BACKLOG_SLACK 200000ULL      /* usec */

struct pulseaudio_monitor_backlog {
	uint64_t window_start; /* ns; 0 until the first sample of a window */
	uint64_t window_min;   /* usec; lowest queued_usec in the window */
};

uint64_t pulseaudio_monitor_backlog_excess(struct pulseaudio_monitor_backlog *backlog, uint64_t now_ns,
					   uint64_t queued_usec, uint64_t target_usec);
