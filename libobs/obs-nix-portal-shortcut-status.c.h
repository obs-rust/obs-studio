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

#include "libobs/build/libobs-ported-nix-portal-shortcut-status.pch.h"

/* ------------------------------------------------------------------------ */
/* Status decisions                                                          */

/* Ported to rust/obs-hotkey (portal_shortcut_status): the hybrid libobs drops
 * this file. */

/* A shortcut's description names this many of its actions; the rest are
 * counted (" and 2 more"). */
#define MAX_DESCRIBED_ACTIONS 3

enum obs_nix_portal_action obs_nix_portal_status_action(size_t count)
{
	if (count > MAX_DESCRIBED_ACTIONS)
		return OBS_NIX_PORTAL_ACTION_UNNAMED;
	return count == 1 ? OBS_NIX_PORTAL_ACTION_FIRST : OBS_NIX_PORTAL_ACTION_NEXT;
}

size_t obs_nix_portal_status_more(size_t count)
{
	return count > MAX_DESCRIBED_ACTIONS ? count - MAX_DESCRIBED_ACTIONS : 0;
}

bool obs_nix_portal_status_has_description(size_t i, size_t num)
{
	return i < num;
}

/* num_part > 0 also covers a request the desktop approved in full: then no
 * key is new and only a description changed, and the approved keys are
 * bound again rather than taken down with the dialog. */
enum obs_nix_portal_rebind obs_nix_portal_status_rebind(size_t num_part, size_t num_declined)
{
	if (num_part == 0)
		return OBS_NIX_PORTAL_REBIND_NONE;
	return num_declined > 0 ? OBS_NIX_PORTAL_REBIND_KEYS_DECLINED : OBS_NIX_PORTAL_REBIND_DESCRIPTION_DECLINED;
}

bool obs_nix_portal_status_keep_approved(bool request_declined, bool nothing)
{
	return request_declined && !nothing;
}

struct obs_nix_portal_outcome obs_nix_portal_status_after_bind(bool bound, bool partly, bool kept)
{
	struct obs_nix_portal_outcome outcome = {false, false, false, false};
	if (!bound) {
		/* bind_approved() set the status itself. */
		if (kept)
			return outcome;
		outcome.mark_declined = true;
		outcome.set_partly = true;
		outcome.partly = false;
		return outcome;
	}
	if (!partly)
		outcome.clear_declined = true;
	outcome.set_partly = true;
	outcome.partly = partly;
	return outcome;
}

bool obs_nix_portal_status_declined(bool request_declined, bool nothing)
{
	return request_declined || nothing;
}

/* On a desktop that picks the keys, a shortcut without a trigger is not
 * bound; on any other, every returned shortcut is. Only a bind reply, or a
 * desktop that picks the keys, changes it. */
enum obs_nix_portal_change obs_nix_portal_status_bound(bool desktop_keys, bool bind_reply, bool triggered)
{
	if (!bind_reply && !desktop_keys)
		return OBS_NIX_PORTAL_CHANGE_KEEP;
	return !desktop_keys || triggered ? OBS_NIX_PORTAL_CHANGE_SET : OBS_NIX_PORTAL_CHANGE_CLEAR;
}

enum obs_nix_portal_change obs_nix_portal_status_desktop_triggers(bool desktop_keys, bool bind_reply)
{
	if (desktop_keys)
		return OBS_NIX_PORTAL_CHANGE_SET;
	return bind_reply ? OBS_NIX_PORTAL_CHANGE_CLEAR : OBS_NIX_PORTAL_CHANGE_KEEP;
}

bool obs_nix_portal_status_can_configure(uint32_t version)
{
	return version >= 2;
}

#undef MAX_DESCRIBED_ACTIONS
