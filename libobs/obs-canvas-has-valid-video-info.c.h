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

bool obs_canvas_has_valid_video_info(obs_canvas_t *canvas)
{
	struct obs_video_info *ovi = &canvas->ovi;
	return obs_canvas_rules_size_set(ovi->base_width) && obs_canvas_rules_size_set(ovi->base_height) &&
	       obs_canvas_rules_size_set(ovi->output_width) && obs_canvas_rules_size_set(ovi->output_height) &&
	       obs_canvas_rules_format_set((int)ovi->output_format);
}
