#include "build/libobs-pulse.pch.h"

#define PULSE_DATA(voidptr) struct audio_monitor *data = voidptr;

_Static_assert(PULSEAUDIO_MONITOR_DO_NOT_SELF_MONITOR == OBS_SOURCE_DO_NOT_SELF_MONITOR,
	       "pulseaudio-monitor-steps.h copies OBS_SOURCE_DO_NOT_SELF_MONITOR");
#define blog(level, msg, ...) blog(level, "pulse-am: " msg, ##__VA_ARGS__)

struct audio_monitor {
	obs_source_t *source;
	pa_stream *stream;
	char *device;
	pa_buffer_attr attr;
	enum speaker_layout speakers;
	pa_sample_format_t format;
	uint_fast32_t samples_per_sec;
	uint_fast32_t bytes_per_frame;
	uint_fast8_t channels;

	uint_fast32_t packets;
	uint_fast64_t frames;

	struct deque new_data;
	audio_resampler_t *resampler;

	/* What a stall left queued is dropped: from new_data, and the rest
	 * by writing the next data that much earlier in the server's buffer. */
	struct pulseaudio_monitor_backlog backlog;
	size_t rewind;

	bool ignore;
	pthread_mutex_t playback_mutex;

	/* Index of the sink the stream plays on, and whether it plays on the
	 * default sink only because the chosen one is missing. */
	uint32_t sink_idx;
	bool fallback;
};

/* Sink events arrive on the pulse mainloop thread, which must not block;
 * monitors are reset from this queue instead. */
static pthread_mutex_t sink_event_mutex = PTHREAD_MUTEX_INITIALIZER;
static os_task_queue_t *sink_event_queue = NULL;

struct sink_event {
	pa_subscription_event_type_t event;
	uint32_t idx;
};

static void sink_events_start(void);

static uint64_t bytes_per_sec(const struct audio_monitor *data)
{
	return (uint64_t)data->bytes_per_frame * data->samples_per_sec;
}

/* Drops what a stall left queued (see pulseaudio_monitor_backlog_excess()),
 * so monitoring catches up instead of staying behind for good. */
static void drop_backlog(struct audio_monitor *data)
{
	const pa_timing_info *timing = pa_stream_get_timing_info(data->stream);
	if (!timing || timing->write_index_corrupt || timing->read_index_corrupt)
		return;

	/* What the server holds: its read index is from the last timing
	 * update, so take off what it has played since. (The interpolated
	 * pa_stream_get_latency() runs ahead between updates with
	 * pipewire-pulse at large quanta.) */
	uint64_t queued = pulseaudio_monitor_queued_bytes(timing->write_index, timing->read_index,
							  timing->playing ? pa_timeval_age(&timing->timestamp) : 0,
							  bytes_per_sec(data), data->new_data.size);

	/* The server may have raised the buffer we asked for (a large
	 * quantum). */
	const pa_buffer_attr *attr = pa_stream_get_buffer_attr(data->stream);
	uint32_t tlength = attr && attr->tlength > data->attr.tlength ? attr->tlength : data->attr.tlength;

	uint64_t excess = pulseaudio_monitor_backlog_excess(&data->backlog, os_gettime_ns(),
							    util_mul_div64(queued, 1000000ULL, bytes_per_sec(data)),
							    util_mul_div64(tlength, 1000000ULL, bytes_per_sec(data)));
	if (!excess)
		return;

	struct pulseaudio_monitor_drop drop =
		pulseaudio_monitor_drop_split(excess, bytes_per_sec(data), data->bytes_per_frame, data->new_data.size);
	deque_pop_front(&data->new_data, NULL, drop.unwritten);
	data->rewind += drop.rewind;

	blog(LOG_INFO, "Monitoring in '%s' was %" PRIu64 " ms behind; dropped %" PRIu64 " ms to catch up", data->device,
	     util_mul_div64(queued, 1000ULL, bytes_per_sec(data)), excess / 1000);
}

