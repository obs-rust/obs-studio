#include "libobs/build/libobs.pch.h"

struct obs_source_properties_fields obs_source_properties_info_fields(const struct obs_source_info *info)
{
	return (struct obs_source_properties_fields){info->get_properties, info->get_properties2, info->get_defaults,
						     info->get_defaults2, info->type_data, NULL, NULL};
}

struct obs_source_properties_fields obs_source_properties_instance_fields(const obs_source_t *source)
{
	struct obs_source_properties_fields fields = obs_source_properties_info_fields(&source->info);
	fields.data = source->context.data;
	fields.settings = &source->context.settings;
	return fields;
}
