#include "libobs/build/libobs.pch.h"

static void source_signal_push_to_delay(obs_source_t *source, const char *signal, uint64_t delay)
{
	struct calldata data;
	uint8_t stack[128];

	calldata_init_fixed(&data, stack, sizeof(stack));
	calldata_set_ptr(&data, "source", source);
	calldata_set_int(&data, "delay", (long long)delay);

	signal_handler_signal(source->context.signals, signal, &data);
}

void obs_source_set_push_to_mute_delay(obs_source_t *source, uint64_t delay)
{
	if (!obs_source_valid(source, "obs_source_set_push_to_mute_delay"))
		return;

	pthread_mutex_lock(&source->audio_mutex);
	source->push_to_mute_delay = delay;

	source_signal_push_to_delay(source, "push_to_mute_delay", delay);
	pthread_mutex_unlock(&source->audio_mutex);
}

void obs_source_set_push_to_talk_delay(obs_source_t *source, uint64_t delay)
{
	if (!obs_source_valid(source, "obs_source_set_push_to_talk_delay"))
		return;

	pthread_mutex_lock(&source->audio_mutex);
	source->push_to_talk_delay = delay;

	source_signal_push_to_delay(source, "push_to_talk_delay", delay);
	pthread_mutex_unlock(&source->audio_mutex);
}