static void do_stream_write(void *param)
{
	PULSE_DATA(param);
	uint8_t *buffer = NULL;

	pulseaudio_lock();
	pthread_mutex_lock(&data->playback_mutex);

	if (pulseaudio_monitor_grow(data->new_data.size, data->attr.tlength)) {
		data->attr.fragsize = (uint32_t)-1;
		data->attr.maxlength = (uint32_t)-1;
		data->attr.prebuf = (uint32_t)-1;
		data->attr.minreq = (uint32_t)-1;
		data->attr.tlength = pulseaudio_monitor_grown_tlength(data->new_data.size);
		pa_stream_set_buffer_attr(data->stream, &data->attr, NULL, NULL);
	}

	if (pa_stream_is_corked(data->stream)) {
		if (pulseaudio_monitor_uncork(data->new_data.size, data->attr.tlength)) {
			pa_stream_cork(data->stream, 0, NULL, NULL);
		} else {
			goto finish;
		}
	}

	drop_backlog(data);

	while (pulseaudio_monitor_write_more(data->new_data.size)) {
		size_t bytesToFill = data->new_data.size;
		if (pa_stream_begin_write(data->stream, (void **)&buffer, &bytesToFill))
			goto finish;

		if (pulseaudio_monitor_write_short(bytesToFill, data->new_data.size)) {
			pa_stream_cancel_write(data->stream);
			goto finish;
		}

		deque_pop_front(&data->new_data, buffer, bytesToFill);

		pa_stream_write(data->stream, buffer, bytesToFill, NULL, -(int64_t)data->rewind, PA_SEEK_RELATIVE);
		data->rewind = 0;
	}

finish:
	pthread_mutex_unlock(&data->playback_mutex);
	pulseaudio_unlock();
}

static void on_audio_playback(void *param, obs_source_t *source, const struct audio_data *audio_data, bool muted)
{
	UNUSED_PARAMETER(muted);

	struct audio_monitor *monitor = param;
	float vol = source->user_volume;
	size_t bytes;

	uint8_t *resample_data[MAX_AV_PLANES];
	uint32_t resample_frames;
	uint64_t ts_offset;
	bool success;

	if (pthread_mutex_trylock(&monitor->playback_mutex) != 0)
		return;

	if (pulseaudio_monitor_playback_idle(os_atomic_load_long(&source->activate_refs)))
		goto unlock;

	success = audio_resampler_resample(monitor->resampler, resample_data, &resample_frames, &ts_offset,
					   (const uint8_t *const *)audio_data->data, (uint32_t)audio_data->frames);

	if (!success)
		goto unlock;

	bytes = pulseaudio_monitor_playback_bytes(monitor->bytes_per_frame, resample_frames);

	if (pulseaudio_monitor_needs_volume(vol)) {
		pulseaudio_monitor_process_volume(monitor->format, monitor->channels, vol, resample_data,
						  resample_frames);
	}

	deque_push_back(&monitor->new_data, resample_data[0], bytes);
	monitor->packets++;
	monitor->frames += resample_frames;

unlock:
	pthread_mutex_unlock(&monitor->playback_mutex);
	do_stream_write(param);
}

static void pulseaudio_server_info(pa_context *c, const pa_server_info *i, void *userdata)
{
	UNUSED_PARAMETER(c);
	UNUSED_PARAMETER(userdata);

	blog(LOG_INFO, "Server name: '%s %s'", i->server_name, i->server_version);

	pulseaudio_signal(0);
}

