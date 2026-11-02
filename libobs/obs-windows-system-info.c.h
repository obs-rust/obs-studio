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

#include "libobs/build/libobs-windows-system-info.pch.h"

/* The system info log rules, moved from obs-windows.c: the packed version,
 * the memory sizes, the gaming feature lines and the security product
 * labels, on plain values. obs-windows.c keeps the Windows calls, the
 * registry reads, the blog() calls and the statics. Declared in
 * obs-windows-system-info.h without EXPORT. */

/* log_system_info() */
uint32_t obs_windows_system_info_version(uint32_t major, uint32_t minor)
{
	return (major << 8) | minor;
}

/* log_available_memory() */
uint32_t obs_windows_system_info_mb(uint64_t bytes)
{
	return (uint32_t)(bytes / 1048576);
}

/* log_gaming_features() */
bool obs_windows_system_info_gaming_logged(uint32_t win_ver)
{
	return !(win_ver < 0xA00);
}

bool obs_windows_system_info_game_mode_retry(int32_t status)
{
	return status != OBS_WINDOWS_SYSTEM_INFO_ERROR_SUCCESS;
}

bool obs_windows_system_info_gaming_found(int32_t status)
{
	return status == OBS_WINDOWS_SYSTEM_INFO_ERROR_SUCCESS;
}

bool obs_windows_system_info_gaming_value_line(int feature, uint32_t value, const char **label, const char **text)
{
	switch (feature) {
	case OBS_WINDOWS_SYSTEM_INFO_GAME_BAR:
		*label = "Game Bar";
		*text = (bool)value ? "On" : "Off";
		return true;
	case OBS_WINDOWS_SYSTEM_INFO_GAME_DVR_ALLOWED:
		*label = "Game DVR Allowed";
		*text = (bool)value ? "Yes" : "No";
		return true;
	case OBS_WINDOWS_SYSTEM_INFO_GAME_DVR:
		*label = "Game DVR";
		*text = (bool)value ? "On" : "Off";
		return true;
	case OBS_WINDOWS_SYSTEM_INFO_GAME_DVR_BG_RECORDING:
		*label = "Game DVR Background Recording";
		*text = (bool)value ? "On" : "Off";
		return true;
	case OBS_WINDOWS_SYSTEM_INFO_GAME_MODE:
		*label = "Game Mode";
		*text = (bool)value ? "On" : "Off";
		return true;
	default:
		return false;
	}
}

bool obs_windows_system_info_gaming_fallback(int feature)
{
	return feature == OBS_WINDOWS_SYSTEM_INFO_GAME_MODE;
}

bool obs_windows_system_info_gaming_fallback_line(int feature, uint32_t win_build, const char **label,
						  const char **text)
{
	if (feature == OBS_WINDOWS_SYSTEM_INFO_GAME_MODE && win_build >= 19042) {
		// On by default in newer Windows 10 builds (no registry key set)
		*label = "Game Mode";
		*text = "Probably On (no reg key set)";
		return true;
	}
	return false;
}

/* log_security_products_by_type() */
const char *obs_windows_system_info_product_state(int state)
{
	switch (state) {
	case OBS_WINDOWS_SYSTEM_INFO_STATE_ON:
		return "enabled";
	case OBS_WINDOWS_SYSTEM_INFO_STATE_OFF:
		return "disabled";
	case OBS_WINDOWS_SYSTEM_INFO_STATE_SNOOZED:
		return "temporarily disabled";
	case OBS_WINDOWS_SYSTEM_INFO_STATE_EXPIRED:
		return "expired";
	default:
		return "unknown";
	}
}

const char *obs_windows_system_info_product_type(int type)
{
	switch (type) {
	case OBS_WINDOWS_SYSTEM_INFO_PROVIDER_ANTIVIRUS:
		return "AV";
	case OBS_WINDOWS_SYSTEM_INFO_PROVIDER_FIREWALL:
		return "FW";
	case OBS_WINDOWS_SYSTEM_INFO_PROVIDER_ANTISPYWARE:
		return "ASW";
	default:
		return "unknown";
	}
}
