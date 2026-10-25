/*
Copyright (C) 2014 by Leonhard Oelke <leonhard@in-verted.de>

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

#include "plugins/linux-pulseaudio/build/linux-pulseaudio-nix.pch.h"

#define NSEC_PER_MSEC 1000000L

#define PULSE_DATA(voidptr) struct pulse_data *data = voidptr;
#define blog(level, msg, ...) blog(level, "pulse-input: " msg, ##__VA_ARGS__)

struct pulse_data {
	obs_source_t *source;
	obs_weak_source_t *weak_source;
	pa_stream *stream;

	/* user settings */
	char *device;
	bool is_default;
	bool input;

	/* server info */
	enum speaker_layout speakers;
	pa_sample_format_t format;
	uint_fast32_t samples_per_sec;
	uint_fast32_t bytes_per_frame;
	uint_fast8_t channels;
	uint64_t first_ts;
	struct pulse_timing timing;
	bool reconnected;
	/* Failed restarts in a row; reset by the read callback, so access it
	 * with the mainloop locked. */
	uint32_t restart_attempts;

	/* statistics */
	uint_fast32_t packets;
	uint_fast64_t frames;
};

static void pulse_stop_recording(struct pulse_data *data);
static void pulse_stream_state_changed(pa_stream *s, void *userdata);

/**
 * get obs from pulse audio format
 */
static enum audio_format pulse_to_obs_audio_format(pa_sample_format_t format)
{
	switch (format) {
	case PA_SAMPLE_U8:
		return AUDIO_FORMAT_U8BIT;
	case PA_SAMPLE_S16LE:
		return AUDIO_FORMAT_16BIT;
	case PA_SAMPLE_S32LE:
		return AUDIO_FORMAT_32BIT;
	case PA_SAMPLE_FLOAT32LE:
		return AUDIO_FORMAT_FLOAT;
	default:
		return AUDIO_FORMAT_UNKNOWN;
	}

	return AUDIO_FORMAT_UNKNOWN;
}

/**
 * Get obs speaker layout from number of channels
 *
 * @param channels number of channels reported by pulseaudio
 *
 * @return obs speaker_layout id
 *
 * @note This *might* not work for some rather unusual setups, but should work
 *       fine for the majority of cases.
 */
static enum speaker_layout pulse_channels_to_obs_speakers(uint_fast32_t channels)
{
	switch (channels) {
	case 1:
		return SPEAKERS_MONO;
	case 2:
		return SPEAKERS_STEREO;
	case 3:
		return SPEAKERS_2POINT1;
	case 4:
		return SPEAKERS_4POINT0;
	case 5:
		return SPEAKERS_4POINT1;
	case 6:
		return SPEAKERS_5POINT1;
	case 8:
		return SPEAKERS_7POINT1;
	}

	return SPEAKERS_UNKNOWN;
}

static pa_channel_map pulse_channel_map(enum speaker_layout layout)
{
	pa_channel_map ret;

	ret.map[0] = PA_CHANNEL_POSITION_FRONT_LEFT;
	ret.map[1] = PA_CHANNEL_POSITION_FRONT_RIGHT;
	ret.map[2] = PA_CHANNEL_POSITION_FRONT_CENTER;
	ret.map[3] = PA_CHANNEL_POSITION_LFE;
	ret.map[4] = PA_CHANNEL_POSITION_REAR_LEFT;
	ret.map[5] = PA_CHANNEL_POSITION_REAR_RIGHT;
	ret.map[6] = PA_CHANNEL_POSITION_SIDE_LEFT;
	ret.map[7] = PA_CHANNEL_POSITION_SIDE_RIGHT;

	switch (layout) {
	case SPEAKERS_MONO:
		ret.channels = 1;
		ret.map[0] = PA_CHANNEL_POSITION_MONO;
		break;

	case SPEAKERS_STEREO:
		ret.channels = 2;
		break;

	case SPEAKERS_2POINT1:
		ret.channels = 3;
		ret.map[2] = PA_CHANNEL_POSITION_LFE;
		break;

	case SPEAKERS_4POINT0:
		ret.channels = 4;
		ret.map[3] = PA_CHANNEL_POSITION_REAR_CENTER;
		break;

	case SPEAKERS_4POINT1:
		ret.channels = 5;
		ret.map[4] = PA_CHANNEL_POSITION_REAR_CENTER;
		break;

	case SPEAKERS_5POINT1:
		ret.channels = 6;
		break;

	case SPEAKERS_7POINT1:
		ret.channels = 8;
		break;

	case SPEAKERS_UNKNOWN:
	default:
		ret.channels = 0;
		break;
	}

	return ret;
}

