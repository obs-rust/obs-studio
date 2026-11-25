#include "libobs/build/libobs.pch.h"

static inline void signal_flags_updated(obs_source_t *source)
{
	struct calldata data;
	uint8_t stack[128];

	calldata_init_fixed(&data, stack, sizeof(stack));
	calldata_set_ptr(&data, "source", source);
	calldata_set_int(&data, "flags", source->flags);

	signal_handler_signal(source->context.signals, "update_flags", &data);
}

void obs_source_set_flags(obs_source_t *source, uint32_t flags)
{
	if (!obs_source_valid(source, "obs_source_set_flags"))
		return;

	if (obs_source_monitoring_changes(source->flags, flags)) {
		source->flags = flags;
		signal_flags_updated(source);
	}
}

void obs_source_set_audio_mixers(obs_source_t *source, uint32_t mixers)
{
	struct calldata data;
	uint8_t stack[128];

	if (!obs_source_valid(source, "obs_source_set_audio_mixers"))
		return;
	if (!source->owns_info_id && !obs_source_monitoring_has_audio(source->info.output_flags))
		return;

	if (!obs_source_monitoring_changes(source->audio_mixers, mixers))
		return;

	calldata_init_fixed(&data, stack, sizeof(stack));
	calldata_set_ptr(&data, "source", source);
	calldata_set_int(&data, "mixers", mixers);

	signal_handler_signal(source->context.signals, "audio_mixers", &data);

	mixers = obs_source_monitoring_mixers_store(calldata_int(&data, "mixers"));

	source->audio_mixers = mixers;
}
