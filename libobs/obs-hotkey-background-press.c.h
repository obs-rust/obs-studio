#include "libobs/build/libobs.pch.h"

#define lock obs_hotkey_seam_lock
#define unlock obs_hotkey_seam_unlock

extern bool obs_hotkey_seam_lock(void);
extern void obs_hotkey_seam_unlock(void);

void obs_hotkey_enable_background_press(bool enable)
{
	if (!lock())
		return;

	bool changed = obs->hotkeys.thread_disable_press == enable;
	obs->hotkeys.thread_disable_press = !enable;
	unlock();

	if (!changed)
		return;

	/* Platforms that have the desktop hold the keys for OBS (the
	 * GlobalShortcuts portal on Wayland) give them back while presses
	 * are disabled. */
	calldata_t data;
	uint8_t stack[128];
	calldata_init_fixed(&data, stack, sizeof(stack));
	calldata_set_bool(&data, "enabled", enable);
	signal_handler_signal(obs->hotkeys.signals, "hotkey_background_press_changed", &data);
}

#undef lock
#undef unlock