#define STARTUP_TIMEOUT_NS (500 * NSEC_PER_MSEC)

/**
 * Callback for pulse which gets executed when new audio data is available
 *
 * @warning The function may be called even after disconnecting the stream
 */
static void pulse_stream_read(pa_stream *p, size_t nbytes, void *userdata)
{
	UNUSED_PARAMETER(p);
	UNUSED_PARAMETER(nbytes);
	PULSE_DATA(userdata);

	const void *frames;
	size_t bytes;

	if (!data->stream)
		goto exit;

	pa_stream_peek(data->stream, &frames, &bytes);

	// check if we got data
	if (!bytes)
		goto exit;

	if (!frames) {
		blog(LOG_ERROR, "Got audio hole of %u bytes", (unsigned int)bytes);
		pa_stream_drop(data->stream);
		goto exit;
	}

	/* The stream works: later failures start counting again. */
	data->restart_attempts = 0;

	struct obs_source_audio out;
	out.speakers = data->speakers;
	out.samples_per_sec = data->samples_per_sec;
	out.format = pulse_to_obs_audio_format(data->format);
	out.data[0] = (uint8_t *)(uintptr_t)frames;
	out.frames = bytes / data->bytes_per_frame;
	out.timestamp = pulse_timing_packet_ts(&data->timing, os_gettime_ns(), out.frames, out.samples_per_sec);

	/* Skip the startup period after a reconnect: the audio was already
	 * running and every dropped packet lengthens the gap. */
	if (!data->first_ts)
		data->first_ts = data->reconnected ? out.timestamp - 1 : out.timestamp + STARTUP_TIMEOUT_NS;

	if (out.timestamp > data->first_ts)
		obs_source_output_audio(data->source, &out);

	data->packets++;
	data->frames += out.frames;

	pa_stream_drop(data->stream);
exit:
	pulse_signal(0);
}

/**
 * Server info callback
 */
static void pulse_server_info(pa_context *c, const pa_server_info *i, void *userdata)
{
	UNUSED_PARAMETER(c);
	PULSE_DATA(userdata);

	blog(LOG_INFO, "Server name: '%s %s'", i->server_name, i->server_version);

	if (data->is_default) {
		char *device = pulse_default_device(i, data->input);

		/* The server may have no default device, e.g. right after it
		 * restarted. Keep the previous one then. */
		if (device) {
			bfree(data->device);
			data->device = device;
			blog(LOG_DEBUG, "Default %s device: '%s'", data->input ? "input" : "output", data->device);
		} else {
			blog(LOG_DEBUG, "No default device yet");
		}
	}

	pulse_signal(0);
}

/**
 * Source info callback
 *
 * We use the default stream settings for recording here unless pulse is
 * configured to something obs can't deal with.
 */
