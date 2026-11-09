#include "libobs/build/libobs.pch.h"

enum obs_encoder_type obs_get_encoder_type(const char *id)
{
	struct obs_encoder_info *info = find_encoder(id);
	return info ? info->type : OBS_ENCODER_AUDIO;
}

uint32_t obs_get_encoder_caps(const char *encoder_id)
{
	struct obs_encoder_info *info = find_encoder(encoder_id);
	return info ? info->caps : 0;
}
