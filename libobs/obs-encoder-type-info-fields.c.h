#include "libobs/build/libobs.pch.h"

struct obs_encoder_type_info_fields obs_encoder_type_info_fields(const struct obs_encoder_info *info)
{
	return (struct obs_encoder_type_info_fields){info->get_name, info->type_data, info->codec};
}