static void pulse_source_info(pa_context *c, const pa_source_info *i, int eol, void *userdata)
{
	UNUSED_PARAMETER(c);
	PULSE_DATA(userdata);
	// An error occurred
	if (eol < 0) {
		data->format = PA_SAMPLE_INVALID;
		goto skip;
	}
	// Terminating call for multi instance callbacks
	if (eol > 0)
		goto skip;

	blog(LOG_INFO,
	     "Audio format: %s, %" PRIu32 " Hz"
	     ", %" PRIu8 " channels",
	     pa_sample_format_to_string(i->sample_spec.format), i->sample_spec.rate, i->sample_spec.channels);

	pa_sample_format_t format = i->sample_spec.format;
	if (pulse_to_obs_audio_format(format) == AUDIO_FORMAT_UNKNOWN) {
		format = PA_SAMPLE_FLOAT32LE;

		blog(LOG_INFO,
		     "Sample format %s not supported by OBS,"
		     "using %s instead for recording",
		     pa_sample_format_to_string(i->sample_spec.format), pa_sample_format_to_string(format));
	}

	uint8_t channels = i->sample_spec.channels;
	if (pulse_channels_to_obs_speakers(channels) == SPEAKERS_UNKNOWN) {
		channels = 2;

		blog(LOG_INFO, "%" PRIu8 " channels not supported by OBS, using %" PRIu8 " instead for recording",
		     i->sample_spec.channels, channels);
	}

	data->format = format;
	data->samples_per_sec = i->sample_spec.rate;
	data->channels = channels;

skip:
	pulse_signal(0);
}

/**
 * Start recording
 *
 * We request the default format used by pulse here because the data will be
 * converted and possibly re-sampled by obs anyway.
 *
 * For now we request a buffer length of 25ms although pulse seems to ignore
 * this setting for monitor streams. For "real" input streams this should work
 * fine though.
 */
static enum pulse_start_result pulse_start_recording(struct pulse_data *data)
{
	if (pulse_get_server_info(pulse_server_info, (void *)data) < 0) {
		blog(LOG_ERROR, "Unable to get server info !");
		return PULSE_START_NO_SERVER;
	}

	if (pulse_get_source_info(pulse_source_info, data->device, (void *)data) < 0) {
		blog(LOG_ERROR, "Unable to get source info !");
		return PULSE_START_NO_SERVER;
	}
	if (data->format == PA_SAMPLE_INVALID) {
		blog(LOG_ERROR, "An error occurred while getting the source info!");
		return PULSE_START_NO_DEVICE;
	}

	pa_sample_spec spec;
	spec.format = data->format;
	spec.rate = data->samples_per_sec;
	spec.channels = data->channels;

	if (!pa_sample_spec_valid(&spec)) {
		blog(LOG_ERROR, "Sample spec is not valid");
		return PULSE_START_FAILED;
	}

	data->speakers = pulse_channels_to_obs_speakers(spec.channels);
	data->bytes_per_frame = pa_frame_size(&spec);

	pa_channel_map channel_map = pulse_channel_map(data->speakers);

	data->stream = pulse_stream_new(obs_source_get_name(data->source), &spec, &channel_map);
	if (!data->stream) {
		blog(LOG_ERROR, "Unable to create stream");
		return PULSE_START_FAILED;
	}

	pulse_lock();
	pa_stream_set_read_callback(data->stream, pulse_stream_read, (void *)data);
	pa_stream_set_state_callback(data->stream, pulse_stream_state_changed, (void *)data);
	pulse_unlock();

	pa_buffer_attr attr;
	attr.fragsize = pa_usec_to_bytes(25000, &spec);
	attr.maxlength = (uint32_t)-1;
	attr.minreq = (uint32_t)-1;
	attr.prebuf = (uint32_t)-1;
	attr.tlength = (uint32_t)-1;

	/* Never let the server move the stream, "default" included: PulseAudio
	 * moves record streams to the default source, so a "default" output
	 * capture whose sink went away would record the microphone. The source
	 * follows the default itself (PULSE_SERVER_CHANGED). */
	pa_stream_flags_t flags = PA_STREAM_ADJUST_LATENCY | PA_STREAM_DONT_MOVE;

	pulse_lock();
	int_fast32_t ret = pa_stream_connect_record(data->stream, data->device, &attr, flags);
	pulse_unlock();
	if (ret < 0) {
		pulse_stop_recording(data);
		blog(LOG_ERROR, "Unable to connect to stream");
		return PULSE_START_FAILED;
	}

	if (data->is_default)
		blog(LOG_INFO, "Started recording from '%s' (default)", data->device);
	else
		blog(LOG_INFO, "Started recording from '%s'", data->device);

	return PULSE_START_OK;
}

/**
 * stop recording
 */
