/* Test-only oracle: the original libobs/obs-key-combination-host-posix.c.h,
 * unmodified, with obs_key_combination_to_str renamed to oracle_* so it can
 * link next to the Rust shim. The dstr functions it calls keep their real
 * names and come from their Rust port (obs-util) under cargo test.
 *
 * obs_key_to_str() is the X11 or Wayland platform's in libobs, which is not
 * linked here. It is defined below under its real name as a stand-in that
 * calls the test's hook, so the original and the Rust shim (which calls
 * obs_key_to_str by name too) name keys through the same function. */
#include <stddef.h>

#define obs_key_combination_to_str oracle_obs_key_combination_to_str
#include "libobs/obs-key-combination-host-posix.c.h"
#undef obs_key_combination_to_str

typedef void (*oracle_key_to_str_fn)(obs_key_t key, struct dstr *str);

static _Thread_local oracle_key_to_str_fn key_to_str_hook;

void oracle_key_combination_set_key_to_str(oracle_key_to_str_fn hook);

void oracle_key_combination_set_key_to_str(oracle_key_to_str_fn hook)
{
	key_to_str_hook = hook;
}

/* Stand-in for the platform's obs_key_to_str(): the hook writes the name; with
 * no hook set the name stays empty. */
void obs_key_to_str(obs_key_t key, struct dstr *str)
{
	if (key_to_str_hook)
		key_to_str_hook(key, str);
}
