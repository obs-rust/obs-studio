#include "libobs/build/libobs.pch.h"

static inline obs_data_t *source_get_defaults(const struct obs_source_info *info)
{
	obs_data_t *settings = obs_data_create();
	if (info->get_defaults2)
		info->get_defaults2(info->type_data, settings);
	else if (info->get_defaults)
		info->get_defaults(settings);
	return settings;
}

obs_data_t *obs_source_settings(const char *id);
obs_data_t *obs_source_settings(const char *id)
{
	const struct obs_source_info *info = get_source_info(id);
	return (info) ? source_get_defaults(info) : NULL;
}

obs_data_t *obs_get_source_defaults(const char *id)
{
	const struct obs_source_info *info = get_source_info(id);
	return info ? source_get_defaults(info) : NULL;
}