static void pulse_stop_recording(struct pulse_data *data)
{
	if (data->stream) {
		pulse_lock();
		pa_stream_set_state_callback(data->stream, NULL, NULL);
		pulse_stream_free(data->stream);
		data->stream = NULL;
		pulse_unlock();
	}

	blog(LOG_INFO, "Stopped recording from '%s'", data->device);
	blog(LOG_INFO, "Got %" PRIuFAST32 " packets with %" PRIuFAST64 " frames", data->packets, data->frames);

	data->first_ts = 0;
	pulse_timing_reset(&data->timing);
	data->packets = 0;
	data->frames = 0;
}

static void pulse_release_weak_source(void *userdata)
{
	obs_weak_source_release(userdata);
}

/* Runs cb on the pulse mainloop thread after delay_usec, with a weak ref. */
static void pulse_call_restart_later(struct pulse_data *data, pulse_reconnected_cb cb, uint64_t delay_usec)
{
	obs_weak_source_addref(data->weak_source);
	pulse_lock();
	pulse_call_later(cb, data->weak_source, pulse_release_weak_source, delay_usec);
	pulse_unlock();
}

static void pulse_retry_restart(void *userdata);
static void pulse_retry_stream(void *userdata);

static void pulse_default_device_info(pa_context *c, const pa_server_info *i, void *userdata)
{
	UNUSED_PARAMETER(c);
	struct pulse_data *data = ((void **)userdata)[0];
	char **device = ((void **)userdata)[1];

	*device = pulse_default_device(i, data->input);
	pulse_signal(0);
}

/**
 * The server's current default device for this source, allocated with
 * bmalloc, or NULL if it has none or cannot be asked. Runs on the UI thread.
 */
static char *pulse_current_default_device(struct pulse_data *data)
{
	char *device = NULL;
	void *userdata[2] = {data, &device};

	if (pulse_get_server_info(pulse_default_device_info, userdata) < 0)
		return NULL;
	return device;
}

/**
 * Start the stream again; if that fails, retry after PULSE_RESTART_RETRY_USEC
 * as pulse_start_should_retry() decides. Runs on the UI thread: starting a
 * stream blocks on the pulse mainloop.
 */
static void pulse_restart_recording(struct pulse_data *data, bool after_reconnect)
{
	if (data->stream)
		pulse_stop_recording(data);

	enum pulse_start_result result = pulse_start_recording(data);

	pulse_lock();
	bool retry = pulse_start_should_retry(result, after_reconnect, &data->restart_attempts);
	pulse_unlock();

	if (retry)
		pulse_call_restart_later(data, after_reconnect ? pulse_retry_restart : pulse_retry_stream,
					 PULSE_RESTART_RETRY_USEC);
	else if (result == PULSE_START_FAILED || (after_reconnect && result != PULSE_START_OK))
		blog(LOG_WARNING, "Giving up restarting '%s', waiting for a device to appear", data->device);
}

/**
 * Restart recording on the new connection after the server came back.
 * Runs on the UI thread.
 */
static void pulse_restart_recording_task(void *param)
{
	obs_weak_source_t *weak_source = param;
	obs_source_t *source = obs_weak_source_get_source(weak_source);

	obs_weak_source_release(weak_source);
	if (!source)
		return;

	struct pulse_data *data = obs_obj_get_data(source);
	if (data) {
		data->reconnected = true;
		pulse_restart_recording(data, true);
	}

	obs_source_release(source);
}

/**
 * Retry timer fired; called on the pulse mainloop thread, owns a weak ref
 */
static void pulse_retry_restart(void *userdata)
{
	obs_queue_task(OBS_TASK_UI, pulse_restart_recording_task, userdata, false);
}

/**
 * Retry a stream that failed or could not be started while the server
 * stayed up. Runs on the UI thread.
 */
static void pulse_retry_stream_task(void *param)
{
	obs_weak_source_t *weak_source = param;
	obs_source_t *source = obs_weak_source_get_source(weak_source);

	obs_weak_source_release(weak_source);
	if (!source)
		return;

	struct pulse_data *data = obs_obj_get_data(source);
	if (data) {
		/* A device that appeared meanwhile may have started it. */
		bool running = false;
		if (data->stream) {
			pulse_lock();
			running = PA_STREAM_IS_GOOD(pa_stream_get_state(data->stream));
			pulse_unlock();
		}
		if (!running)
			pulse_restart_recording(data, false);
	}

	obs_source_release(source);
}

