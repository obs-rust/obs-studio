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
#include <stddef.h>
#include <stdint.h>

/* The rules of obs-win-crash-handler.c's crash report, on plain values
 * (obs-win-crash-report.c):
 * - enum_all_modules() and enum_module(): which module holds an address;
 * - write_thread_trace(): which thread crashed, which pass traces it, and
 *   which threads belong to another process;
 * - walk_stack(): whether a frame is written with its symbol, and the masks
 *   a 32-bit build applies to the frame's values;
 * - get_thread_name(): whether a name gets its leading space;
 * - exception_handler(): when the exception is passed on untouched.
 *
 * obs-win-crash-handler.c keeps every DbgHelp, toolhelp and thread call, the
 * conversions, every dstr call and format string, and the static flags, each
 * at its point. The functions take no Windows type, so this header and its
 * file build on every OS; they write nothing, call into no libobs, keep no
 * state and allocate nothing, since they run inside the unhandled exception
 * filter.
 *
 * Hidden in libobs: not EXPORT. */

/* dbghelp.h's SYMFLAG_EXPORT, checked against it with a _Static_assert in
 * obs-win-crash-handler.c. */
#define OBS_WIN_CRASH_REPORT_SYMFLAG_EXPORT 0x200

/* Whether a module of this base and size holds addr: base <= addr <
 * base + size, the end in uint64_t, so a module that ends past 2^64 wraps
 * and holds nothing. */
bool obs_win_crash_report_contains(uint64_t addr, uint64_t base, uint32_t size);

/* Whether a thread is the crashed one: its id is the current thread's (the
 * filter runs on the thread that crashed). */
bool obs_win_crash_report_is_crashed(uint32_t thread_id, uint32_t current_thread_id);

/* Whether a thread is traced in this pass: the crashed thread in the first
 * pass, every other thread in the second. */
bool obs_win_crash_report_in_pass(bool first_pass, bool crashed);

/* Whether a thread belongs to another process, and so is skipped. */
bool obs_win_crash_report_other_process(uint32_t owner_process_id, uint32_t current_process_id);

/* Whether a resolved symbol is written by name: it is not an export. The
 * caller checks that the lookup succeeded first, and reads the flags only
 * then. */
bool obs_win_crash_report_names_symbol(uint32_t flags);

/* A 32-bit build's mask on a frame's stack and PC addresses: the low 36
 * bits. */
uint64_t obs_win_crash_report_mask_address(uint64_t value);

/* A 32-bit build's mask on each of a frame's four parameters: the low 32
 * bits. */
uint64_t obs_win_crash_report_mask_param(uint64_t value);

/* Whether a thread name of this length gets a leading space: it is not
 * empty. */
bool obs_win_crash_report_name_prefix(size_t len);

/* Whether the filter passes the exception on untouched: a debugger is
 * present, or the filter is already running (*inside). *inside is read only
 * when no debugger is present. */
bool obs_win_crash_report_pass_on(bool debugger, const bool *inside);
