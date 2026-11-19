#include "libobs/build/libobs.pch.h"

extern void obs_hotkey_unregister_one(obs_hotkey_id id);
void obs_hotkey_unregister_entry_single_call(obs_hotkey_id id);

/* Keep the caller's single-remover namespace in isolated registry fixtures. */
void obs_hotkey_unregister_entry_single_call(obs_hotkey_id id)
{
	obs_hotkey_unregister_one(id);
}