/**
 * Retry timer fired; called on the pulse mainloop thread, owns a weak ref
 */
static void pulse_retry_stream(void *userdata)
{
	obs_queue_task(OBS_TASK_UI, pulse_retry_stream_task, userdata, false);
}

/**
 * Server reconnected; called on the pulse mainloop thread
 */
static void pulse_reconnected(void *userdata)
{
	obs_weak_source_t *weak_source = userdata;

	obs_weak_source_addref(weak_source);
	obs_queue_task(OBS_TASK_UI, pulse_restart_recording_task, weak_source, false);
}

struct pulse_stream_task {
	obs_weak_source_t *weak_source;
	enum pulse_stream_event event;
	uint32_t index;
	/* PULSE_STREAM_FAILED: the stream that failed */
	pa_stream *stream;
};

/**
 * The source's own stream died or a device came or went while the server
 * connection stayed up. Runs on the UI thread: starting a stream blocks on
 * the pulse mainloop.
 */
static void pulse_stream_event_task(void *param)
{
	struct pulse_stream_task *task = param;
	obs_source_t *source = obs_weak_source_get_source(task->weak_source);

	obs_weak_source_release(task->weak_source);
	if (!source)
		goto out;

	struct pulse_data *data = obs_obj_get_data(source);
	if (!data)
		goto release;

	/* A failure of a stream that was already replaced is old news. */
	if (task->event == PULSE_STREAM_FAILED && task->stream != data->stream)
		goto release;

	struct pulse_stream_status status = {
		.has_stream = data->stream != NULL,
		.follows_default = data->is_default,
		.device_index = PA_INVALID_INDEX,
	};
	char *new_default = NULL;
	if (task->event == PULSE_SERVER_CHANGED && data->is_default) {
		new_default = pulse_current_default_device(data);
		status.default_changed = new_default && (!data->device || strcmp(new_default, data->device) != 0);
	}
	if (data->stream) {
		pulse_lock();
		pa_stream_state_t state = pa_stream_get_state(data->stream);
		status.failed = !PA_STREAM_IS_GOOD(state);
		if (state == PA_STREAM_READY)
			status.device_index = pa_stream_get_device_index(data->stream);
		pulse_unlock();
	}

	switch (pulse_stream_on_event(&status, task->event, task->index)) {
	case PULSE_STREAM_STOP:
		blog(LOG_WARNING, "Device '%s' went away, waiting for it to return", data->device);
		pulse_stop_recording(data);
		break;
	case PULSE_STREAM_RESTART:
		/* After a failure or a change of the default the audio was
		 * running: skip the startup period as after a reconnect. */
		data->reconnected = task->event == PULSE_STREAM_FAILED || task->event == PULSE_SERVER_CHANGED;
		if (task->event == PULSE_SERVER_CHANGED)
			blog(LOG_INFO, "Default %s device changed from '%s' to '%s'", data->input ? "input" : "output",
			     data->device, new_default);
		if (task->event == PULSE_STREAM_FAILED) {
			uint64_t delay_usec;
			pulse_lock();
			bool restart = pulse_stream_failed_should_restart(&data->restart_attempts, &delay_usec);
			pulse_unlock();

			pulse_stop_recording(data);
			if (!restart) {
				blog(LOG_WARNING, "Giving up restarting '%s', waiting for a device to appear",
				     data->device);
				break;
			}
			blog(LOG_WARNING, "Stream from '%s' failed, restarting it", data->device);
			if (delay_usec) {
				pulse_call_restart_later(data, pulse_retry_stream, delay_usec);
				break;
			}
		}
		pulse_restart_recording(data, false);
		break;
	case PULSE_STREAM_KEEP:
		break;
	}

	bfree(new_default);
release:
	obs_source_release(source);
out:
	bfree(task);
}

