/*
This program is free software: you can redistribute it and/or modify
it under the terms of the GNU General Public License as published by
the Free Software Foundation, either version 2 of the License, or
(at your option) any later version.

This program is distributed in the hope that it will be useful,
but WITHOUT ANY WARRANTY; without even the implied warranty of
MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
GNU General Public License for more details.

You should have received a copy of the GNU General Public License
along with this program.  If not, see <http://www.gnu.org/licenses/>.
*/

#include "libobs/build/libobs-nix-portal-shortcut-ids.pch.h"

/* ------------------------------------------------------------------------ */
/* Shortcut ids and triggers                                                 */

static const struct {
	uint32_t modifier;
	const char *id;
	const char *trigger;
} modifier_names[] = {
	{INTERACT_CONTROL_KEY, "ctrl", "CTRL"},
	{INTERACT_ALT_KEY, "alt", "ALT"},
	{INTERACT_SHIFT_KEY, "shift", "SHIFT"},
	{INTERACT_COMMAND_KEY, "super", "LOGO"},
};

#define NUM_MODIFIERS (sizeof(modifier_names) / sizeof(modifier_names[0]))

bool obs_nix_portal_shortcut_id(obs_key_combination_t combo, struct dstr *id)
{
	if (!bindable(combo))
		return false;

	dstr_free(id);
	for (size_t i = 0; i < NUM_MODIFIERS; i++) {
		if (combo.modifiers & modifier_names[i].modifier) {
			dstr_cat(id, modifier_names[i].id);
			dstr_cat(id, "+");
		}
	}
	dstr_cat(id, obs_key_to_name(combo.key));
	return true;
}

bool obs_nix_portal_shortcut_combo(const char *id, obs_key_combination_t *combo)
{
	obs_key_combination_t result = {0, OBS_KEY_NONE};
	const char *part = id;
	const char *plus;

	while ((plus = strchr(part, '+')) != NULL) {
		size_t len = plus - part;
		bool known = false;
		for (size_t i = 0; i < NUM_MODIFIERS; i++) {
			if (strlen(modifier_names[i].id) == len && strncmp(part, modifier_names[i].id, len) == 0) {
				result.modifiers |= modifier_names[i].modifier;
				known = true;
				break;
			}
		}
		if (!known)
			return false;
		part = plus + 1;
	}

	if (strncmp(part, "OBS_KEY_", 8) != 0)
		return false;
	result.key = obs_key_from_name(part);
	if (!bindable(result))
		return false;

	*combo = result;
	return true;
}

bool obs_nix_portal_shortcut_trigger(obs_key_combination_t combo, obs_nix_portal_key_name_func key_name,
				     struct dstr *trigger)
{
	struct dstr name = {0};
	if (!bindable(combo) || !key_name || !key_name(combo.key, &name) || dstr_is_empty(&name)) {
		dstr_free(&name);
		return false;
	}

	dstr_free(trigger);
	for (size_t i = 0; i < NUM_MODIFIERS; i++) {
		if (combo.modifiers & modifier_names[i].modifier) {
			dstr_cat(trigger, modifier_names[i].trigger);
			dstr_cat(trigger, "+");
		}
	}
	dstr_cat_dstr(trigger, &name);
	dstr_free(&name);
	return true;
}

/* File-local macros (BUILD.md): out of the next file in the unity group. */
#undef NUM_MODIFIERS
