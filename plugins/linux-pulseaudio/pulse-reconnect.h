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

#include <pulse/introspect.h>

/* Decisions for recovering from a lost server connection, kept free of
 * mainloop and context calls so they can be tested on their own. */

#define PULSE_RECONNECT_MAX_DELAY_USEC (10 * PA_USEC_PER_SEC)

struct pulse_reconnect_state {
	/* Only a connection that worked once is retried; if there never was a
	 * server, fail as before. */
	bool was_ready;
	bool reconnecting;
	uint32_t attempts;
};

/**
 * The context failed. Returns true if it should be replaced after
 * *delay_usec.
 */
bool pulse_reconnect_on_failed(struct pulse_reconnect_state *state, uint64_t *delay_usec);

/**
 * The context is ready. Returns true if it replaced a lost one, so sources
 * must recreate their streams.
 */
bool pulse_reconnect_on_ready(struct pulse_reconnect_state *state);

/**
 * The default device to record, allocated with bmalloc, or NULL if the
 * server has none (e.g. right after it restarted).
 */
char *pulse_default_device(const pa_server_info *info, bool input);

/* After the server comes back its devices reappear over the next moments;
 * sources retry starting their stream until theirs is there. */
#define PULSE_RESTART_RETRY_USEC (100 * 1000)
#define PULSE_RESTART_MAX_ATTEMPTS 100

/**
 * Count a failed stream restart. Returns true if it should be retried after
 * PULSE_RESTART_RETRY_USEC.
 */
bool pulse_restart_should_retry(uint32_t *attempts);

/* How an attempt to start a source's stream ended. */
enum pulse_start_result {
	PULSE_START_OK,
	/* No connection to the server: the reconnection restarts the source. */
	PULSE_START_NO_SERVER,
	/* The device is not there: the source waits for one to appear. */
	PULSE_START_NO_DEVICE,
	/* Anything else, e.g. the stream could not be created or connected,
	 * while the device is there. */
	PULSE_START_FAILED,
};

/**
 * A restart of the source's stream ended with result. Returns true if it
 * should be retried after PULSE_RESTART_RETRY_USEC; failures count towards
 * PULSE_RESTART_MAX_ATTEMPTS in *attempts. after_reconnect: the server just
 * came back and its devices reappear over the next moments, so a missing
 * device is retried too. Otherwise only PULSE_START_FAILED is retried: a
 * missing device is waited for (PULSE_SOURCE_NEW), so a device that is gone
 * for good is not polled.
 */
bool pulse_start_should_retry(enum pulse_start_result result, bool after_reconnect, uint32_t *attempts);

/**
 * The source's stream failed while the server stayed up. Returns true if it
 * should be restarted after *delay_usec: at once the first time, then after
 * PULSE_RESTART_RETRY_USEC, so a server that keeps failing the stream is not
 * hammered. Returns false after PULSE_RESTART_MAX_ATTEMPTS failures; the
 * source then waits for a device to appear. *attempts is shared with
 * pulse_start_should_retry() and reset once the stream delivers audio.
 */
bool pulse_stream_failed_should_restart(uint32_t *attempts, uint64_t *delay_usec);

/* A source's own record stream can die while the connection to the server
 * stays up: the server kills it, or its device goes away and comes back
 * (console switch, USB replug; PulseAudio kills the stream, pipewire-pulse
 * leaves a stream that may not move unlinked). These decide what the source
 * does about it. Each event leads to at most one start attempt, so a device
 * that is gone for good is not polled. A failure while the device is there is
 * retried with pulse_start_should_retry() and
 * pulse_stream_failed_should_restart(). */

enum pulse_stream_event {
	/* The stream failed or was terminated while the shared connection
	 * stayed up; the stream's own connection may be gone with it. */
	PULSE_STREAM_FAILED,
	/* A source (device) appeared on the server. */
	PULSE_SOURCE_NEW,
	/* The source with the given index went away. */
	PULSE_SOURCE_REMOVED,
	/* The server changed, e.g. its default sink or source. */
	PULSE_SERVER_CHANGED,
};

enum pulse_stream_action {
	PULSE_STREAM_KEEP,
	/* Drop the stream and wait for the device to return. */
	PULSE_STREAM_STOP,
	/* Create the stream again. */
	PULSE_STREAM_RESTART,
};

struct pulse_stream_status {
	/* The source has a stream ... */
	bool has_stream;
	/* ... that failed or was terminated. */
	bool failed;
	/* The source is set to the default device. Its stream is connected
	 * to the device the default was when it started, and may not be moved
	 * by the server: PulseAudio moves record streams to the default
	 * source, which for a sink monitor is the microphone. The source
	 * restarts it on the new default instead (PULSE_SERVER_CHANGED). */
	bool follows_default;
	/* PULSE_SERVER_CHANGED: the server's default device is not the one
	 * the source records from (or was going to). */
	bool default_changed;
	/* Index of the device the stream records from, or PA_INVALID_INDEX. */
	uint32_t device_index;
};

enum pulse_stream_action pulse_stream_on_event(const struct pulse_stream_status *status, enum pulse_stream_event event,
					       uint32_t index);
