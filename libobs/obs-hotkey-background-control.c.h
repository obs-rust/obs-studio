#include "libobs/build/libobs.pch.h"

/* Reuse the registry mutex without exposing its file-local helpers. */
extern bool obs_hotkey_seam_lock(void);
extern void obs_hotkey_seam_unlock(void);

#define lock obs_hotkey_seam_lock
#define unlock obs_hotkey_seam_unlock

/* Lets the platform client act on a retry or probe change, then unlocks.
 * Called under the mutex, so the client cannot be stopped meanwhile. */
static void wake_background_client(void)
{
	if (obs->hotkeys.background.wake)
		obs->hotkeys.background.wake(obs->hotkeys.background.wake_data);
	unlock();
}

void obs_hotkeys_retry_background(void)
{
	if (!lock())
		return;

	obs->hotkeys.background.retry = true;
	wake_background_client();
}

void obs_hotkeys_configure_background(void)
{
	if (!lock())
		return;

	obs->hotkeys.background.configure = true;
	obs->hotkeys.background.flags &= ~(uint32_t)OBS_HOTKEYS_BACKGROUND_CONFIGURE_FAILED;
	wake_background_client();
}

void obs_hotkeys_set_background_probe(bool enable)
{
	if (!lock())
		return;

	if (obs->hotkeys.background.probe == enable) {
		unlock();
		return;
	}
	obs->hotkeys.background.probe = enable;
	/* A request to open the desktop's editor belongs to this probe. */
	if (!enable)
		obs->hotkeys.background.configure = false;
	wake_background_client();
}

#undef lock
#undef unlock
