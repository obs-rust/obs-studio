#include "libobs/build/libobs.pch.h"

struct obs_core *const *obs_core_initialized_field(void)
{
	return &obs;
}
