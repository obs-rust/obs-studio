#include "libobs/build/libobs.pch.h"

#define cmdline_args (*obs_core_cmdline_args_field())

struct obs_cmdline_args obs_get_cmdline_args(void)
{
	return cmdline_args;
}

#undef cmdline_args
