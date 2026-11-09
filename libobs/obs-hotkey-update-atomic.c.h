#include "libobs/build/libobs.pch.h"

#define lock obs_hotkey_seam_lock
#define unlock obs_hotkey_seam_unlock

extern bool obs_hotkey_seam_lock(void);
extern void obs_hotkey_seam_unlock(void);

void obs_hotkey_update_atomic(obs_hotkey_atomic_update_func func, void *data)
{
	if (!lock())
		return;

	func(data);

	unlock();
}

#undef lock
#undef unlock
