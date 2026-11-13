#include "libobs/build/libobs-nix.pch.h"

static inline void add_combo_key(obs_key_t key, struct dstr *str)
{
	struct dstr key_str = {0};

	obs_key_to_str(key, &key_str);

	if (!dstr_is_empty(&key_str)) {
		if (!dstr_is_empty(str)) {
			dstr_cat(str, " + ");
		}
		dstr_cat_dstr(str, &key_str);
	}

	dstr_free(&key_str);
}

void obs_key_combination_to_str(obs_key_combination_t combination, struct dstr *str)
{
	if ((combination.modifiers & INTERACT_CONTROL_KEY) != 0) {
		add_combo_key(OBS_KEY_CONTROL, str);
	}
	if ((combination.modifiers & INTERACT_COMMAND_KEY) != 0) {
		add_combo_key(OBS_KEY_META, str);
	}
	if ((combination.modifiers & INTERACT_ALT_KEY) != 0) {
		add_combo_key(OBS_KEY_ALT, str);
	}
	if ((combination.modifiers & INTERACT_SHIFT_KEY) != 0) {
		add_combo_key(OBS_KEY_SHIFT, str);
	}
	if (combination.key != OBS_KEY_NONE) {
		add_combo_key(combination.key, str);
	}
}
