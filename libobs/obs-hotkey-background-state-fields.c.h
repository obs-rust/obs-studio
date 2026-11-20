#include "libobs/build/libobs.pch.h"

/* Defined in obs-hotkey.c.h, later in the same unity group, or by the
 * recording fixture that includes this file. */
static inline bool lock(void);
static inline void unlock(void);

/* Private adapters for the Rust background-state query. The hotkey mutex and
 * the background fields stay in C; Rust reads each field once, only between a
 * successful lock and its unlock. */
bool obs_hotkeys_background_state_lock(void);
bool obs_hotkeys_background_state_lock(void)
{
	return lock();
}

/* Called only with the hotkey mutex held. */
enum obs_hotkeys_background_state obs_hotkeys_background_state_read(void);
enum obs_hotkeys_background_state obs_hotkeys_background_state_read(void)
{
	return obs->hotkeys.background.state;
}

/* Called only with the hotkey mutex held. */
uint32_t obs_hotkeys_background_state_flags(void);
uint32_t obs_hotkeys_background_state_flags(void)
{
	return obs->hotkeys.background.flags;
}

void obs_hotkeys_background_state_unlock(void);
void obs_hotkeys_background_state_unlock(void)
{
	unlock();
}
