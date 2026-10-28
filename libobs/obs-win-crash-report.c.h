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

#include "libobs/build/libobs-ported-win-crash-report.pch.h"

/* The crash report rules, moved from obs-win-crash-handler.c: the module
 * that holds an address, the thread passes, the frame line, the 32-bit
 * masks, the thread name prefix and when the filter passes on, on plain
 * values. obs-win-crash-handler.c keeps the DbgHelp, toolhelp and thread
 * calls, the conversions, the dstr calls and the static flags. Declared in
 * obs-win-crash-report.h without EXPORT. Ported to rust/obs-core
 * (win_crash_report): the hybrid libobs drops this file. */

/* enum_all_modules() and enum_module() */
bool obs_win_crash_report_contains(uint64_t addr, uint64_t base, uint32_t size)
{
	return addr >= base && addr < base + size;
}

/* write_thread_trace() */
bool obs_win_crash_report_is_crashed(uint32_t thread_id, uint32_t current_thread_id)
{
	return thread_id == current_thread_id;
}

bool obs_win_crash_report_in_pass(bool first_pass, bool crashed)
{
	return first_pass == crashed;
}

bool obs_win_crash_report_other_process(uint32_t owner_process_id, uint32_t current_process_id)
{
	return owner_process_id != current_process_id;
}

/* walk_stack() */
bool obs_win_crash_report_names_symbol(uint32_t flags)
{
	return (flags & OBS_WIN_CRASH_REPORT_SYMFLAG_EXPORT) == 0;
}

uint64_t obs_win_crash_report_mask_address(uint64_t value)
{
	return value & 0xFFFFFFFFF;
}

uint64_t obs_win_crash_report_mask_param(uint64_t value)
{
	return value & 0xFFFFFFFF;
}

/* get_thread_name() */
bool obs_win_crash_report_name_prefix(size_t len)
{
	return len != 0;
}

/* exception_handler() */
bool obs_win_crash_report_pass_on(bool debugger, const bool *inside)
{
	if (debugger)
		return true;

	return *inside;
}
