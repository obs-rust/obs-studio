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

obs_scene_t *obs_canvas_scene_create(obs_canvas_t *canvas, const char *name)
{
	struct obs_source *source = obs_source_create_canvas(canvas, "scene", name, NULL, NULL);
	return source->context.data;
}

void obs_canvas_scene_remove(obs_scene_t *scene)
{
	obs_canvas_remove_source(scene->source);
}
