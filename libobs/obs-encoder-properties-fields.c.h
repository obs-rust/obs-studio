#include "libobs/build/libobs.pch.h"

struct obs_encoder_properties_fields obs_encoder_properties_type_fields(const struct obs_encoder_info *info)
{
	return (struct obs_encoder_properties_fields){info->get_properties,
						      info->get_properties2,
						      info->get_defaults,
						      info->get_defaults2,
						      info->type_data,
						      NULL,
						      NULL};
}

struct obs_encoder_properties_fields obs_encoder_properties_instance_fields(const obs_encoder_t *encoder)
{
	return (struct obs_encoder_properties_fields){
		encoder->orig_info.get_properties, encoder->orig_info.get_properties2, encoder->orig_info.get_defaults,
		encoder->orig_info.get_defaults2,  encoder->orig_info.type_data,       encoder->context.data,
		encoder->context.settings};
}