static void pulseaudio_sink_info(pa_context *c, const pa_sink_info *i, int eol, void *userdata)
{
	UNUSED_PARAMETER(c);
	PULSE_DATA(userdata);
	switch (pulseaudio_monitor_sink_result(eol)) {
	case PULSEAUDIO_MONITOR_SINK_ERROR:
		data->format = PA_SAMPLE_INVALID;
		goto skip;
	case PULSEAUDIO_MONITOR_SINK_END:
		goto skip;
	case PULSEAUDIO_MONITOR_SINK_INFO:
		break;
	}

	blog(LOG_INFO, "Audio format: %s, %" PRIu32 " Hz, %" PRIu8 " channels",
	     pa_sample_format_to_string(i->sample_spec.format), i->sample_spec.rate, i->sample_spec.channels);

	pa_sample_format_t format = pulseaudio_monitor_sink_format(i->sample_spec.format);
	if (format != i->sample_spec.format) {
		blog(LOG_INFO,
		     "Sample format %s not supported by OBS,"
		     "using %s instead for recording",
		     pa_sample_format_to_string(i->sample_spec.format), pa_sample_format_to_string(format));
	}

	uint8_t channels = pulseaudio_monitor_sink_channels(i->sample_spec.channels);
	if (channels != i->sample_spec.channels) {
		blog(LOG_INFO,
		     "%" PRIu8 " channels not supported by OBS,"
		     "using %" PRIu8 " instead for recording",
		     i->sample_spec.channels, channels);
	}

	data->format = format;
	data->sink_idx = i->index;
	data->samples_per_sec = i->sample_spec.rate;
	data->channels = channels;
skip:
	pulseaudio_signal(0);
}

static void pulseaudio_stop_playback(struct audio_monitor *monitor)
{
	if (monitor->stream) {
		/* Stop the stream */
		pulseaudio_lock();
		pa_stream_disconnect(monitor->stream);
		pulseaudio_unlock();

		/* Remove the callbacks, to ensure we no longer try to do anything
		 * with this stream object */
		pulseaudio_write_callback(monitor->stream, NULL, NULL);

		/* Unreference the stream and drop it. PA will free it when it can. */
		pulseaudio_lock();
		pa_stream_unref(monitor->stream);
		pulseaudio_unlock();
		monitor->stream = NULL;
	}

	blog(LOG_INFO, "Stopped Monitoring in '%s'", monitor->device);
	blog(LOG_INFO, "Got %" PRIuFAST32 " packets with %" PRIuFAST64 " frames", monitor->packets, monitor->frames);

	monitor->packets = 0;
	monitor->frames = 0;
}

