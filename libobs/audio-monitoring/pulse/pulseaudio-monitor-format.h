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

#include <pulse/channelmap.h>
#include <pulse/sample.h>

#include <media-io/audio-io.h>

/* The monitoring stream's sample format, channel map and volume, the
 * arithmetic of catching up after a stall, and the feedback-loop device
 * match, kept free of context and stream calls so they can be tested on
 * their own. */

/* The speaker layout of a sink with this many channels: SPEAKERS_UNKNOWN for
 * 0, 7 and more than 8. */
enum speaker_layout pulseaudio_channels_to_obs_speakers(uint_fast32_t channels);

/* The OBS sample format of a sink format: AUDIO_FORMAT_UNKNOWN for anything
 * but U8, S16LE, S32LE and FLOAT32LE. */
enum audio_format pulseaudio_to_obs_audio_format(pa_sample_format_t format);

/* The channel map for a speaker layout; 0 channels for SPEAKERS_UNKNOWN and
 * layouts the monitor does not play. map[] is always filled up to map[7]. */
pa_channel_map pulseaudio_channel_map(enum speaker_layout layout);

/* What the monitor plays on a sink with this spec: a format OBS cannot
 * produce plays as FLOAT32LE, and a channel count with no speaker layout as
 * 2 channels. */
pa_sample_format_t pulseaudio_monitor_sink_format(pa_sample_format_t format);
uint8_t pulseaudio_monitor_sink_channels(uint8_t channels);

/* Applies a source's volume to resample_frames * channels interleaved
 * samples of format in resample_data[0]. Other formats are left unchanged. */
void pulseaudio_monitor_process_volume(pa_sample_format_t format, uint_fast8_t channels, float vol,
				       uint8_t *const *resample_data, uint32_t resample_frames);

/* How much audio is queued for the stream, in bytes: what the server holds
 * (write_index - read_index, less what it has played in played_usec since
 * that timing update; 0 if that is negative) plus unwritten, what is not
 * written to the server yet. played_usec is 0 when the stream is not
 * playing. */
uint64_t pulseaudio_monitor_queued_bytes(int64_t write_index, int64_t read_index, uint64_t played_usec,
					 uint64_t bytes_per_sec, size_t unwritten);

/* Where excess_usec of queued audio (pulseaudio_monitor_backlog_excess()) is
 * dropped. It is rounded down to whole frames (bytes_per_frame is never 0),
 * dropped first from what is not written yet (unwritten, at most that
 * much), and the rest (rewind) by writing the next data that much earlier in
 * the server's buffer. */
struct pulseaudio_monitor_drop {
	size_t unwritten;
	size_t rewind;
};

struct pulseaudio_monitor_drop pulseaudio_monitor_drop_split(uint64_t excess_usec, uint64_t bytes_per_sec,
							     size_t bytes_per_frame, size_t unwritten);

/* Whether a sound source (id1) is the .monitor device for the selected
 * monitoring output (id2). "default" on either side stands for default_sink,
 * the default sink's name ("" if there is none). It must not be NULL when
 * either id is "default"; the caller looks it up only then. */
bool pulseaudio_monitor_devices_match(const char *id1, const char *id2, const char *default_sink);
