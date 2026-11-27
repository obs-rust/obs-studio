#include "libobs/build/libobs.pch.h"

/* What obs_source_audio_output_capture_device_changed() and its activate
 * handler read and call (the obs-source shim), each one step of the
 * original: the core's monitoring fields, the source's flags and name, the
 * host default device, the platform's devices_match(), the
 * deduplication_changed signal, the activate connection and the settings
 * read. Rust decides which run, and in what order. */

extern bool devices_match(const char *id1, const char *id2);

bool obs_source_output_capture_device_monitoring_set(void);
bool obs_source_output_capture_device_monitoring_set(void)
{
	return obs->audio.monitoring_device_name != NULL;
}

uint32_t obs_source_output_capture_device_flags(const obs_source_t *source);
uint32_t obs_source_output_capture_device_flags(const obs_source_t *source)
{
	return source->info.output_flags;
}

const char *obs_source_output_capture_device_monitoring_id(void);
const char *obs_source_output_capture_device_monitoring_id(void)
{
	return obs->audio.monitoring_device_id;
}

bool obs_source_output_capture_device_host_default(const char *device_id, char **def_id);
bool obs_source_output_capture_device_host_default(const char *device_id, char **def_id)
{
	return obs_host_default_monitoring_device(device_id, def_id);
}

bool obs_source_output_capture_device_match(const char *id1, const char *id2);
bool obs_source_output_capture_device_match(const char *id1, const char *id2)
{
	return devices_match(id1, id2);
}

void obs_source_output_capture_device_free(char *id);
void obs_source_output_capture_device_free(char *id)
{
	bfree(id);
}

void obs_source_output_capture_device_signal(obs_source_t *source);
void obs_source_output_capture_device_signal(obs_source_t *source)
{
	struct calldata cd;
	uint8_t stack[128];
	calldata_init_fixed(&cd, stack, sizeof(stack));
	calldata_set_ptr(&cd, "source", source);
	signal_handler_signal(obs->signals, "deduplication_changed", &cd);
}

void obs_source_output_capture_device_connect(obs_source_t *source, signal_callback_t handler);
void obs_source_output_capture_device_connect(obs_source_t *source, signal_callback_t handler)
{
	signal_handler_connect(source->context.signals, "activate", handler, NULL);
}

void obs_source_output_capture_device_disconnect(obs_source_t *source, signal_callback_t handler);
void obs_source_output_capture_device_disconnect(obs_source_t *source, signal_callback_t handler)
{
	signal_handler_disconnect(source->context.signals, "activate", handler, NULL);
}

const char *obs_source_output_capture_device_name(const obs_source_t *source);
const char *obs_source_output_capture_device_name(const obs_source_t *source)
{
	return source->context.name;
}

obs_source_t *obs_source_output_capture_device_duplicating(void);
obs_source_t *obs_source_output_capture_device_duplicating(void)
{
	return obs->audio.monitoring_duplicating_source;
}

obs_source_t *obs_source_output_capture_device_calldata_source(calldata_t *cd);
obs_source_t *obs_source_output_capture_device_calldata_source(calldata_t *cd)
{
	return calldata_ptr(cd, "source");
}

obs_data_t *obs_source_output_capture_device_settings(obs_source_t *source);
obs_data_t *obs_source_output_capture_device_settings(obs_source_t *source)
{
	return obs_source_get_settings(source);
}

const char *obs_source_output_capture_device_settings_id(obs_data_t *settings);
const char *obs_source_output_capture_device_settings_id(obs_data_t *settings)
{
	return obs_data_get_string(settings, "device_id");
}

void obs_source_output_capture_device_release_settings(obs_data_t *settings);
void obs_source_output_capture_device_release_settings(obs_data_t *settings)
{
	obs_data_release(settings);
}
