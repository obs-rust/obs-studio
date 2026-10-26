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

#include "libobs/build/libobs-ported-pulse-monitor.pch.h"

bool pulseaudio_monitor_grow(size_t queued, uint32_t tlength)
{
	// If we have grown a large buffer internally, grow the pulse buffer to match so we can write our data out.
	return queued > tlength * 2;
}

uint32_t pulseaudio_monitor_grown_tlength(size_t queued)
{
	return (uint32_t)queued;
}

bool pulseaudio_monitor_uncork(size_t queued, uint32_t tlength)
{
	// Buffer up enough data before we start playing.
	return queued >= tlength;
}

bool pulseaudio_monitor_write_more(size_t queued)
{
	return queued > 0;
}

bool pulseaudio_monitor_write_short(size_t asked, size_t queued)
{
	// PA may request we submit more or less data than we have.
	// Wait for more data if we cannot perform a full write.
	return asked > queued;
}

bool pulseaudio_monitor_playback_idle(long activate_refs)
{
	return activate_refs == 0;
}

size_t pulseaudio_monitor_playback_bytes(uint_fast32_t bytes_per_frame, uint32_t frames)
{
	return bytes_per_frame * frames;
}

bool pulseaudio_monitor_needs_volume(float vol)
{
	return !close_float(vol, 1.0f, EPSILON);
}

enum pulseaudio_monitor_sink_result pulseaudio_monitor_sink_result(int eol)
{
	// An error occurred
	if (eol < 0)
		return PULSEAUDIO_MONITOR_SINK_ERROR;
	// Terminating call for multi instance callbacks
	if (eol > 0)
		return PULSEAUDIO_MONITOR_SINK_END;
	return PULSEAUDIO_MONITOR_SINK_INFO;
}

bool pulseaudio_monitor_has_device(const char *id)
{
	return id != NULL;
}

bool pulseaudio_monitor_self_monitor_check(uint32_t output_flags)
{
	return (output_flags & PULSEAUDIO_MONITOR_DO_NOT_SELF_MONITOR) != 0;
}

bool pulseaudio_monitor_is_default(const char *id)
{
	return strcmp(id, "default") == 0;
}

bool pulseaudio_monitor_fallback(pa_sample_format_t format, const char *id)
{
	/* The chosen sink is gone (unplugged); play on the default sink
	 * until it returns, as the server would have moved us there. */
	return format == PA_SAMPLE_INVALID && strcmp(id, "default") != 0;
}

bool pulseaudio_monitor_sink_named(const char *device)
{
	return device && *device;
}

bool pulseaudio_monitor_format_invalid(pa_sample_format_t format)
{
	return format == PA_SAMPLE_INVALID;
}

pa_usec_t pulseaudio_monitor_tlength_usec(void)
{
	return 25000;
}

pa_stream_flags_t pulseaudio_monitor_open_flags(bool fallback, const char *id)
{
	return pulseaudio_monitor_stream_flags(fallback ? "default" : id);
}

bool pulseaudio_monitor_event_queued(pa_subscription_event_type_t event)
{
	pa_subscription_event_type_t type = event & PA_SUBSCRIPTION_EVENT_TYPE_MASK;
	return type == PA_SUBSCRIPTION_EVENT_NEW || type == PA_SUBSCRIPTION_EVENT_REMOVE;
}

bool pulseaudio_monitor_context_good(pa_context_state_t state)
{
	return PA_CONTEXT_IS_GOOD(state);
}

bool pulseaudio_monitor_last(size_t num)
{
	return num == 0;
}
