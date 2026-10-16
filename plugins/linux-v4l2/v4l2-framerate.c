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
	/* "Leave Unchanged", or packed in 64 bits as the list stores it now */
	if (setting < 0 || (setting >> 32) != 0)
		return setting;

	/* A tuple from before the 64-bit packing: numerator in the high 16 bits */
	uint32_t legacy_num = (uint32_t)setting >> 16;
	uint32_t legacy_denom = (uint32_t)setting & 0xffff;

	for (size_t i = 0; i < count; i++) {
		if (legacy_num && intervals[i].numerator == legacy_num && intervals[i].denominator == legacy_denom)
			return v4l2_pack_tuple(intervals[i].numerator, intervals[i].denominator);
	}

	/* The truncated value: the denominator of the interval picked. If
	 * intervals with different numerators share it (a driver that does not
	 * reduce them, such as 333333/10000000 and 166667/10000000), it is
	 * unknown which one was picked. */
	const struct v4l2_fract *match = NULL;
	for (size_t i = 0; i < count; i++) {
		if (intervals[i].denominator != (uint32_t)setting)
			continue;
		if (match && match->numerator != intervals[i].numerator)
			return -1;
		match = &intervals[i];
	}
	if (match)
		return v4l2_pack_tuple(match->numerator, match->denominator);

	/* Neither names a rate (0/x or x/0 s per frame): leave the device's
	 * rate unchanged rather than have the driver pick its fastest. */
	if (!legacy_num || !legacy_denom)
		return -1;

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
