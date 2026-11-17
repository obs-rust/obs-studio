#include "libobs/build/libobs.pch.h"

/* Private forwarding seams keep pair metadata dispatch independently testable
 * after the public single-hotkey setters become Rust exports. */
void obs_hotkey_pair_meta_set_name(obs_hotkey_id id, const char *value);
void obs_hotkey_pair_meta_set_description(obs_hotkey_id id, const char *value);

void obs_hotkey_pair_meta_set_name(obs_hotkey_id id, const char *value)
{
	obs_hotkey_set_name(id, value);
}

void obs_hotkey_pair_meta_set_description(obs_hotkey_id id, const char *value)
{
	obs_hotkey_set_description(id, value);
}
