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

#include "libobs/build/libobs-ported-nix-system-info.pch.h"

/* The system info parsing, moved from obs-nix.c: the /proc/cpuinfo and
 * dmesg.boot line rules and the memory sizes, on plain values. obs-nix.c
 * keeps the file reads, the dstr copies, sysinfo(), sysctl(), the pointer
 * checks and the logs. Declared in obs-nix-system-info.h without EXPORT.
 * Ported to rust/obs-core (nix_system_info): the hybrid libobs drops this
 * file. */

/* log_processor_info() */
bool obs_nix_system_info_is_name(const char *line)
{
	return !strncmp(line, "model name", 10);
}

bool obs_nix_system_info_is_physical_id(const char *line)
{
	return !strncmp(line, "physical id", 11);
}

bool obs_nix_system_info_is_speed(const char *line)
{
	return !strncmp(line, "cpu MHz", 7);
}

const char *obs_nix_system_info_value(const char *line)
{
	const char *start = strchr(line, ':');
	if (!start || *(++start) == '\0')
		return NULL;

	return start;
}

int obs_nix_system_info_physical_id(const char *value)
{
	return atoi(value);
}

bool obs_nix_system_info_prints(const char *line, int physical_id, int last_physical_id)
{
	return *line == '\n' && physical_id != last_physical_id;
}

/* log_processor_speed() */
bool obs_nix_system_info_is_dmesg_cpu(const char *line)
{
	return !strncmp(line, "CPU: ", 5);
}

const char *obs_nix_system_info_dmesg_speed(const char *line, size_t *len)
{
	const char *start = strrchr(line, '(');
	if (!start || *(++start) == '\0')
		return NULL;

	*len = strcspn(start, "-");
	return start;
}

/* log_memory_info() */
uint64_t obs_nix_system_info_total_mb(uint64_t totalram, uint32_t mem_unit)
{
	return totalram * mem_unit / 1024 / 1024;
}

uint64_t obs_nix_system_info_free_mb(uint64_t freeram, uint64_t bufferram, uint32_t mem_unit)
{
	return (freeram + bufferram) * mem_unit / 1024 / 1024;
}

int64_t obs_nix_system_info_physmem_mb(int64_t mem)
{
	return mem / 1024 / 1024;
}
