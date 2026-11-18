#include "libobs/build/libobs.pch.h"

struct obs_encoder_defaults_fields obs_encoder_defaults_type_fields(const struct obs_encoder_info *info)
{
	return (struct obs_encoder_defaults_fields){info->get_defaults, info->get_defaults2, info->type_data};
}

struct obs_encoder_defaults_fields obs_encoder_defaults_instance_fields(const obs_encoder_t *encoder)
{
	return obs_encoder_defaults_type_fields(&encoder->info);
}
