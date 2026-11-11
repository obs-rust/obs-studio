#include "libobs/build/libobs.pch.h"

const char *obs_source_get_display_name(const char *id)
{
	const struct obs_source_info *info = get_source_info(id);
	return (info != NULL) ? info->get_name(info->type_data) : NULL;
}

bool obs_is_source_configurable(const char *id)
{
	const struct obs_source_info *info = get_source_info(id);
	return info && (info->get_properties || info->get_properties2);
}

uint32_t obs_get_source_output_flags(const char *id)
{
	const struct obs_source_info *info = get_source_info(id);
	return info ? info->output_flags : 0;
}
