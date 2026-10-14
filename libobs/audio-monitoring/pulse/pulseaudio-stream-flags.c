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

#include <string.h>

#include "pulseaudio-stream-flags.h"

pa_stream_flags_t pulseaudio_monitor_stream_flags(const char *device_id)
{
	pa_stream_flags_t flags = PA_STREAM_INTERPOLATE_TIMING | PA_STREAM_AUTO_TIMING_UPDATE | PA_STREAM_START_CORKED;

	/* "default" is connected to the default sink's name, and must follow
	 * it when the default changes. A sink the user chose must not be
	 * swapped for the new default behind their back. */
	if (!device_id || strcmp(device_id, "default") != 0)
		flags |= PA_STREAM_DONT_MOVE;

	return flags;
}

bool pulseaudio_monitor_reset_on_sink_event(pa_subscription_event_type_t event, uint32_t event_idx, uint32_t sink_idx,
					    bool fallback)
{
	if ((event & PA_SUBSCRIPTION_EVENT_FACILITY_MASK) != PA_SUBSCRIPTION_EVENT_SINK)
		return false;

	switch (event & PA_SUBSCRIPTION_EVENT_TYPE_MASK) {
	case PA_SUBSCRIPTION_EVENT_REMOVE:
		/* The fallback stream is movable; the server moves it. */
		return !fallback && sink_idx != PA_INVALID_INDEX && event_idx == sink_idx;
	case PA_SUBSCRIPTION_EVENT_NEW:
		return fallback;
	default:
		return false;
	}
}

uint64_t pulseaudio_monitor_backlog_excess(struct pulseaudio_monitor_backlog *backlog, uint64_t now_ns,
					   uint64_t queued_usec, uint64_t target_usec)
{
	if (!backlog->window_start) {
		backlog->window_start = now_ns;
		backlog->window_min = queued_usec;
		return 0;
	}

	if (queued_usec < backlog->window_min)
		backlog->window_min = queued_usec;
	if (now_ns - backlog->window_start < PULSEAUDIO_MONITOR_BACKLOG_WINDOW)
		return 0;

	/* The window is over: the queue never got below window_min in it.
	 * The next sample starts a new window. */
	uint64_t min = backlog->window_min;
	backlog->window_start = 0;

	uint64_t slack = target_usec > PULSEAUDIO_MONITOR_BACKLOG_SLACK ? target_usec
									: PULSEAUDIO_MONITOR_BACKLOG_SLACK;
	if (min <= target_usec + slack)
		return 0;

	/* queued_usec is at least min, so the target is left. */
	return min - target_usec;
}
