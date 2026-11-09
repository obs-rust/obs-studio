/******************************************************************************
    Copyright (C) 2025 by Dennis Sädtler <saedtler@twitch.tv>

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

#include "libobs/build/libobs.pch.h"

static inline void canvas_dosignal(obs_canvas_t *canvas, const char *signal_obs, const char *signal_canvas)
{
	struct calldata data;
	uint8_t stack[128];

	calldata_init_fixed(&data, stack, sizeof(stack));
	calldata_set_ptr(&data, "canvas", canvas);
	if (signal_obs && obs_canvas_rules_public(canvas->context.private))
		signal_handler_signal(obs->signals, signal_obs, &data);
	if (signal_canvas)
		signal_handler_signal(canvas->context.signals, signal_canvas, &data);
}

static inline void canvas_dosignal_source(const char *signal, obs_canvas_t *canvas, obs_source_t *source)
{
	struct calldata data;
	uint8_t stack[128];

	calldata_init_fixed(&data, stack, sizeof(stack));
	calldata_set_ptr(&data, "canvas", canvas);
	calldata_set_ptr(&data, "source", source);

	signal_handler_signal(canvas->context.signals, signal, &data);
}
