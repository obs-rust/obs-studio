#include "libobs/build/libobs-ported-service-getters.pch.h"

enum obs_icon_type obs_source_get_icon_type(const char *id)
{
	const struct obs_source_info *info = get_source_info(id);
	return (info) ? info->icon_type : OBS_ICON_TYPE_UNKNOWN;
}
