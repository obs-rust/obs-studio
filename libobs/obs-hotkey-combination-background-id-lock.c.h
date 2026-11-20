#include "libobs/build/libobs.pch.h"

extern bool obs_hotkey_seam_lock(void);
extern void obs_hotkey_seam_unlock(void);

/* The extracted wrapper calls the static lock pair of obs-hotkey.c.h. In the
 * C reference's own group it reaches the same mutex through the seam. */
static inline bool lock(void)
{
	return obs_hotkey_seam_lock();
}

static inline void unlock(void)
{
	obs_hotkey_seam_unlock();
}
