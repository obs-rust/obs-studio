#include "libobs/build/libobs.pch.h"

static struct obs_cmdline_args cmdline_args = {0, NULL};

struct obs_cmdline_args *obs_core_cmdline_args_field(void)
{
	return &cmdline_args;
}
