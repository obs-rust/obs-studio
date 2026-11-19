#include "libobs/build/libobs.pch.h"

bool obs_hotkey_inject_entry_strict(const void *core);

bool obs_hotkey_inject_entry_strict(const void *core)
{
	const struct obs_core *context = core;
	return context->hotkeys.strict_modifiers;
}
