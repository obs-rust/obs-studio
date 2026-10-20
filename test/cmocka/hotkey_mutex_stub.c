/* obs_hotkeys_mutex_lock/unlock for tests that compile obs-nix-hotkey-state.c
 * without obs-hotkey.c. Built with the test's obs=<test core> definition, it
 * locks that core's hotkey mutex, as libobs does for the real one. */

#include <obs-internal.h>

void obs_hotkeys_mutex_lock(void)
{
	pthread_mutex_lock(&obs->hotkeys.mutex);
}

void obs_hotkeys_mutex_unlock(void)
{
	pthread_mutex_unlock(&obs->hotkeys.mutex);
}
