/*
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
*/

#include <math.h>

#include "obs-scene-bounds.h"

void obs_bounds_fit_scale(enum obs_bounds_type type, const struct vec2 *bounds, uint32_t cx, uint32_t cy,
			  struct vec2 *scale)
{
	float width = (float)cx * fabsf(scale->x);
	float height = (float)cy * fabsf(scale->y);
	float item_aspect = width / height;
	float bounds_aspect = bounds->x / bounds->y;
	enum obs_bounds_type bounds_type = type;

	if (type == OBS_BOUNDS_MAX_ONLY)
		if (width > bounds->x || height > bounds->y)
			bounds_type = OBS_BOUNDS_SCALE_INNER;

	if (bounds_type == OBS_BOUNDS_SCALE_INNER || bounds_type == OBS_BOUNDS_SCALE_OUTER) {
		bool use_width = (bounds_aspect < item_aspect);
		float mul;

		if (type == OBS_BOUNDS_SCALE_OUTER)
			use_width = !use_width;

		mul = use_width ? bounds->x / width : bounds->y / height;

		vec2_mulf(scale, scale, mul);

	} else if (bounds_type == OBS_BOUNDS_SCALE_TO_WIDTH) {
		vec2_mulf(scale, scale, bounds->x / width);

	} else if (bounds_type == OBS_BOUNDS_SCALE_TO_HEIGHT) {
		vec2_mulf(scale, scale, bounds->y / height);

	} else if (bounds_type == OBS_BOUNDS_STRETCH) {
		scale->x = copysignf(bounds->x / (float)cx, scale->x);
		scale->y = copysignf(bounds->y / (float)cy, scale->y);
	}
}
