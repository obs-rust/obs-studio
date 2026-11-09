#include "libobs/build/libobs.pch.h"

void obs_source_add_audio_pause_callback(obs_source_t *source, signal_callback_t callback, void *param)
{
	if (!obs_source_valid(source, "obs_source_add_audio_pause_callback"))
		return;

	signal_handler_t *handler = obs_source_get_signal_handler(source);

	signal_handler_connect(handler, "media_pause", callback, param);
	signal_handler_connect(handler, "media_stopped", callback, param);
}

void obs_source_remove_audio_pause_callback(obs_source_t *source, signal_callback_t callback, void *param)
{
	if (!obs_source_valid(source, "obs_source_remove_audio_pause_callback"))
		return;

	signal_handler_t *handler = obs_source_get_signal_handler(source);

	signal_handler_disconnect(handler, "media_pause", callback, param);
	signal_handler_disconnect(handler, "media_stopped", callback, param);
}