static bool audio_monitor_init(struct audio_monitor *monitor, obs_source_t *source)
{
	pthread_mutex_init_value(&monitor->playback_mutex);

	monitor->source = source;

	const char *id = obs->audio.monitoring_device_id;
	if (!pulseaudio_monitor_has_device(id))
		return false;

	if (pulseaudio_monitor_self_monitor_check(source->info.output_flags)) {
		obs_data_t *s = obs_source_get_settings(source);
		const char *s_dev_id = obs_data_get_string(s, "device_id");
		bool match = devices_match(s_dev_id, id);
		obs_data_release(s);

		if (match) {
			monitor->ignore = true;
			blog(LOG_INFO, "Prevented feedback-loop in '%s'", s_dev_id);
			return true;
		}
	}

	pulseaudio_init();

	if (pulseaudio_monitor_is_default(id))
		get_default_id(&monitor->device);
	else
		monitor->device = bstrdup(id);

	if (!monitor->device)
		return false;

	if (pulseaudio_get_server_info(pulseaudio_server_info, (void *)monitor) < 0) {
		blog(LOG_ERROR, "Unable to get server info !");
		return false;
	}

	if (pulseaudio_get_sink_info(pulseaudio_sink_info, monitor->device, (void *)monitor) < 0) {
		blog(LOG_ERROR, "Unable to get sink info !");
		return false;
	}
	if (pulseaudio_monitor_fallback(monitor->format, id)) {
		blog(LOG_WARNING, "'%s' is not available, monitoring on the default device until it returns",
		     monitor->device);
		bfree(monitor->device);
		get_default_id(&monitor->device);
		monitor->fallback = true;
		if (!pulseaudio_monitor_sink_named(monitor->device) ||
		    pulseaudio_get_sink_info(pulseaudio_sink_info, monitor->device, (void *)monitor) < 0) {
			blog(LOG_ERROR, "Unable to get sink info !");
			return false;
		}
	}
	if (pulseaudio_monitor_format_invalid(monitor->format)) {
		blog(LOG_ERROR, "An error occurred while getting the source info!");
		return false;
	}

	pa_sample_spec spec;
	spec.format = monitor->format;
	spec.rate = (uint32_t)monitor->samples_per_sec;
	spec.channels = monitor->channels;

	if (!pa_sample_spec_valid(&spec)) {
		blog(LOG_ERROR, "Sample spec is not valid");
		return false;
	}

	const struct audio_output_info *info = audio_output_get_info(obs->audio.audio);

	struct resample_info from = {.samples_per_sec = info->samples_per_sec,
				     .speakers = info->speakers,
				     .format = AUDIO_FORMAT_FLOAT_PLANAR};
	struct resample_info to = {.samples_per_sec = (uint32_t)monitor->samples_per_sec,
				   .speakers = pulseaudio_channels_to_obs_speakers(monitor->channels),
				   .format = pulseaudio_to_obs_audio_format(monitor->format)};

	monitor->resampler = audio_resampler_create(&to, &from);
	if (!monitor->resampler) {
		blog(LOG_WARNING, "%s: %s", __func__, "Failed to create resampler");
		return false;
	}

	monitor->speakers = pulseaudio_channels_to_obs_speakers(spec.channels);
	monitor->bytes_per_frame = pa_frame_size(&spec);

	pa_channel_map channel_map = pulseaudio_channel_map(monitor->speakers);

	monitor->stream = pulseaudio_stream_new(obs_source_get_name(monitor->source), &spec, &channel_map);
	if (!monitor->stream) {
		blog(LOG_ERROR, "Unable to create stream");
		return false;
	}

	monitor->attr.fragsize = (uint32_t)-1;
	monitor->attr.maxlength = (uint32_t)-1;
	monitor->attr.minreq = (uint32_t)-1;
	monitor->attr.prebuf = (uint32_t)-1;
	monitor->attr.tlength = (uint32_t)pa_usec_to_bytes(pulseaudio_monitor_tlength_usec(), &spec);

	pa_stream_flags_t flags = pulseaudio_monitor_open_flags(monitor->fallback, id);

	int_fast32_t ret = pulseaudio_connect_playback(monitor->stream, monitor->device, &monitor->attr, flags);
	if (ret < 0) {
		pulseaudio_stop_playback(monitor);
		blog(LOG_ERROR, "Unable to connect to stream");
		return false;
	}

	sink_events_start();
	blog(LOG_INFO, "Started Monitoring in '%s'%s", monitor->device, monitor->fallback ? " (fallback)" : "");
	return true;
}

static void handle_sink_event(void *param)
{
	struct sink_event *ev = param;

	pthread_mutex_lock(&obs->audio.monitoring_mutex);
	for (size_t i = 0; i < obs->audio.monitors.num; i++) {
		struct audio_monitor *monitor = obs->audio.monitors.array[i];
		if (monitor->ignore)
			continue;

		/* The chosen sink went away: the stream cannot move, so reopen
		 * it on the default sink. A sink appeared while on the fallback:
		 * it may be the chosen one, so try it again. */
		uint32_t sink_idx = monitor->stream ? monitor->sink_idx : PA_INVALID_INDEX;
		if (pulseaudio_monitor_reset_on_sink_event(ev->event, ev->idx, sink_idx, monitor->fallback))
			audio_monitor_reset(monitor);
	}
	pthread_mutex_unlock(&obs->audio.monitoring_mutex);

	bfree(ev);
}

