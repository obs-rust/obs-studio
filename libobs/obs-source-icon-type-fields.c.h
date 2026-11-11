#include "libobs/build/libobs.pch.h"

/* Private view of registry metadata, not a public ABI layout. */
const enum obs_icon_type *obs_source_icon_type_fields(const struct obs_source_info *info);
const enum obs_icon_type *obs_source_icon_type_fields(const struct obs_source_info *info)
{
	return &info->icon_type;
}

/* Registry type metadata, distinct from an instance's context data. */
void *obs_source_info_icon_data(const struct obs_source_info *info);
void *obs_source_info_icon_data(const struct obs_source_info *info)
{
	return info->type_data;
}

typedef const char *(*obs_source_info_icon_callback)(void *);
obs_source_info_icon_callback obs_source_info_dark_icon(const struct obs_source_info *info);
obs_source_info_icon_callback obs_source_info_dark_icon(const struct obs_source_info *info)
{
	return info->get_dark_icon;
}

obs_source_info_icon_callback obs_source_info_light_icon(const struct obs_source_info *info);
obs_source_info_icon_callback obs_source_info_light_icon(const struct obs_source_info *info)
{
	return info->get_light_icon;
}
