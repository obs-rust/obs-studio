#include "libobs/build/libobs.pch.h"

uint32_t obs_get_version(void)
{
	return LIBOBS_API_VER;
}

const char *obs_get_version_string(void)
{
	return OBS_VERSION;
}
