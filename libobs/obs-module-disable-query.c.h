#include "libobs/build/libobs.pch.h"

static bool is_core_module(const char *name);

bool obs_get_module_allow_disable(const char *name)
{
	return obs_module_load_rules_allow_disable(is_core_module(name));
}
