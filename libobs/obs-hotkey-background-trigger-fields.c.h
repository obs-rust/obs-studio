#include "libobs/build/libobs.pch.h"

extern bool obs_hotkey_seam_lock(void);
extern void obs_hotkey_seam_unlock(void);

void *obs_hotkey_background_trigger_lock(void);
bool obs_hotkey_background_trigger_dispatch(void *core, obs_key_combination_t combo, struct dstr *trigger);
void obs_hotkey_background_trigger_unlock(void);
void obs_hotkey_background_trigger_release(struct dstr *trigger);

void *obs_hotkey_background_trigger_lock(void)
{
	return obs_hotkey_seam_lock() ? obs : NULL;
}

bool obs_hotkey_background_trigger_dispatch(void *core, obs_key_combination_t combo, struct dstr *trigger)
{
	struct obs_core *context = core;
	struct obs_hotkeys_background *background = &context->hotkeys.background;
	return background->shortcut_trigger && background->shortcut_trigger(background->wake_data, combo, trigger);
}

void obs_hotkey_background_trigger_unlock(void)
{
	obs_hotkey_seam_unlock();
}

void obs_hotkey_background_trigger_release(struct dstr *trigger)
{
	dstr_free(trigger);
}
