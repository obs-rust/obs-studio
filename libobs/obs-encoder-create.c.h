#include "libobs/build/libobs.pch.h"

/* Private constructor engine; allocation and context ownership remain in C. */
extern struct obs_encoder *create_encoder(const char *id, enum obs_encoder_type type, const char *name,
					  obs_data_t *settings, size_t mixer_idx, obs_data_t *hotkey_data);

obs_encoder_t *obs_video_encoder_create(const char *id, const char *name, obs_data_t *settings, obs_data_t *hotkey_data)
{
	if (!name || !id)
		return NULL;
	return create_encoder(id, OBS_ENCODER_VIDEO, name, settings, 0, hotkey_data);
}

obs_encoder_t *obs_audio_encoder_create(const char *id, const char *name, obs_data_t *settings, size_t mixer_idx,
					obs_data_t *hotkey_data)
{
	if (!name || !id)
		return NULL;
	return create_encoder(id, OBS_ENCODER_AUDIO, name, settings, mixer_idx, hotkey_data);
}
