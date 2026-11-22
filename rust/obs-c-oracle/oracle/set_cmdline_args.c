/* Original obs_set_cmdline_args() on the process command line that
 * core_cmdline_getter.c's real-name obs_core_cmdline_args_field() adapter
 * owns. Under cargo test that adapter is the only definition of its name, so
 * the Rust shim stores into the same struct; the probe resets it between runs
 * (oracle_core_cmdline_set) and frees each block. The sizing rules are the
 * Rust obs_core_setup_rules_cmdline_*, as in libobs.
 *
 * Three mutants make the probe's sensitivity observable: a stored table
 * counts as set (so a zero count is not unset), the strings are copied in
 * reverse order inside a block of the same size, and the table points at the
 * caller's strings instead of copies.
 *
 * Excluded: a negative count (the original writes before its block), argv
 * holding fewer than argc strings, and concurrent calls. No intentional
 * differences for counts of 0 and above. */
/* The MSVC UCRT headers define snprintf and friends inline in every object
 * that includes them, which lets a COFF link pull this member into an
 * unrelated test; keep them declarations only. */
#define _NO_CRT_STDIO_INLINE
#include "libobs/build/libobs.pch.h"

void oracle_obs_set_cmdline_args(int argc, const char *const *argv);

#define obs_set_cmdline_args oracle_obs_set_cmdline_args
#include "libobs/obs-set-cmdline-args.c.h"
#undef obs_set_cmdline_args

static size_t scla_size(int argc, const char *const *argv)
{
	size_t len = 0;
	for (int i = 0; i < argc; i++)
		len += strlen(argv[i]) + 1;
	return ((size_t)argc + 1) * sizeof(char *) + len;
}

void oracle_mutant_set_cmdline_args_table_is_set(int argc, const char *const *argv);
void oracle_mutant_set_cmdline_args_table_is_set(int argc, const char *const *argv)
{
	struct obs_cmdline_args *args = obs_core_cmdline_args_field();
	if (args->argc || args->argv)
		return;
	oracle_obs_set_cmdline_args(argc, argv);
}

void oracle_mutant_set_cmdline_args_reversed(int argc, const char *const *argv);
void oracle_mutant_set_cmdline_args_reversed(int argc, const char *const *argv)
{
	struct obs_cmdline_args *args = obs_core_cmdline_args_field();
	if (args->argc)
		return;
	args->argc = argc;
	args->argv = bmalloc(scla_size(argc, argv));
	char *data = (char *)(args->argv + argc + 1);
	for (int i = argc - 1; i >= 0; i--) {
		size_t len = strlen(argv[i]) + 1;
		args->argv[i] = data;
		memcpy(data, argv[i], len);
		data += len;
	}
	args->argv[argc] = NULL;
}

void oracle_mutant_set_cmdline_args_no_copy(int argc, const char *const *argv);
void oracle_mutant_set_cmdline_args_no_copy(int argc, const char *const *argv)
{
	struct obs_cmdline_args *args = obs_core_cmdline_args_field();
	if (args->argc)
		return;
	args->argc = argc;
	args->argv = bmalloc(scla_size(argc, argv));
	for (int i = 0; i < argc; i++)
		args->argv[i] = (char *)(uintptr_t)argv[i];
	args->argv[argc] = NULL;
}
