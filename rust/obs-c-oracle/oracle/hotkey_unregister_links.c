/* Test-only link stand-ins for the hotkey unregister shims
 * (rust/obs-hotkey/src/ffi/unregister.rs), under libobs's own names.
 *
 * The shims name the private field accessors of
 * libobs/obs-hotkey-unregister-fields.c.h and two weak releases that live in
 * libobs. The MSVC, MinGW and macOS linkers want every symbol an object names
 * resolved, used or not, so obs-tests-core needs a definition; libobs has the
 * real ones. obs_weak_output_release() and obs_weak_service_release() come
 * from obs-output, the seam signal and binding removal from the hotkey tests.
 *
 * The tests never reach these: they run the safe core with a Registry of
 * their own, and the parity cases go through oracle/hotkey_unregister.c. */

#include "obs-internal.h"

size_t obs_hotkey_unregister_next_id(void);
obs_hotkey_t *obs_hotkey_unregister_find(size_t id);
void obs_hotkey_unregister_remove(obs_hotkey_t *key);
int obs_hotkey_unregister_category(obs_hotkey_t *key);
void *obs_hotkey_unregister_owner(obs_hotkey_t *key);
void obs_hotkey_unregister_clear_owner(obs_hotkey_t *key);
void *obs_hotkey_unregister_name(obs_hotkey_t *key);
void *obs_hotkey_unregister_description(obs_hotkey_t *key);

size_t obs_hotkey_unregister_next_id(void)
{
	return 0;
}

obs_hotkey_t *obs_hotkey_unregister_find(size_t id)
{
	UNUSED_PARAMETER(id);
	return NULL;
}

void obs_hotkey_unregister_remove(obs_hotkey_t *key)
{
	UNUSED_PARAMETER(key);
}

int obs_hotkey_unregister_category(obs_hotkey_t *key)
{
	UNUSED_PARAMETER(key);
	return 0;
}

void *obs_hotkey_unregister_owner(obs_hotkey_t *key)
{
	UNUSED_PARAMETER(key);
	return NULL;
}

void obs_hotkey_unregister_clear_owner(obs_hotkey_t *key)
{
	UNUSED_PARAMETER(key);
}

void *obs_hotkey_unregister_name(obs_hotkey_t *key)
{
	UNUSED_PARAMETER(key);
	return NULL;
}

void *obs_hotkey_unregister_description(obs_hotkey_t *key)
{
	UNUSED_PARAMETER(key);
	return NULL;
}

void obs_weak_source_release(obs_weak_source_t *weak)
{
	UNUSED_PARAMETER(weak);
}

void obs_weak_encoder_release(obs_weak_encoder_t *weak)
{
	UNUSED_PARAMETER(weak);
}
