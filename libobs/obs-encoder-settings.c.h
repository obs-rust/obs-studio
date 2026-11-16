#include "libobs/build/libobs.pch.h"

obs_data_t *obs_encoder_get_settings(const obs_encoder_t *encoder)
{
	if (!obs_encoder_valid(encoder, "obs_encoder_get_settings"))
		return NULL;

	obs_data_addref(encoder->context.settings);
	return encoder->context.settings;
}
