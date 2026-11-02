/******************************************************************************
    Copyright (C) 2023 by Lain Bailey <lain@obsproject.com>

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
#include <stdint.h>

/* The rules of obs-windows.c's system info log, on plain values
 * (obs-windows-system-info.c):
 * - log_system_info(): the packed Windows version;
 * - log_available_memory(): the sizes in MB;
 * - log_gaming_features(): the Windows 10 gate, whether Game Mode is read a
 *   second time, whether each read found its value, and each feature's line,
 *   with its label and text, from its value or, for Game Mode with no value,
 *   from the Windows build;
 * - log_security_products_by_type(): the labels of a security product's
 *   state and type.
 *
 * obs-windows.c keeps get_win_ver(), GlobalMemoryStatusEx(), every registry
 * read, the COM calls, the 32-bit memory note, every blog() call and format
 * string, and the win_ver and win_build statics, each at its point. The
 * functions take no Windows type, so this header and its file build on every
 * OS; they keep no state and allocate nothing.
 *
 * Not EXPORT: obs-windows.c calls them inside libobs. */

#ifdef __cplusplus
extern "C" {
#endif

/* ERROR_SUCCESS (winerror.h), the status of a registry read that found its
 * value. obs-windows.c checks it with _Static_assert. */
#define OBS_WINDOWS_SYSTEM_INFO_ERROR_SUCCESS 0

/* WSC_SECURITY_PRODUCT_STATE (iwscapi.h) and WSC_SECURITY_PROVIDER
 * (wscapi.h). obs-windows.c checks them with _Static_assert. */
#define OBS_WINDOWS_SYSTEM_INFO_STATE_ON 0
#define OBS_WINDOWS_SYSTEM_INFO_STATE_OFF 1
#define OBS_WINDOWS_SYSTEM_INFO_STATE_SNOOZED 2
#define OBS_WINDOWS_SYSTEM_INFO_STATE_EXPIRED 3

#define OBS_WINDOWS_SYSTEM_INFO_PROVIDER_FIREWALL 0x1
#define OBS_WINDOWS_SYSTEM_INFO_PROVIDER_ANTIVIRUS 0x4
#define OBS_WINDOWS_SYSTEM_INFO_PROVIDER_ANTISPYWARE 0x8

/* The gaming features log_gaming_features() reads, in the order it logs
 * them. */
#define OBS_WINDOWS_SYSTEM_INFO_GAME_BAR 0
#define OBS_WINDOWS_SYSTEM_INFO_GAME_DVR_ALLOWED 1
#define OBS_WINDOWS_SYSTEM_INFO_GAME_DVR 2
#define OBS_WINDOWS_SYSTEM_INFO_GAME_DVR_BG_RECORDING 3
#define OBS_WINDOWS_SYSTEM_INFO_GAME_MODE 4
#define OBS_WINDOWS_SYSTEM_INFO_GAMING_FEATURES 5

/* log_system_info(): win_ver, (major << 8) | minor. */
uint32_t obs_windows_system_info_version(uint32_t major, uint32_t minor);

/* log_available_memory(): a size in MB, cut to 32 bits as the DWORD the log
 * prints. */
uint32_t obs_windows_system_info_mb(uint64_t bytes);

/* log_gaming_features(): whether the gaming features are logged at all
 * (Windows 10 and later). */
bool obs_windows_system_info_gaming_logged(uint32_t win_ver);

/* log_gaming_features(): whether Game Mode is read again under its older
 * value name, given the status of the first read. */
bool obs_windows_system_info_game_mode_retry(int32_t status);

/* log_gaming_features(): whether a feature's read found its value, given its
 * status. Only then is the value read. */
bool obs_windows_system_info_gaming_found(int32_t status);

/* log_gaming_features(): the line of a feature whose read found its value.
 * Returns false for a feature outside the five; otherwise sets *label and
 * *text, logged as "\t%s: %s". Neither is written on false. */
bool obs_windows_system_info_gaming_value_line(int feature, uint32_t value, const char **label, const char **text);

/* log_gaming_features(): whether a feature whose value is missing may still
 * get a line (Game Mode). Only then is the Windows build read. */
bool obs_windows_system_info_gaming_fallback(int feature);

/* log_gaming_features(): the line of such a feature, from the Windows build.
 * Returns false when none is logged; otherwise sets *label and *text, as
 * above. Neither is written on false. */
bool obs_windows_system_info_gaming_fallback_line(int feature, uint32_t win_build, const char **label,
						  const char **text);

/* log_security_products_by_type(): the label of a product's state and of its
 * type, "unknown" for any other value. */
const char *obs_windows_system_info_product_state(int state);
const char *obs_windows_system_info_product_type(int type);

#ifdef __cplusplus
}
#endif
