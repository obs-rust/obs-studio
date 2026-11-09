#include "libobs/build/libobs.pch.h"

static inline bool data_valid(const struct obs_source *source, const char *f)
{
	return obs_source_valid(source, f) && source->context.data;
}

static inline obs_data_t *source_get_defaults(const struct obs_source_info *info)
{
	obs_data_t *settings = obs_data_create();
	if (info->get_defaults2)
		info->get_defaults2(info->type_data, settings);
	else if (info->get_defaults)
		info->get_defaults(settings);
	return settings;
}

obs_properties_t *obs_get_source_properties(const char *id)
{
	const struct obs_source_info *info = get_source_info(id);
	if (info && (info->get_properties || info->get_properties2)) {
		obs_data_t *defaults = source_get_defaults(info);
		obs_properties_t *props;

		if (info->get_properties2)
			props = info->get_properties2(NULL, info->type_data);
		else
			props = info->get_properties(NULL);

		obs_properties_apply_settings(props, defaults);
		obs_data_release(defaults);
		return props;
	}
	return NULL;
}

obs_properties_t *obs_source_properties(const obs_source_t *source)
{
	if (!data_valid(source, "obs_source_properties"))
		return NULL;

	if (source->info.get_properties2) {
		obs_properties_t *props;
		props = source->info.get_properties2(source->context.data, source->info.type_data);
		obs_properties_apply_settings(props, source->context.settings);
		return props;

	} else if (source->info.get_properties) {
		obs_properties_t *props;
		props = source->info.get_properties(source->context.data);
		obs_properties_apply_settings(props, source->context.settings);
		return props;
	}

	return NULL;
}
