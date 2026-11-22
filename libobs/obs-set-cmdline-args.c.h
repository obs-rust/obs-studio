#include "libobs/build/libobs.pch.h"

void obs_set_cmdline_args(int argc, const char *const *argv)
{
	struct obs_cmdline_args *args = obs_core_cmdline_args_field();
	char *data;
	size_t len;
	int i;

	/* Once argc is set (non-zero) we shouldn't call again */
	if (obs_core_setup_rules_cmdline_set(args->argc))
		return;

	args->argc = argc;

	/* Safely copy over argv */
	len = 0;
	for (i = 0; i < argc; i++)
		len = obs_core_setup_rules_cmdline_len(len, strlen(argv[i]));

	args->argv = bmalloc(obs_core_setup_rules_cmdline_table(argc, sizeof(char *)) + len);
	data = (char *)args->argv + obs_core_setup_rules_cmdline_table(argc, sizeof(char *));

	for (i = 0; i < argc; i++) {
		args->argv[i] = data;
		len = strlen(argv[i]) + 1;
		memcpy(data, argv[i], len);
		data += len;
	}

	args->argv[argc] = NULL;
}
