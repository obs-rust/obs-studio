#include "libobs/build/libobs.pch.h"

const char *obs_encoder_get_display_name(const char *id)
{
	struct obs_encoder_info *ei = find_encoder(id);
	return ei ? ei->get_name(ei->type_data) : NULL;
}

const char *obs_get_encoder_codec(const char *id)
{
	struct obs_encoder_info *info = find_encoder(id);
	return info ? info->codec : NULL;
}
