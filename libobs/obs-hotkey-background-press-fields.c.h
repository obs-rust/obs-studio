#include "libobs/build/libobs.pch.h"

extern bool obs_hotkey_seam_lock(void);
extern void obs_hotkey_seam_unlock(void);

void *obs_hotkey_background_press_lock(void);
bool *obs_hotkey_background_press_disabled(void *core);
void obs_hotkey_background_press_unlock(void);
void obs_hotkey_background_press_signal(void *core, bool enabled);

void *obs_hotkey_background_press_lock(void)
{
	return obs_hotkey_seam_lock() ? obs : NULL;
}

bool *obs_hotkey_background_press_disabled(void *core)
{
	struct obs_core *context = core;
	return &context->hotkeys.thread_disable_press;
}

void obs_hotkey_background_press_unlock(void)
{
	obs_hotkey_seam_unlock();
}

void obs_hotkey_background_press_signal(void *core, bool enabled)
{
	struct obs_core *context = core;
	/* Desktop portals give held keys back while background presses are disabled. */
	calldata_t data;
	uint8_t stack[128];
	calldata_init_fixed(&data, stack, sizeof(stack));
	calldata_set_bool(&data, "enabled", enabled);
	signal_handler_signal(context->hotkeys.signals, "hotkey_background_press_changed", &data);
}
