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

/* The audio render mix plan of obs-source.c, on plain values
 * (obs-source-audio-render.c): which way obs_source_audio_render() takes,
 * and which mixes custom_audio_render() and process_audio_source_tick()
 * clear, copy and scale.
 *
 * - obs_source_audio_render_route(): pending without an output buffer, or
 *   with an audio_render callback but no data yet (a source still being
 *   created); the callback with data; else the tick;
 * - custom_audio_render(), around the plugin's audio_render callback:
 *   - obs_source_audio_render_custom_clear_before(): a mix in both the
 *     source's mixers and the tick's is cleared before the callback;
 *   - obs_source_audio_render_custom_ts() and _custom_pending(): the
 *     callback's timestamp when it succeeds, else 0 and pending;
 *   - obs_source_audio_render_custom_unmixed(): with no timestamp or no
 *     tick mixers, nothing is cleared or scaled after the callback;
 *   - obs_source_audio_render_custom_in_tick() and _custom_clear_after():
 *     a mix in the tick's mixers but not the source's is cleared after it;
 * - process_audio_source_tick(), on the buffered input:
 *   - obs_source_audio_render_tick_unset() and _tick_short(): pending with
 *     no timestamp or less input than the tick needs;
 *   - obs_source_audio_render_tick_stop() and _tick_copy(): mixes 1 to 5
 *     copy mix 0 when in both masks, else are cleared; a submix source
 *     (OBS_SOURCE_SUBMIX) does mix 1 only, as if both masks were 1;
 *   - obs_source_audio_render_tick_volume() and _tick_clear_main(): a
 *     submix source stops there; any other clears mix 0 unless it is in
 *     both masks, then scales by its volume.
 *
 * obs-source.c keeps every read and write of the source's fields, in the
 * same order and under the same conditions, and passes the values it read:
 * the output buffer, the callbacks and context.data only where the checks
 * before reached them (the audio thread can render a source still being
 * created), audio_ts and the input size under audio_buf_mutex, and
 * audio_mixers (set by other threads, without a lock) once per mix where it
 * was read. It keeps audio_buf_mutex, deque_peek_front(), every memset and
 * memcpy, the plugin callbacks, audio_submix() and apply_audio_volume().
 * None of the functions takes a lock, calls into libobs, touches memory or
 * keeps state.
 *
 * Hidden in libobs: not EXPORT. */

/* Which way obs_source_audio_render() takes. */
enum obs_source_audio_render_route {
	/* no output buffer, or a custom render without data: pending */
	OBS_SOURCE_AUDIO_RENDER_PENDING = 0,
	/* the source's audio_render callback (custom_audio_render()) */
	OBS_SOURCE_AUDIO_RENDER_CUSTOM = 1,
	/* the buffered input (audio_submix() first when the source has an
	 * audio_mix callback, then process_audio_source_tick()) */
	OBS_SOURCE_AUDIO_RENDER_TICK = 2,
};

/* PENDING without has_buffer; with custom, CUSTOM when has_data, else
 * PENDING; else TICK. The caller reads custom only with has_buffer and
 * has_data only with custom. */
enum obs_source_audio_render_route obs_source_audio_render_route(bool has_buffer, bool custom, bool has_data);

/* (audio_mixers & mixers & (1 << mix)) != 0: custom_audio_render() clears
 * the mix before the callback. mix is below MAX_AUDIO_MIXES (the C shifts
 * an int by it). */
bool obs_source_audio_render_custom_clear_before(uint32_t audio_mixers, uint32_t mixers, size_t mix);

/* success ? ts : 0, the audio_ts the callback leaves. ts is ignored when
 * the callback failed (it may not have written it). */
uint64_t obs_source_audio_render_custom_ts(bool success, uint64_t ts);

/* !success: the source is pending after the callback. */
bool obs_source_audio_render_custom_pending(bool success);

/* !audio_ts || !mixers: after a successful callback, nothing is cleared or
 * scaled. */
bool obs_source_audio_render_custom_unmixed(uint64_t audio_ts, uint32_t mixers);

/* (mixers & (1 << mix)) != 0: the clear after the callback looks at the
 * mix. */
bool obs_source_audio_render_custom_in_tick(uint32_t mixers, size_t mix);

/* (audio_mixers & (1 << mix)) == 0: a mix of the tick that the source is
 * not on is cleared after the callback. */
bool obs_source_audio_render_custom_clear_after(uint32_t audio_mixers, size_t mix);

/* !audio_ts: no input timestamp yet, the tick is pending. */
bool obs_source_audio_render_tick_unset(uint64_t audio_ts);

/* input_size < size: not enough buffered input (in bytes of channel 0),
 * the tick is pending. */
bool obs_source_audio_render_tick_short(size_t input_size, size_t size);

/* submix && mix > 1: a submix source fills mix 1 only. */
bool obs_source_audio_render_tick_stop(bool submix, size_t mix);

/* Whether mix copies mix 0 (else it is cleared): in both audio_mixers and
 * mixers; for a submix source, whether audio_mixers has bit 0 (the C sets
 * both masks to 1 there). */
bool obs_source_audio_render_tick_copy(bool submix, uint32_t audio_mixers, uint32_t mixers, size_t mix);

/* !submix: the tick goes on to mix 0 and the volume. */
bool obs_source_audio_render_tick_volume(bool submix);

/* (audio_mixers & 1) == 0 || (mixers & 1) == 0: mix 0 is cleared before
 * the volume. */
bool obs_source_audio_render_tick_clear_main(uint32_t audio_mixers, uint32_t mixers);
