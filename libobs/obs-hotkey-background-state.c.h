#include "libobs/build/libobs.pch.h"

/* Defined in obs-hotkey.c.h, later in the same unity group. */
static inline bool lock(void);
static inline void unlock(void);

enum obs_hotkeys_background_state obs_hotkeys_get_background_state(uint32_t *flags)
{
	if (flags)
		*flags = 0;
	if (!lock())
		return OBS_HOTKEYS_BACKGROUND_NATIVE;

	enum obs_hotkeys_background_state state = obs->hotkeys.background.state;
	if (flags)
		*flags = obs->hotkeys.background.flags;
	unlock();
	return state;
}
