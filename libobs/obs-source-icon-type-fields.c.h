#include "libobs/build/libobs.pch.h"

/* Private view of registry metadata, not a public ABI layout. */
const enum obs_icon_type *obs_source_icon_type_fields(const struct obs_source_info *info);
const enum obs_icon_type *obs_source_icon_type_fields(const struct obs_source_info *info)
{
	return &info->icon_type;
}