static void pulse_queue_stream_task(obs_weak_source_t *weak_source, enum pulse_stream_event event, uint32_t index,
				    pa_stream *stream)
{
	struct pulse_stream_task *task = bzalloc(sizeof(*task));

	obs_weak_source_addref(weak_source);
	task->weak_source = weak_source;
	task->event = event;
	task->index = index;
	task->stream = stream;
	obs_queue_task(OBS_TASK_UI, pulse_stream_event_task, task, false);
}

/**
 * Stream state callback; called on the pulse mainloop thread
 */
static void pulse_stream_state_changed(pa_stream *s, void *userdata)
{
	PULSE_DATA(userdata);

	if (PA_STREAM_IS_GOOD(pa_stream_get_state(s)))
		return;
	/* With the shared connection lost, pulse_reconnected restarts it. The
	 * stream's own connection may have been dropped alone (#532): restart
	 * it then like a stream that failed. */
	if (!pulse_connected())
		return;

	pulse_queue_stream_task(data->weak_source, PULSE_STREAM_FAILED, PA_INVALID_INDEX, s);
}

/**
 * A device appeared or went away, or the server's defaults may have changed;
 * called on the pulse mainloop thread
 */
static void pulse_source_event(void *userdata, pa_subscription_event_type_t type, uint32_t index)
{
	enum pulse_stream_event event;

	if (type & PA_SUBSCRIPTION_EVENT_FACILITY_MASK)
		event = PULSE_SERVER_CHANGED;
	else
		event = type == PA_SUBSCRIPTION_EVENT_NEW ? PULSE_SOURCE_NEW : PULSE_SOURCE_REMOVED;
	pulse_queue_stream_task(userdata, event, index, NULL);
}

/**
 * input info callback
 */
static void pulse_input_info(pa_context *c, const pa_source_info *i, int eol, void *userdata)
{
	UNUSED_PARAMETER(c);
	if (eol != 0 || i->monitor_of_sink != PA_INVALID_INDEX)
		goto skip;

	obs_property_list_add_string((obs_property_t *)userdata, i->description, i->name);

skip:
	pulse_signal(0);
}

/**
 * output info callback
 */
static void pulse_output_info(pa_context *c, const pa_sink_info *i, int eol, void *userdata)
{
	UNUSED_PARAMETER(c);
	if (eol != 0 || i->monitor_source == PA_INVALID_INDEX)
		goto skip;

	obs_property_list_add_string((obs_property_t *)userdata, i->description, i->monitor_source_name);

skip:
	pulse_signal(0);
}

/**
 * Get plugin properties
 */
static obs_properties_t *pulse_get_obs_properties(bool input)
{
	obs_properties_t *props = obs_properties_create();
	obs_property_t *devices = obs_properties_add_list(props, "device_id", obs_module_text("Device"),
							  OBS_COMBO_TYPE_LIST, OBS_COMBO_FORMAT_STRING);

	pulse_init();
	if (input)
		pulse_get_source_info_list(pulse_input_info, (void *)devices);
	else
		pulse_get_sink_info_list(pulse_output_info, (void *)devices);
	pulse_unref();

	size_t count = obs_property_list_item_count(devices);

	if (count > 0)
		obs_property_list_insert_string(devices, 0, obs_module_text("Default"), "default");

	return props;
}

static obs_properties_t *pulse_input_properties(void *unused)
{
	UNUSED_PARAMETER(unused);

	return pulse_get_obs_properties(true);
}

static obs_properties_t *pulse_output_properties(void *unused)
{
	UNUSED_PARAMETER(unused);

	return pulse_get_obs_properties(false);
}

/**
 * Get plugin defaults
 */
static void pulse_defaults(obs_data_t *settings)
{
	obs_data_set_default_string(settings, "device_id", "default");
}

/**
 * Returns the name of the plugin
 */
static const char *pulse_input_getname(void *unused)
{
	UNUSED_PARAMETER(unused);
	return obs_module_text("PulseInput");
}

static const char *pulse_output_getname(void *unused)
{
	UNUSED_PARAMETER(unused);
	return obs_module_text("PulseOutput");
}

