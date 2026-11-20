/* A fixture-owned backend name takes precedence over the real runtime. */
#include "libobs/build/libobs-graphics.pch.h"

extern const char *obs_link_support_preprocessor_name(void);

const char *gs_preprocessor_name(void)
{
	return obs_link_support_preprocessor_name();
}
