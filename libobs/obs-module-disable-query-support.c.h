#include "libobs/build/libobs.pch.h"

/* Original helper retained only for the isolated C reference. */
static bool is_core_module(const char *name)
{
	return obs_in_core_module_list(name);
}
