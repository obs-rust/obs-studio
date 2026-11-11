#include "libobs/build/libobs.pch.h"

struct obs_output_type_info_fields obs_output_type_info_fields(const struct obs_output_info *info)
{
	return (struct obs_output_type_info_fields){info->get_name, info->type_data, info->flags,
						    info->encoded_video_codecs, info->encoded_audio_codecs};
}
