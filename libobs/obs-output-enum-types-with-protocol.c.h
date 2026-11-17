#include "libobs/build/libobs.pch.h"

void obs_enum_output_types_with_protocol(const char *protocol, void *data, bool (*enum_cb)(void *data, const char *id))
{
	if (!obs_is_output_protocol_registered(protocol))
		return;

	for (size_t i = 0; i < obs->output_types.num; i++) {
		if (!(obs->output_types.array[i].flags & OBS_OUTPUT_SERVICE))
			continue;

		size_t matches = obs_output_protocol_list_count(obs->output_types.array[i].protocols, protocol);
		for (size_t match = 0; match < matches; match++) {
			if (!enum_cb(data, obs->output_types.array[i].id))
				return;
		}
	}
}
