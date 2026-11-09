#include "libobs/build/libobs.pch.h"

/* Defined in obs-encoder.c.h in this unity group. */
static inline obs_data_t *encoder_get_defaults(const struct obs_encoder_info *info);

obs_properties_t *obs_get_encoder_properties(const char *id)
{
	const struct obs_encoder_info *ei = find_encoder(id);
	if (ei && (ei->get_properties || ei->get_properties2)) {
		obs_data_t *defaults = encoder_get_defaults(ei);
		obs_properties_t *properties = NULL;

		if (ei->get_properties2) {
			properties = ei->get_properties2(NULL, ei->type_data);
		} else if (ei->get_properties) {
			properties = ei->get_properties(NULL);
		}

		obs_properties_apply_settings(properties, defaults);
		obs_data_release(defaults);
		return properties;
	}
	return NULL;
}

obs_properties_t *obs_encoder_properties(const obs_encoder_t *encoder)
{
	if (!obs_encoder_valid(encoder, "obs_encoder_properties"))
		return NULL;

	if (encoder->orig_info.get_properties2) {
		obs_properties_t *props;
		props = encoder->orig_info.get_properties2(encoder->context.data, encoder->orig_info.type_data);
		obs_properties_apply_settings(props, encoder->context.settings);
		return props;

	} else if (encoder->orig_info.get_properties) {
		obs_properties_t *props;
		props = encoder->orig_info.get_properties(encoder->context.data);
		obs_properties_apply_settings(props, encoder->context.settings);
		return props;
	}

	return NULL;
}
