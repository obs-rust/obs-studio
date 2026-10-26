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
#include <stddef.h>
#include <stdint.h>

#include <pulse/def.h>
#include <pulse/sample.h>

/* The PulseAudio monitor's open and write steps of pulseaudio-output.c, on
 * plain values (pulseaudio-monitor-steps.c). One function per decision, each
 * asked where the C made it, from values the caller reads at that point:
 * - do_stream_write(): pulseaudio_monitor_grow() and _grown_tlength() (the
 *   server buffer grows to what is queued), _uncork() (a corked stream starts
 *   once a whole tlength is queued), _write_more() (the write loop) and
 *   _write_short() (the server asks for more than is queued: cancel and
 *   wait);
 * - on_audio_playback(): pulseaudio_monitor_playback_idle() (a source with no
 *   activate refs is skipped), _playback_bytes() (the bytes pushed) and
 *   _needs_volume() (the volume step runs unless the volume is 1 within
 *   EPSILON);
 * - pulseaudio_sink_info(): pulseaudio_monitor_sink_result() (an error, the
 *   end of the list, or a sink);
 * - audio_monitor_init(): pulseaudio_monitor_has_device(),
 *   _self_monitor_check() (the source asks not to be monitored into its own
 *   device), _is_default(), _fallback() (the chosen sink is gone: play on the
 *   default), _sink_named() (the default sink has a name),
 *   _format_invalid(), _tlength_usec() (25 ms) and _open_flags() (the
 *   fallback stream follows the default sink);
 * - on_sink_event(): pulseaudio_monitor_event_queued() (only new and remove
 *   events);
 * - audio_monitor_reset(): pulseaudio_monitor_context_good() (a failed
 *   replacement keeps the old stream only while its context is good);
 * - audio_monitor_destroy(): pulseaudio_monitor_last() (the last monitor
 *   stops the sink events).
 *
 * pulseaudio-output.c keeps pulseaudio_lock() and _unlock(), the playback
 * mutex (and its trylock in the capture callback), the monitoring mutex,
 * every read of the monitor, the source and the settings, every pa_* call,
 * pulseaudio_get_server_info() and _get_sink_info(), get_default_id(),
 * devices_match(), the resampler, the deque, the buffer attributes it
 * writes, the task queue, every allocation and every log. It passes their results here.
 * Nothing here takes a lock, calls into PulseAudio or libobs or keeps state.
 *
 * OBS_SOURCE_DO_NOT_SELF_MONITOR is copied below, so this header needs no
 * core header; pulseaudio-output.c checks it against obs-source.h.
 *
 * Hidden in libobs: not EXPORT. */

/* OBS_SOURCE_DO_NOT_SELF_MONITOR */
#define PULSEAUDIO_MONITOR_DO_NOT_SELF_MONITOR (1 << 9)

/* do_stream_write(): more than twice tlength is queued, so the server buffer
 * grows to match. tlength * 2 is a uint32_t product. */
bool pulseaudio_monitor_grow(size_t queued, uint32_t tlength);

/* do_stream_write(): the tlength the buffer grows to, what is queued as a
 * uint32_t (truncated). The other attributes are set to (uint32_t)-1 by the
 * caller. */
uint32_t pulseaudio_monitor_grown_tlength(size_t queued);

/* do_stream_write(), for a corked stream: a whole tlength is queued, so it is
 * uncorked; otherwise it stays corked and nothing is written. */
bool pulseaudio_monitor_uncork(size_t queued, uint32_t tlength);

/* do_stream_write(): something is queued, so the write loop runs again. */
bool pulseaudio_monitor_write_more(size_t queued);

/* do_stream_write(): the server asked for more than is queued, so the write
 * is cancelled and the stream waits for more data. */
bool pulseaudio_monitor_write_short(size_t asked, size_t queued);

/* on_audio_playback(): the source has no activate refs (loaded by the caller
 * after the trylock), so the packet is skipped. */
bool pulseaudio_monitor_playback_idle(long activate_refs);

/* on_audio_playback(): the bytes of resample_frames frames, a product in
 * uint_fast32_t (64 bits on glibc, 32 on FreeBSD), as a size_t. */
size_t pulseaudio_monitor_playback_bytes(uint_fast32_t bytes_per_frame, uint32_t frames);

/* on_audio_playback(): the volume is not 1 within EPSILON, so it is applied
 * (NaN is not close to 1). */
bool pulseaudio_monitor_needs_volume(float vol);

/* pulseaudio_sink_info(): what a sink info callback is. */
enum pulseaudio_monitor_sink_result {
	/* eol is 0: a sink. */
	PULSEAUDIO_MONITOR_SINK_INFO = 0,
	/* eol is above 0: the end of the list. */
	PULSEAUDIO_MONITOR_SINK_END = 1,
	/* eol is below 0: an error; the format becomes PA_SAMPLE_INVALID. */
	PULSEAUDIO_MONITOR_SINK_ERROR = 2,
};

enum pulseaudio_monitor_sink_result pulseaudio_monitor_sink_result(int eol);

/* audio_monitor_init(): a monitoring device is set (not NULL). */
bool pulseaudio_monitor_has_device(const char *id);

/* audio_monitor_init(): the source has OBS_SOURCE_DO_NOT_SELF_MONITOR, so its
 * device_id setting is matched against the monitoring device. */
bool pulseaudio_monitor_self_monitor_check(uint32_t output_flags);

/* audio_monitor_init(): the monitoring device is "default". id is not
 * NULL. */
bool pulseaudio_monitor_is_default(const char *id);

/* audio_monitor_init(): the chosen sink's info gave no format and the device
 * is not "default", so the stream falls back to the default sink. id is not
 * NULL; it is compared only when the format is invalid. */
bool pulseaudio_monitor_fallback(pa_sample_format_t format, const char *id);

/* audio_monitor_init(), on the fallback: the default sink's name is set and
 * not empty. */
bool pulseaudio_monitor_sink_named(const char *device);

/* audio_monitor_init(): no sink gave a format. */
bool pulseaudio_monitor_format_invalid(pa_sample_format_t format);

/* audio_monitor_init(): the stream's target buffer, in usec (25 ms), for
 * pa_usec_to_bytes(). */
pa_usec_t pulseaudio_monitor_tlength_usec(void);

/* audio_monitor_init(): the stream's flags,
 * pulseaudio_monitor_stream_flags() of "default" on the fallback and of id
 * otherwise. */
pa_stream_flags_t pulseaudio_monitor_open_flags(bool fallback, const char *id);

/* on_sink_event(): the event's type is new or remove, so it is queued. */
bool pulseaudio_monitor_event_queued(pa_subscription_event_type_t event);

/* audio_monitor_reset(): PA_CONTEXT_IS_GOOD(state), for the context of the
 * old stream after a failed replacement (the caller checks the context is
 * not NULL first). */
bool pulseaudio_monitor_context_good(pa_context_state_t state);

/* audio_monitor_destroy(): no monitor is left, so the sink events stop. */
bool pulseaudio_monitor_last(size_t num);
