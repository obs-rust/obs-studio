#include "libobs/build/libobs.pch.h"

const char *obs_source_get_dark_icon(const char *id)
{
	if (obs_source_get_icon_type(id) != OBS_ICON_TYPE_CUSTOM)
		return NULL;

	const struct obs_source_info *info = get_source_info(id);
	return (info && info->get_dark_icon) ? info->get_dark_icon(info->type_data) : NULL;
}

const char *obs_source_get_light_icon(const char *id)
{
	if (obs_source_get_icon_type(id) != OBS_ICON_TYPE_CUSTOM)
		return NULL;

	const struct obs_source_info *info = get_source_info(id);
	return (info && info->get_light_icon) ? info->get_light_icon(info->type_data) : NULL;
}
