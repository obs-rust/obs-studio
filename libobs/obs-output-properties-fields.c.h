#include "libobs/build/libobs.pch.h"

struct obs_output_properties_fields obs_output_properties_type_fields(const struct obs_output_info *info)
{
	return (struct obs_output_properties_fields){info->get_defaults, info->get_properties, NULL, NULL};
}

struct obs_output_properties_fields obs_output_properties_instance_fields(const obs_output_t *output)
{
	return (struct obs_output_properties_fields){output->info.get_defaults, output->info.get_properties,
						     output->context.data, output->context.settings};
}