/**
 * Destroy the plugin object and free all memory
 */
static void pulse_destroy(void *vptr)
{
	PULSE_DATA(vptr);

	if (!data)
		return;

	pulse_lock();
	pulse_remove_reconnected_callback(pulse_reconnected, data->weak_source);
	pulse_remove_source_event_callback(pulse_source_event, data->weak_source);
	pulse_unlock();

	if (data->stream)
		pulse_stop_recording(data);

	/* If the device is also used for monitoring, a cleanup is needed. */
	if (!data->input)
		obs_source_audio_output_capture_device_changed(data->source, NULL);

	pulse_unref();

	if (data->device)
		bfree(data->device);
	obs_weak_source_release(data->weak_source);
	bfree(data);
}

/**
 * Update the input settings
 */
static void pulse_update(void *vptr, obs_data_t *settings)
{
	PULSE_DATA(vptr);
	bool restart = false;
	const char *new_device = obs_data_get_string(settings, "device_id");
	/* data->device holds what "default" resolved to. */
	const char *old_device = data->is_default ? "default" : data->device;
	if (!old_device || strcmp(old_device, new_device) != 0) {
		/* Signal to deduplication logic in case the device is also used for monitoring. */
		if (!data->input)
			obs_source_audio_output_capture_device_changed(data->source, new_device);

		if (data->device)
			bfree(data->device);

		data->device = bstrdup(new_device);
		data->is_default = strcmp("default", data->device) == 0;
		restart = true;
	}

	if (!restart)
		return;

	if (data->stream)
		pulse_stop_recording(data);
	data->reconnected = false;
	pulse_start_recording(data);
}

/**
 * Create the plugin object
 */
static void *pulse_create(obs_data_t *settings, obs_source_t *source, bool input)
{
	struct pulse_data *data = bzalloc(sizeof(struct pulse_data));

	data->input = input;
	data->source = source;
	/* The device runs on its own clock: keep its audio in pace with OBS. */
	obs_source_set_async_compensation(source, true);
	data->weak_source = obs_source_get_weak_source(source);

	pulse_init();

	pulse_lock();
	pulse_add_reconnected_callback(pulse_reconnected, data->weak_source);
	pulse_add_source_event_callback(pulse_source_event, data->weak_source);
	pulse_unlock();
	pulse_update(data, settings);

	return data;
}

static void *pulse_input_create(obs_data_t *settings, obs_source_t *source)
{
	return pulse_create(settings, source, true);
}

static void *pulse_output_create(obs_data_t *settings, obs_source_t *source)
{
	return pulse_create(settings, source, false);
}

struct obs_source_info pulse_input_capture = {
	.id = "pulse_input_capture",
	.type = OBS_SOURCE_TYPE_INPUT,
	.output_flags = OBS_SOURCE_AUDIO | OBS_SOURCE_DO_NOT_DUPLICATE,
	.get_name = pulse_input_getname,
	.create = pulse_input_create,
	.destroy = pulse_destroy,
	.update = pulse_update,
	.get_defaults = pulse_defaults,
	.get_properties = pulse_input_properties,
	.icon_type = OBS_ICON_TYPE_AUDIO_INPUT,
};

struct obs_source_info pulse_output_capture = {
	.id = "pulse_output_capture",
	.type = OBS_SOURCE_TYPE_INPUT,
	.output_flags = OBS_SOURCE_AUDIO | OBS_SOURCE_DO_NOT_DUPLICATE | OBS_SOURCE_DO_NOT_SELF_MONITOR,
	.get_name = pulse_output_getname,
	.create = pulse_output_create,
	.destroy = pulse_destroy,
	.update = pulse_update,
	.get_defaults = pulse_defaults,
	.get_properties = pulse_output_properties,
	.icon_type = OBS_ICON_TYPE_AUDIO_OUTPUT,
};

/* File-local macros (BUILD.md): out of the next file in the unity group. */
#undef NSEC_PER_MSEC
#undef PULSE_DATA
#undef blog
#undef STARTUP_TIMEOUT_NS
