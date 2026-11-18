#include "libobs/build/libobs.pch.h"

obs_data_t *obs_encoder_defaults(const char *id)
{
	const struct obs_encoder_info *info = find_encoder(id);
	return (info) ? encoder_get_defaults(info) : NULL;
}

obs_data_t *obs_encoder_get_defaults(const obs_encoder_t *encoder)
{
	if (!obs_encoder_valid(encoder, "obs_encoder_defaults"))
		return NULL;

	return encoder_get_defaults(&encoder->info);
}
