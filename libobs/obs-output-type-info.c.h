#include "libobs/build/libobs.pch.h"

const char *obs_output_get_display_name(const char *id)
{
	const struct obs_output_info *info = find_output(id);
	return (info != NULL) ? info->get_name(info->type_data) : NULL;
}

uint32_t obs_get_output_flags(const char *id)
{
	const struct obs_output_info *info = find_output(id);
	return info ? info->flags : 0;
}

const char *obs_get_output_supported_video_codecs(const char *id)
{
	const struct obs_output_info *info = find_output(id);
	return info ? info->encoded_video_codecs : NULL;
}

const char *obs_get_output_supported_audio_codecs(const char *id)
{
	const struct obs_output_info *info = find_output(id);
	return info ? info->encoded_audio_codecs : NULL;
}
