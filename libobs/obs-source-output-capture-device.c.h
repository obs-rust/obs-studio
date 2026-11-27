#include "libobs/build/libobs.pch.h"

void obs_source_audio_output_capture_device_activated(void *vptr, calldata_t *cd);
void obs_source_audio_output_capture_device_activated(void *vptr, calldata_t *cd)
{
	UNUSED_PARAMETER(vptr);
	obs_source_t *src = calldata_ptr(cd, "source");
	if (!src)
		return;

	obs_data_t *settings = obs_source_get_settings(src);
	const char *device_id = obs_data_get_string(settings, "device_id");
	obs_source_audio_output_capture_device_changed(src, device_id);
	obs_data_release(settings);
}

extern bool devices_match(const char *id1, const char *id2);
void obs_source_audio_output_capture_device_changed(obs_source_t *src, const char *device_id)
{
	struct obs_core_audio *audio = &obs->audio;

	if (!audio->monitoring_device_name)
		return;

	if (!(src->info.output_flags & OBS_SOURCE_DO_NOT_SELF_MONITOR))
		return;

	const char *mon_id = audio->monitoring_device_id;
	bool id_match = false;

	char *def_id = NULL;
	if (obs_host_default_monitoring_device(device_id, &def_id)) {
		id_match = devices_match(def_id, mon_id);
		if (def_id)
			bfree(def_id);
	} else {
		id_match = devices_match(device_id, mon_id);
	}
	struct calldata cd;
	uint8_t stack[128];
	calldata_init_fixed(&cd, stack, sizeof(stack));

	if (id_match) {
		calldata_set_ptr(&cd, "source", src);
		signal_handler_signal(obs->signals, "deduplication_changed", &cd);
		signal_handler_connect(src->context.signals, "activate",
				       obs_source_audio_output_capture_device_activated, NULL);
		blog(LOG_INFO,
		     "Device for 'Audio Output Capture' source %s is also used for audio monitoring."
		     "\nDeduplication logic is being applied to all monitored sources.",
		     src->context.name);
	} else {
		if (src == audio->monitoring_duplicating_source) {
			calldata_set_ptr(&cd, "source", NULL);
			signal_handler_disconnect(src->context.signals, "activate",
						  obs_source_audio_output_capture_device_activated, NULL);
			signal_handler_signal(obs->signals, "deduplication_changed", &cd);
			blog(LOG_INFO, "Deduplication logic stopped.");
		}
	}
}