static void on_sink_event(pa_context *c, pa_subscription_event_type_t t, uint32_t idx, void *userdata)
{
	UNUSED_PARAMETER(c);
	UNUSED_PARAMETER(userdata);

	if (!pulseaudio_monitor_event_queued(t))
		return;

	pthread_mutex_lock(&sink_event_mutex);
	if (sink_event_queue) {
		struct sink_event *ev = bmalloc(sizeof(*ev));
		ev->event = t;
		ev->idx = idx;
		os_task_queue_queue_task(sink_event_queue, handle_sink_event, ev);
	}
	pthread_mutex_unlock(&sink_event_mutex);
}

static void sink_events_start(void)
{
	pthread_mutex_lock(&sink_event_mutex);
	if (!sink_event_queue)
		sink_event_queue = os_task_queue_create();
	pthread_mutex_unlock(&sink_event_mutex);

	pulseaudio_subscribe_sinks(on_sink_event, NULL);
}

static void sink_events_stop(void)
{
	pthread_mutex_lock(&sink_event_mutex);
	os_task_queue_t *queue = sink_event_queue;
	sink_event_queue = NULL;
	pthread_mutex_unlock(&sink_event_mutex);

	os_task_queue_destroy(queue);
}

static void audio_monitor_init_final(struct audio_monitor *monitor)
{
	if (monitor->ignore)
		return;

	obs_source_add_audio_capture_callback(monitor->source, on_audio_playback, monitor);
}

static inline void audio_monitor_free(struct audio_monitor *monitor)
{
	if (monitor->ignore)
		return;

	if (monitor->source)
		obs_source_remove_audio_capture_callback(monitor->source, on_audio_playback, monitor);

	audio_resampler_destroy(monitor->resampler);
	deque_free(&monitor->new_data);

	if (monitor->stream)
		pulseaudio_stop_playback(monitor);
	pulseaudio_unref();

	bfree(monitor->device);
}

struct audio_monitor *audio_monitor_create(obs_source_t *source)
{
	struct audio_monitor monitor = {0};
	struct audio_monitor *out;

	if (!audio_monitor_init(&monitor, source))
		goto fail;

	out = bmemdup(&monitor, sizeof(monitor));

	pthread_mutex_lock(&obs->audio.monitoring_mutex);
	da_push_back(obs->audio.monitors, &out);
	pthread_mutex_unlock(&obs->audio.monitoring_mutex);

	audio_monitor_init_final(out);
	return out;

fail:
	audio_monitor_free(&monitor);
	return NULL;
}

void audio_monitor_reset(struct audio_monitor *monitor)
{
	struct audio_monitor new_monitor = {0};

	if (audio_monitor_init(&new_monitor, monitor->source)) {
		audio_monitor_free(monitor);
		*monitor = new_monitor;
		audio_monitor_init_final(monitor);
	} else {
		audio_monitor_free(&new_monitor);
		if (monitor->stream) {
			pulseaudio_lock();
			pa_context *context = pa_stream_get_context(monitor->stream);
			bool context_good = context && pulseaudio_monitor_context_good(pa_context_get_state(context));
			pulseaudio_unlock();

			if (!context_good) {
				/* A failed replacement must not leave a terminal stream's
				 * capture callback active. The empty monitor owns no ref. */
				obs_source_t *source = monitor->source;
				audio_monitor_free(monitor);
				*monitor = (struct audio_monitor){.source = source, .ignore = true};
			}
		}
	}
}

void audio_monitor_destroy(struct audio_monitor *monitor)
{
	if (monitor) {
		audio_monitor_free(monitor);

		pthread_mutex_lock(&obs->audio.monitoring_mutex);
		da_erase_item(obs->audio.monitors, &monitor);
		bool last = pulseaudio_monitor_last(obs->audio.monitors.num);
		pthread_mutex_unlock(&obs->audio.monitoring_mutex);

		if (last)
			sink_events_stop();

		bfree(monitor);
	}
}

/* File-local macros: keep them out of the next source of a unity build. */
#undef blog

/* File-local macros (BUILD.md): out of the next file in the unity group. */
#undef PULSE_DATA
