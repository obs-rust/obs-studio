#include "libobs/build/libobs.pch.h"

const char *obs_get_latest_input_type_id(const char *unversioned_id)
{
	struct obs_source_info *latest = NULL;
	int version = OBS_CORE_SETUP_RULES_LATEST_NONE;

	if (!unversioned_id)
		return NULL;

	for (size_t i = 0; i < obs->source_types.num; i++) {
		struct obs_source_info *info = &obs->source_types.array[i];
		if (strcmp(info->unversioned_id, unversioned_id) == 0 &&
		    obs_core_setup_rules_latest_newer(info->version, version)) {
			latest = info;
			version = (int)info->version;
		}
	}

	assert(!!latest);
	if (!latest)
		return NULL;

	return latest->id;
}
