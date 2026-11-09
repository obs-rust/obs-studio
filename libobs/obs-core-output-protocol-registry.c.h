#include "libobs/build/libobs.pch.h"

bool obs_is_output_protocol_registered(const char *protocol)
{
	if (!protocol)
		return false;

	for (size_t i = 0; i < obs->data.protocols.num; i++) {
		if (strcmp(protocol, obs->data.protocols.array[i]) == 0)
			return true;
	}

	return false;
}

bool obs_enum_output_protocols(size_t idx, char **protocol)
{
	if (!protocol || idx >= obs->data.protocols.num)
		return false;

	*protocol = obs->data.protocols.array[idx];
	return true;
}
