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

/* Free canvas mix (if any) */
void obs_canvas_clear_mix(obs_canvas_t *canvas)
{
	if (!canvas->mix)
		return;

	pthread_mutex_lock(&obs->video.mixes_mutex);
	for (size_t i = 0; i < obs->video.mixes.num; i++) {
		struct obs_core_video_mix *mix = obs->video.mixes.array[i];
		if (mix == canvas->mix) {
			da_erase(obs->video.mixes, i);
			obs_free_video_mix(mix);
			break;
		}
	}
	pthread_mutex_unlock(&obs->video.mixes_mutex);

	canvas->mix = NULL;
}
