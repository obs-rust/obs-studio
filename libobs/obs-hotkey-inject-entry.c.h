#include "libobs/build/libobs.pch.h"

/* Use the existing registry seams without exposing its file-local helpers. */
#define lock obs_hotkey_seam_lock
#define unlock obs_hotkey_seam_unlock
#define enum_bindings obs_hotkey_seam_enum_bindings

struct obs_hotkey_internal_inject {
	obs_key_combination_t hotkey;
	bool pressed;
	bool strict_modifiers;
};

static inline bool inject_hotkey(void *data, size_t idx, obs_hotkey_binding_t *binding)
{
	UNUSED_PARAMETER(idx);
	struct obs_hotkey_internal_inject *event = data;
	obs_hotkey_binding_inject(binding, event->hotkey, event->pressed, event->strict_modifiers);
	return true;
}

void obs_hotkey_inject_event(obs_key_combination_t hotkey, bool pressed)
{
	if (!lock())
		return;

	struct obs_hotkey_internal_inject event = {
		{hotkey.modifiers, hotkey.key},
		pressed,
		obs->hotkeys.strict_modifiers,
	};
	enum_bindings(inject_hotkey, &event);
	unlock();
}

#undef lock
#undef unlock
#undef enum_bindings
