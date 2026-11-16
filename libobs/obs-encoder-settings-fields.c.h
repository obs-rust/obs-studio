#include "libobs/build/libobs.pch.h"

obs_data_t *obs_encoder_settings_fields(const obs_encoder_t *encoder);
obs_data_t *obs_encoder_settings_fields(const obs_encoder_t *encoder)
{
	return encoder->context.settings;
}
