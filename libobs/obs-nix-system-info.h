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

#pragma once

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

/* The system info parsing of obs-nix.c's startup log, on plain values
 * (obs-nix-system-info.c):
 * - log_processor_info() (Linux): which key a /proc/cpuinfo line holds, the
 *   value after its first ':', the physical id as atoi() reads it, and when
 *   a block is printed;
 * - log_processor_speed() (FreeBSD): the speed of a dmesg.boot "CPU: " line;
 * - log_memory_info(): the MB values of the sysinfo() fields, and OpenBSD's
 *   HW_PHYSMEM64.
 *
 * The distribution, Flatpak and desktop log rules (obs-nix-log-rules.c):
 * - read_distribution_info() (obs-nix-os-release.h): which key an
 *   os-release line holds and where its value starts, the value's span with
 *   one pair of quotes cut, and the version when none was read;
 * - log_flatpak_extensions(): the name of an extension entry;
 * - log_desktop_session_info(): the form of the desktop line.
 *
 * obs-nix.c and obs-nix-os-release.h keep the file reads (fopen(),
 * getline(), fclose(), free()), every dstr copy and trim, the list splits,
 * sysinfo(), sysctl(), getenv(), the pointer checks and every log, each at
 * its point. Each key check is its own function, so the
 * three checks of a cpuinfo line stay where they were. The line and value
 * arguments are NUL-terminated, as getline() leaves them; the functions read
 * no further than that NUL, write nothing, call into no libobs and keep no
 * state.
 *
 * Hidden in libobs: not EXPORT. */

/* Whether a cpuinfo line starts with "model name" (compared by that length,
 * so a longer key with that prefix also matches). */
bool obs_nix_system_info_is_name(const char *line);

/* Whether a cpuinfo line starts with "physical id". */
bool obs_nix_system_info_is_physical_id(const char *line);

/* Whether a cpuinfo line starts with "cpu MHz". */
bool obs_nix_system_info_is_speed(const char *line);

/* The value of a cpuinfo line: the byte after its first ':', or NULL when
 * the line has no ':' or ends there. The caller copies it, cuts its last
 * byte (the newline) and depads it. */
const char *obs_nix_system_info_value(const char *line);

/* A physical id value as atoi() reads it. */
int obs_nix_system_info_physical_id(const char *value);

/* Whether a cpuinfo line ends a block that is printed: a line that starts
 * with a newline, with a physical id other than the last one printed. */
bool obs_nix_system_info_prints(const char *line, int physical_id, int last_physical_id);

/* Whether a dmesg.boot line starts with "CPU: ". */
bool obs_nix_system_info_is_dmesg_cpu(const char *line);

/* The speed of a dmesg.boot "CPU: " line: the byte after its last '(', with
 * its length up to the first '-' in *len, or NULL (and *len untouched) when
 * the line has no '(' or ends there. */
const char *obs_nix_system_info_dmesg_speed(const char *line, size_t *len);

/* The total memory in MB: totalram * mem_unit / 1024 / 1024, in uint64_t. */
uint64_t obs_nix_system_info_total_mb(uint64_t totalram, uint32_t mem_unit);

/* The free memory in MB: (freeram + bufferram) * mem_unit / 1024 / 1024, in
 * uint64_t. */
uint64_t obs_nix_system_info_free_mb(uint64_t freeram, uint64_t bufferram, uint32_t mem_unit);

/* OpenBSD's physical memory in MB: mem / 1024 / 1024, in int64_t. */
int64_t obs_nix_system_info_physmem_mb(int64_t mem);

/* read_distribution_info() */

/* The os-release keys read_distribution_info() reads. */
enum obs_nix_os_release_key {
	OBS_NIX_OS_RELEASE_NAME = 0,
	OBS_NIX_OS_RELEASE_VERSION_ID = 1,
	OBS_NIX_OS_RELEASE_BUILD_ID = 2,
};

/* The value of an os-release line: the byte after "NAME=", "VERSION_ID=" or
 * "BUILD_ID=" (compared by prefix, in that order), with the key in *key, or
 * NULL (and *key untouched) when the line starts with none of them. */
const char *obs_nix_system_info_os_release_field(const char *line, enum obs_nix_os_release_key *key);

/* The text of an os-release value: its span up to the first CR or LF in
 * *len, with one pair of matching '"' or '\'' around it cut when the span is
 * at least two bytes long. No escapes are read. The caller copies it only
 * when *len is not zero. */
const char *obs_nix_system_info_os_release_value(const char *start, size_t *len);

/* The version when the file gave none: NULL when a BUILD_ID was read (the
 * caller copies it), otherwise "rolling" for a distro other than "Unknown",
 * or "Unknown". */
const char *obs_nix_system_info_version_fallback(bool has_build_id, const char *distro);

/* log_flatpak_extensions() */

/* The name of a Flatpak extension entry, "name=commit", with its length in
 * *len, or NULL (and *len untouched) when the entry logs no line. The C
 * gives the first non-empty piece between its '='s, or the text "(null)"
 * when every piece is empty; the Rust port (rust/obs-core) gives the part
 * before the first '=', and NULL when that is empty. */
const char *obs_nix_system_info_extension_name(const char *ext, size_t *len);

/* log_desktop_session_info() */

/* The forms of the desktop line. */
enum obs_nix_desktop_form {
	/* No line. */
	OBS_NIX_DESKTOP_NONE = 0,
	/* "Desktop Environment: %s", with whichever name is set. */
	OBS_NIX_DESKTOP_ONE = 1,
	/* "Desktop Environment: %s (%s)", XDG_CURRENT_DESKTOP then
	 * XDG_SESSION_DESKTOP. */
	OBS_NIX_DESKTOP_BOTH = 2,
};

/* The form of the desktop line, from which of XDG_CURRENT_DESKTOP and
 * XDG_SESSION_DESKTOP are set (an empty value counts as set). */
enum obs_nix_desktop_form obs_nix_system_info_desktop_form(bool has_current, bool has_session);
