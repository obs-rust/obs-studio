/* Original getter and automatically discovered adapter over a private fixture. */
#include <stddef.h>
#include <stdint.h>
#define obs_get_cmdline_args oracle_obs_get_cmdline_args
#include "obs-core-cmdline-getter.c.h"
#undef obs_get_cmdline_args
#include "core_cmdline_fields.inc"

void oracle_core_cmdline_set(int argc, char **argv);
void oracle_core_cmdline_set(int argc, char **argv)
{
	cmdline_args.argc = argc;
	cmdline_args.argv = argv;
}

void oracle_core_cmdline_layout(size_t *layout);
void oracle_core_cmdline_layout(size_t *layout)
{
	layout[0] = sizeof(struct obs_cmdline_args);
	layout[1] = _Alignof(struct obs_cmdline_args);
	layout[2] = offsetof(struct obs_cmdline_args, argc);
	layout[3] = offsetof(struct obs_cmdline_args, argv);
	layout[4] = sizeof(int);
	layout[5] = _Alignof(int);
	layout[6] = sizeof(char **);
	layout[7] = _Alignof(char **);
}
