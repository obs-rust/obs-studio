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

/* The reconnect policy of obs-output.c, on plain values
 * (obs-output-reconnect.c). One function per decision, each asked where the
 * C made it, from values the caller reads at that point:
 * - output_reconnect(), on the first attempt (reconnecting not set):
 *   obs_output_reconnect_first_delay(), the delay from the setting; then
 *   obs_output_reconnect_give_up() (the output stops as disconnected), and
 *   obs_output_reconnect_grows() and _next_delay() (a later attempt waits
 *   longer, up to the limit);
 * - can_reconnect(): obs_output_reconnect_active() (whether reconnecting is
 *   on, and so whether the reconnect callback is asked), then
 *   obs_output_reconnect_can() (whether the stop reconnects).
 *
 * obs-output.c keeps obs_output_set_reconnect_settings() and its stores, the
 * exponent's random jitter at create, obs_output_valid(), every atomic load
 * and store (reconnecting, delay_active, delay_capturing,
 * delay_restart_refs), each loaded where it always was and tested in its
 * && order, the reconnect stop event's waits, tests and reset, the stop
 * code stores, the reconnect callback and its NULL test,
 * reconnect_thread() with its pthread_create(), pthread_detach() and
 * obs_output_actual_start(), obs_output_end_data_capture() and
 * obs_output_end_data_capture_internal(), the retry count's reset and
 * increment, the delay's stores, signal_reconnect() and every log. Nothing
 * here takes a lock, calls into libobs or keeps state.
 *
 * The constants are copied below, so this header needs no core header;
 * obs-output.c checks them against obs-defs.h.
 *
 * Hidden in libobs: not EXPORT. */

/* OBS_OUTPUT_SUCCESS, OBS_OUTPUT_DISCONNECTED */
#define OBS_OUTPUT_RECONNECT_SUCCESS 0
#define OBS_OUTPUT_RECONNECT_DISCONNECTED -5

/* The longest wait between two attempts, in milliseconds (15 minutes). */
#define OBS_OUTPUT_RECONNECT_MAX_MSEC (15 * 60 * 1000)

/* output_reconnect(), on the first attempt: the wait before it, in
 * milliseconds, from the delay setting retry_sec (reconnect_retry_sec, in
 * seconds). */
uint32_t obs_output_reconnect_first_delay(int retry_sec);

/* output_reconnect(): retries attempts were made (reconnect_retries, 0 on the
 * first) out of retry_max (reconnect_retry_max), so the output gives up and
 * stops as disconnected. */
bool obs_output_reconnect_give_up(int retries, int retry_max);

/* output_reconnect(), for an attempt that goes ahead: retries attempts were
 * made before it, so this one waits longer than the last
 * (obs_output_reconnect_next_delay()); the first keeps its first delay. */
bool obs_output_reconnect_grows(int retries);

/* output_reconnect(): the wait before a later attempt, from the last one
 * (cur_msec, reconnect_retry_cur_msec) times the exponent (exp,
 * reconnect_retry_exp), at most OBS_OUTPUT_RECONNECT_MAX_MSEC. */
uint32_t obs_output_reconnect_next_delay(uint32_t cur_msec, float exp);

/* can_reconnect(): reconnecting is on for a count of retry_max
 * (reconnect_retry_max), so the reconnect callback, if any, is asked first,
 * and a disconnect reconnects. */
bool obs_output_reconnect_active(int retry_max);

/* can_reconnect(), once the callback, if asked, agreed: a stop with code
 * reconnects. A failed attempt (reconnecting) retries on any code but
 * OBS_OUTPUT_SUCCESS; otherwise only a disconnect reconnects, and only while
 * reconnecting is on (active, from obs_output_reconnect_active()). */
bool obs_output_reconnect_can(bool reconnecting, bool active, int code);
