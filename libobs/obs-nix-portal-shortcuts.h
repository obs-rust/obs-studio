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

#pragma once

#include "obs-nix-hotkey-state.h"
#include "util/dstr.h"

/* Global hotkeys on Wayland through the xdg-desktop-portal GlobalShortcuts
 * interface.
 *
 * One portal shortcut is bound per distinct key combination in the current
 * OBS hotkey bindings, not per OBS action. The shortcut id names the
 * combination, so it stays the same across profiles and scene collections,
 * and one shortcut drives every action bound to that combination. The OBS
 * binding is offered to the desktop as the preferred trigger; the desktop
 * decides the final trigger. Activated/Deactivated feed
 * obs_nix_hotkey_state, and the libobs hotkey thread then presses and
 * releases the matching bindings exactly as it does on X11.
 *
 * The set is bound again 500 ms after the last hotkey_bindings_changed or
 * hotkey_unregister signal, in a new session (the portal allows one
 * BindShortcuts per session). A session the desktop closes is not bound
 * again until a new combination is assigned; a restarted portal is.
 *
 * While obs_hotkey_enable_background_press(false) is in effect (the
 * "Disable hotkeys when main window is not in focus" setting, or a dialog
 * that disables hotkeys), the hotkey thread drops every press, so the
 * session is closed and the desktop stops holding the keys for OBS; the
 * current set is bound again when presses are enabled.
 *
 * Without the portal (or without its GlobalShortcuts interface) the client
 * logs once and stops; hotkeys keep working only while OBS is focused. */

struct obs_nix_portal_shortcuts;

/* XKB keysym name of key for the preferred trigger ("F13", "a"); return
 * false if there is none. */
typedef bool (*obs_nix_portal_key_name_func)(obs_key_t key, struct dstr *name);

struct obs_nix_portal_shortcuts *obs_nix_portal_shortcuts_start(struct obs_nix_hotkey_state *state,
								obs_nix_portal_key_name_func key_name);
void obs_nix_portal_shortcuts_stop(struct obs_nix_portal_shortcuts *portal);

/* Shortcut id <-> key combination ("ctrl+shift+OBS_KEY_F13"). */
bool obs_nix_portal_shortcut_id(obs_key_combination_t combo, struct dstr *id);
bool obs_nix_portal_shortcut_combo(const char *id, obs_key_combination_t *combo);

/* Preferred trigger in the XDG shortcuts format ("CTRL+SHIFT+F13"). */
bool obs_nix_portal_shortcut_trigger(obs_key_combination_t combo, obs_nix_portal_key_name_func key_name,
				     struct dstr *trigger);

/* Whether combo can be a portal shortcut. Shared by the id codec
 * (obs-nix-portal-shortcut-ids.c) and the binding collection
 * (obs-nix-portal-shortcuts.c). */
static inline bool bindable(obs_key_combination_t combo)
{
	/* Portal triggers are keyboard shortcuts; modifier-only and mouse
	 * combinations keep working only while OBS is focused. */
	if (combo.key <= OBS_KEY_NONE || combo.key >= OBS_KEY_LAST_VALUE)
		return false;
	return combo.key < OBS_KEY_MOUSE1 || combo.key > OBS_KEY_MOUSE29;
}

/* Whether a and b are the same key combination. Shared by the combination
 * sets (obs-nix-portal-shortcut-sets.c) and the binding collection
 * (obs-nix-portal-shortcuts.c). */
static inline bool same_combo(obs_key_combination_t a, obs_key_combination_t b)
{
	return a.key == b.key && a.modifiers == b.modifiers;
}

/* The combination set logic of the portal client
 * (obs-nix-portal-shortcut-sets.c): which bindings to ask the desktop for,
 * and when to ask again. It only reads the arrays, which the portal client
 * owns, and returns positions in them. */
struct obs_nix_portal_combos {
	const obs_key_combination_t *array;
	size_t num;
};

/* The description of each combination, parallel to an
 * obs_nix_portal_combos. There may be fewer descriptions than combinations,
 * and a description may be NULL. */
struct obs_nix_portal_descriptions {
	const char *const *array;
	size_t num;
};

/* Whether combos holds combo. */
bool obs_nix_portal_sets_has(const struct obs_nix_portal_combos *combos, obs_key_combination_t combo);

/* Whether a sync leaves the portal session as it is. With a session:
 * current holds the combinations of bound, each with the same description,
 * unless retry asks again for a partly declined set. Without one: current
 * is empty, or every combination of it was declined. */
bool obs_nix_portal_sets_unchanged(bool session, bool retry, bool partly_declined,
				   const struct obs_nix_portal_combos *current,
				   const struct obs_nix_portal_descriptions *current_descriptions,
				   const struct obs_nix_portal_combos *bound,
				   const struct obs_nix_portal_descriptions *bound_descriptions,
				   const struct obs_nix_portal_combos *declined);

/* The combinations of current to ask the desktop for, as positions in
 * current, in its order (request has room for current->num): all of them,
 * or while partly_declined, those not in declined. Returns whether that is
 * only part of current. */
bool obs_nix_portal_sets_request(const struct obs_nix_portal_combos *current, bool partly_declined,
				 const struct obs_nix_portal_combos *declined, size_t *request, size_t *num_request);

/* The desktop declined request: the combinations of current that it
 * approved before and that are in request (part), and the other ones of
 * current (declined), as positions in current, in its order (each has room
 * for current->num). Returns whether part is bound again: it is neither
 * empty nor as large as request. */
bool obs_nix_portal_sets_partition(const struct obs_nix_portal_combos *current,
				   const struct obs_nix_portal_combos *approved,
				   const struct obs_nix_portal_combos *request, size_t *part, size_t *num_part,
				   size_t *declined, size_t *num_declined);
