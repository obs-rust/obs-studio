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

#include <pulse/timeval.h>

#include <util/bmem.h>

#include "pulse-reconnect.h"

bool pulse_reconnect_on_failed(struct pulse_reconnect_state *state, uint64_t *delay_usec)
{
	if (!state->was_ready)
		return false;

	/* Retry at once, then back off: a server that refuses or rejects the
	 * connection fails again right away. */
	*delay_usec = 0;
	if (state->attempts > 0) {
		*delay_usec = (PA_USEC_PER_SEC / 4) << (state->attempts < 6 ? state->attempts : 6);
		if (*delay_usec > PULSE_RECONNECT_MAX_DELAY_USEC)
			*delay_usec = PULSE_RECONNECT_MAX_DELAY_USEC;
	}
	state->attempts++;
	state->reconnecting = true;
	return true;
}

bool pulse_reconnect_on_ready(struct pulse_reconnect_state *state)
{
	bool reconnected = state->reconnecting;

	state->was_ready = true;
	state->reconnecting = false;
	state->attempts = 0;
	return reconnected;
}

char *pulse_default_device(const pa_server_info *info, bool input)
{
	const char *name = input ? info->default_source_name : info->default_sink_name;
	char *device;

	if (!name)
		return NULL;
	if (input)
		return bstrdup(name);

	device = bzalloc(strlen(name) + 9);
	strcat(device, name);
	strcat(device, ".monitor");
	return device;
}

bool pulse_restart_should_retry(uint32_t *attempts)
{
	if (++*attempts < PULSE_RESTART_MAX_ATTEMPTS)
		return true;

	*attempts = 0;
	return false;
}

bool pulse_start_should_retry(enum pulse_start_result result, bool after_reconnect, uint32_t *attempts)
{
	switch (result) {
	case PULSE_START_OK:
		return false;
	case PULSE_START_NO_SERVER:
	case PULSE_START_NO_DEVICE:
		if (!after_reconnect)
			return false;
		break;
	case PULSE_START_FAILED:
		break;
	}

	return pulse_restart_should_retry(attempts);
}

bool pulse_stream_failed_should_restart(uint32_t *attempts, uint64_t *delay_usec)
{
	*delay_usec = *attempts > 0 ? PULSE_RESTART_RETRY_USEC : 0;
	return pulse_restart_should_retry(attempts);
}

enum pulse_stream_action pulse_stream_on_event(const struct pulse_stream_status *status, enum pulse_stream_event event,
					       uint32_t index)
{
	bool running = status->has_stream && !status->failed;

	switch (event) {
	case PULSE_STREAM_FAILED:
		/* If the device is gone too, the start fails and the source
		 * waits for PULSE_SOURCE_NEW. */
		return status->has_stream && status->failed ? PULSE_STREAM_RESTART : PULSE_STREAM_KEEP;

	case PULSE_SOURCE_REMOVED:
		/* A stream on the default device is moved by the server. */
		if (running && !status->follows_default && status->device_index != PA_INVALID_INDEX &&
		    index == status->device_index)
			return PULSE_STREAM_STOP;
		return PULSE_STREAM_KEEP;

	case PULSE_SOURCE_NEW:
		/* Device names are unique, so a running stream cannot be
		 * waiting for this one. */
		return running ? PULSE_STREAM_KEEP : PULSE_STREAM_RESTART;
	}

	return PULSE_STREAM_KEEP;
}
