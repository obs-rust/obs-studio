#include "libobs/build/libobs.pch.h"

void obs_source_update_properties(obs_source_t *source)
{
	if (!obs_source_valid(source, "obs_source_update_properties"))
		return;

	obs_source_dosignal(source, NULL, "update_properties");
}
