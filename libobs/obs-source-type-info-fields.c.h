#include "libobs/build/libobs.pch.h"

struct obs_source_type_info_fields obs_source_type_info_fields(const struct obs_source_info *info)
{
	return (struct obs_source_type_info_fields){info->get_name, info->type_data, info->output_flags,
						    info->get_properties != NULL, info->get_properties2 != NULL};
}
