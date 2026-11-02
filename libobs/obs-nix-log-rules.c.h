/******************************************************************************
    Copyright (C) 2023 by Lain Bailey <lain@obsproject.com>
    Copyright (C) 2014 by Zachary Lund <admin@computerquip.com>

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
******************************************************************************/

#include "libobs/build/libobs-nix-log-rules.pch.h"

/* The distribution, Flatpak and desktop log rules, moved from
 * obs-nix-os-release.h and obs-nix.c: the os-release keys, value span,
 * quote cut and version fallback, the Flatpak extension name and the desktop
 * line form, on plain values. The callers keep the file reads, the dstr
 * copies, the list split, getenv() and the logs. Declared in
 * obs-nix-system-info.h without EXPORT. */

/* read_distribution_info() */
const char *obs_nix_system_info_os_release_field(const char *line, enum obs_nix_os_release_key *key)
{
	if (!strncmp(line, "NAME=", 5)) {
		*key = OBS_NIX_OS_RELEASE_NAME;
		return line + 5;
	} else if (!strncmp(line, "VERSION_ID=", 11)) {
		*key = OBS_NIX_OS_RELEASE_VERSION_ID;
		return line + 11;
	} else if (!strncmp(line, "BUILD_ID=", 9)) {
		*key = OBS_NIX_OS_RELEASE_BUILD_ID;
		return line + 9;
	}

	return NULL;
}

const char *obs_nix_system_info_os_release_value(const char *start, size_t *len)
{
	*len = strcspn(start, "\r\n");
	if (*len >= 2 && (start[0] == '"' || start[0] == '\'') && start[*len - 1] == start[0]) {
		start++;
		*len -= 2;
	}

	return start;
}

const char *obs_nix_system_info_version_fallback(bool has_build_id, const char *distro)
{
	if (has_build_id)
		return NULL;

	return strcmp(distro, "Unknown") ? "rolling" : "Unknown";
}

/* log_flatpak_extensions() */
const char *obs_nix_system_info_extension_name(const char *ext, size_t *len)
{
	/* The first piece strlist_split(ext, '=', false) keeps: it drops the
	 * empty ones. */
	for (;;) {
		size_t piece = strcspn(ext, "=");
		if (piece) {
			*len = piece;
			return ext;
		}
		if (*ext == '\0')
			break;
		ext++;
	}

	/* Every piece is empty: the list is empty, and blog() printed its NULL
	 * first entry as "(null)". */
	*len = 6;
	return "(null)";
}

/* log_desktop_session_info() */
enum obs_nix_desktop_form obs_nix_system_info_desktop_form(bool has_current, bool has_session)
{
	if (has_current && has_session)
		return OBS_NIX_DESKTOP_BOTH;
	else if (has_current || has_session)
		return OBS_NIX_DESKTOP_ONE;

	return OBS_NIX_DESKTOP_NONE;
}
