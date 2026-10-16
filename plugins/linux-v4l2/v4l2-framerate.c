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

#include "v4l2-framerate.h"
#include "v4l2-helpers.h"

void v4l2_framerate_list_add_discrete(obs_property_t *prop, struct dstr *buffer, uint32_t numerator,
				      uint32_t denominator)
{
	float fps = (float)denominator / numerator;
	int64_t pack = v4l2_pack_tuple(numerator, denominator);
	dstr_printf(buffer, "%.2f", (double)fps);
	obs_property_list_add_int(prop, buffer->array, pack);
}

int64_t v4l2_framerate_saved(int64_t setting, const struct v4l2_fract *intervals, size_t count)
{
	(void)intervals;
	(void)count;
	return setting;
}

bool v4l2_framerate_changed(int64_t current, int64_t requested, bool *unchanged)
{
	if (requested == -1 && !*unchanged) {
		*unchanged = true;
		return true;
	} else if (requested == -1 && *unchanged) {
		return false;
	}

	*unchanged = false;
	return current != requested;
}
