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

/*** Public Canvas Object API ***/

void obs_canvas_set_channel(obs_canvas_t *canvas, uint32_t channel, obs_source_t *source)
{
	assert(channel < MAX_CHANNELS);

	if (!obs_canvas_rules_channel_ok(channel))
		return;

	struct obs_view *view = &canvas->view;

	pthread_mutex_lock(&view->channels_mutex);

	source = obs_source_get_ref(source);

	obs_source_t *prev_source = view->channels[channel];

	if (source == prev_source) {
		obs_source_release(source);
		pthread_mutex_unlock(&view->channels_mutex);
		return;
	}

	struct calldata params = {0};
	calldata_set_ptr(&params, "canvas", canvas);
	calldata_set_int(&params, "channel", channel);
	calldata_set_ptr(&params, "prev_source", prev_source);
	calldata_set_ptr(&params, "source", source);

	signal_handler_signal(canvas->context.signals, "channel_change", &params);
	if (obs_canvas_rules_global_signals(canvas->flags))
		signal_handler_signal(obs->signals, "channel_change", &params);

	/* For some reason the original implementation allows overriding the source from the callback,
	 * so just in case support that here as well. This isn't used anywhere in OBS itself. */
	calldata_get_ptr(&params, "source", &source);
	view->channels[channel] = source;

	calldata_free(&params);
	pthread_mutex_unlock(&view->channels_mutex);

	if (source)
		obs_source_activate(source, view->type);

	if (prev_source) {
		obs_source_deactivate(prev_source, view->type);
		obs_source_release(prev_source);
	}
}
